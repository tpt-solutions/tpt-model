//! `#[derive(Model)]` for `tpt-model`.
//!
//! Supported attributes:
//!
//! Type level (`struct` or `enum`):
//! * `version = N`: schema version, default `1`.
//! * `name = "..."`: logical name, default is the type name without a trailing `V<N>`.
//!
//! Field level:
//! * `since = N`: version the field was added in, default `1`.
//! * `removed_in = N`: version the field is removed in. Requires `migrate`.
//! * `migrate = "path::to::fn"`: migration function for the removed data. Checked for type.
//! * `default = "expr"`: default used by automatic evolution (see `tpt_model::default_expr`).
//! * `rename = "old"`: former name. May be repeated.
//! * `intent = "read" | "write"`, `capability = "..."`: field access rule.
//! * `min = N`, `max = N`: value bounds (length bounds for strings, bytes and sequences).
//!
//! Variant level (enums, unit and newtype variants only): `since`, `removed_in`, `migrate`.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, Data, DeriveInput, Fields, GenericArgument,
    LitInt, LitStr, PathArguments, Type,
};

/// Derives `tpt_model::Model`, emitting the schema graph as `const` data.
#[proc_macro_derive(Model, attributes(tpt))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

const ITEM_KEYS: &[&str] = &["version", "name"];
const FIELD_KEYS: &[&str] = &[
    "since",
    "removed_in",
    "migrate",
    "default",
    "rename",
    "intent",
    "capability",
    "min",
    "max",
];
const VARIANT_KEYS: &[&str] = &["since", "removed_in", "migrate"];

#[derive(Default)]
struct Attrs {
    version: Option<(u32, Span)>,
    name: Option<(String, Span)>,
    since: Option<(u32, Span)>,
    removed_in: Option<(u32, Span)>,
    migrate: Option<(String, Span)>,
    default: Option<(String, Span)>,
    renames: Vec<(String, Span)>,
    intent: Option<(String, Span)>,
    capability: Option<(String, Span)>,
    min: Option<(i64, Span)>,
    max: Option<(i64, Span)>,
}

/// Reads every `#[tpt(...)]` attribute, rejecting keys outside `allowed`.
fn parse_attrs(attrs: &[Attribute], allowed: &[&str], context: &str) -> syn::Result<Attrs> {
    let mut out = Attrs::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("tpt")) {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(|i| i.to_string())
                .ok_or_else(|| meta.error("expected a simple attribute name"))?;
            let span = meta.path.span();
            if !allowed.contains(&key.as_str()) {
                return Err(meta.error(format!("unknown tpt attribute `{key}` on {context}")));
            }
            match key.as_str() {
                "version" => out.version = Some((parse_u32(&meta)?, span)),
                "since" => out.since = Some((parse_u32(&meta)?, span)),
                "removed_in" => out.removed_in = Some((parse_u32(&meta)?, span)),
                "name" => out.name = Some((parse_str(&meta)?, span)),
                "migrate" => out.migrate = Some((parse_str(&meta)?, span)),
                "default" => out.default = Some((parse_str(&meta)?, span)),
                "rename" => out.renames.push((parse_str(&meta)?, span)),
                "intent" => out.intent = Some((parse_str(&meta)?, span)),
                "capability" => out.capability = Some((parse_str(&meta)?, span)),
                "min" => out.min = Some((parse_i64(&meta)?, span)),
                "max" => out.max = Some((parse_i64(&meta)?, span)),
                _ => unreachable!("allowed keys are checked above"),
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn parse_u32(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<u32> {
    let lit: LitInt = meta.value()?.parse()?;
    lit.base10_parse::<u32>()
}

fn parse_str(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    let lit: LitStr = meta.value()?.parse()?;
    Ok(lit.value())
}

/// Strips a trailing `V<digits>` from a type name: `CadPartV2` -> `CadPart`.
fn default_name(ident: &str) -> String {
    if let Some(pos) = ident.rfind('V') {
        let suffix = &ident[pos + 1..];
        if pos > 0 && !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return ident[..pos].to_string();
        }
    }
    ident.to_string()
}

fn opt_u32(v: Option<u32>) -> TokenStream2 {
    match v {
        Some(n) => quote!(::core::option::Option::Some(#n)),
        None => quote!(::core::option::Option::None),
    }
}

fn opt_str(v: Option<&str>) -> TokenStream2 {
    match v {
        Some(s) => quote!(::core::option::Option::Some(#s)),
        None => quote!(::core::option::Option::None),
    }
}

fn private_path() -> TokenStream2 {
    quote!(::tpt_model::__private)
}

/// Emits a `TypeRef` value expression for a Rust type.
fn type_ref(ty: &Type) -> syn::Result<TokenStream2> {
    let p = private_path();
    match ty {
        Type::Reference(r) => type_ref(&r.elem),
        Type::Path(tp) if tp.qself.is_none() => {
            let seg = tp
                .path
                .segments
                .last()
                .ok_or_else(|| syn::Error::new(ty.span(), "empty type path"))?;
            let name = seg.ident.to_string();
            let args: Vec<&Type> = match &seg.arguments {
                PathArguments::AngleBracketed(ab) => ab
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        GenericArgument::Type(t) => Some(t),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let prim = |variant: TokenStream2| quote!(#p::TypeRef::Prim(#p::Prim::#variant));
            Ok(match name.as_str() {
                "bool" => prim(quote!(Bool)),
                "i8" | "i16" | "i32" | "i64" | "isize" => prim(quote!(I64)),
                "u8" | "u16" | "u32" | "u64" | "usize" => prim(quote!(U64)),
                "f32" | "f64" => prim(quote!(F64)),
                "u128" | "i128" => return Err(syn::Error::new(
                    ty.span(),
                    "128-bit integers are not supported by tpt-model; use u64, i64 or a byte array",
                )),
                "String" | "str" => prim(quote!(String)),
                "EntityId" | "UuidV7" => prim(quote!(Uuid)),
                "Option" => {
                    let inner = one_arg(&args, ty.span(), "Option")?;
                    let inner = type_ref(inner)?;
                    quote!(#p::TypeRef::Option(&#inner))
                }
                "Box" => {
                    let inner = one_arg(&args, ty.span(), "Box")?;
                    type_ref(inner)?
                }
                "Vec" => {
                    let inner = one_arg(&args, ty.span(), "Vec")?;
                    if is_u8(inner) {
                        prim(quote!(Bytes))
                    } else {
                        let inner = type_ref(inner)?;
                        quote!(#p::TypeRef::Array(&#inner))
                    }
                }
                "BTreeMap" | "HashMap" => {
                    if args.len() != 2 || !is_string(args[0]) {
                        return Err(syn::Error::new(ty.span(), "maps must be keyed by String"));
                    }
                    let value = type_ref(args[1])?;
                    quote!(#p::TypeRef::Map(&#value))
                }
                _ => {
                    let named = default_name(&name);
                    quote!(#p::TypeRef::Named(#named))
                }
            })
        }
        Type::Array(_) => Err(syn::Error::new(
            ty.span(),
            "fixed-size arrays are not supported by tpt-model; use Vec<T>",
        )),
        Type::Tuple(_) => Err(syn::Error::new(
            ty.span(),
            "tuples are not supported by tpt-model; use a named struct",
        )),
        _ => Err(syn::Error::new(
            ty.span(),
            "unsupported field type for tpt-model",
        )),
    }
}

fn one_arg<'a>(args: &[&'a Type], span: Span, what: &str) -> syn::Result<&'a Type> {
    match args {
        [one] => Ok(one),
        _ => Err(syn::Error::new(
            span,
            format!("{what} takes exactly one type argument"),
        )),
    }
}

fn is_u8(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.is_ident("u8"))
}

fn is_string(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => tp.path.segments.last().is_some_and(|s| s.ident == "String"),
        Type::Reference(r) => is_string(&r.elem),
        _ => false,
    }
}

/// Checks the `removed_in` / `migrate` pairing and emits the migration signature check.
fn removal(
    name: &str,
    span: Span,
    version: u32,
    since: u32,
    removed_in: Option<(u32, Span)>,
    migrate: &Option<(String, Span)>,
    what: &str,
) -> syn::Result<(TokenStream2, Option<TokenStream2>)> {
    let p = private_path();
    if let Some((removed, rspan)) = removed_in {
        if removed <= version {
            return Err(syn::Error::new(
                rspan,
                format!("{what} `{name}` has removed_in = {removed}, which must be greater than this version ({version})"),
            ));
        }
        if removed <= since {
            return Err(syn::Error::new(
                rspan,
                format!("{what} `{name}` is removed before it is added"),
            ));
        }
    }
    match (removed_in.is_some(), migrate) {
        (true, None) => {
            return Err(syn::Error::new(
                span,
                format!("{what} `{name}` is removed in a later version, so it needs `migrate = \"...\"` to say where its data goes"),
            ))
        }
        (false, Some((_, mspan))) => {
            return Err(syn::Error::new(*mspan, format!("`migrate` on {what} `{name}` requires `removed_in`")))
        }
        _ => {}
    }
    let removed_tokens = opt_u32(removed_in.map(|(r, _)| r));
    let migrate_tokens = match migrate {
        None => None,
        Some((path_str, mspan)) => {
            let path: syn::Path = syn::parse_str(path_str)
                .map_err(|e| syn::Error::new(*mspan, format!("invalid migrate path: {e}")))?;
            // Forces the named function to exist with the expected signature.
            Some(quote! {
                const _: fn(&#p::UirNode, &#p::MigrationContext) -> ::core::result::Result<#p::UirNode, #p::MigrationError> = #path;
            })
        }
    };
    Ok((removed_tokens, migrate_tokens))
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "#[derive(Model)] does not support generic types",
        ));
    }
    let item = parse_attrs(&input.attrs, ITEM_KEYS, "the type")?;
    let ident = &input.ident;
    let version = item.version.map(|(v, _)| v).unwrap_or(1);
    if version == 0 {
        return Err(syn::Error::new(
            item.version.map(|(_, s)| s).unwrap_or(ident.span()),
            "version must be at least 1",
        ));
    }
    let name = item
        .name
        .map(|(n, _)| n)
        .unwrap_or_else(|| default_name(&ident.to_string()));
    let p = private_path();

    let (body, checks) = match &input.data {
        Data::Struct(s) => {
            let named = match &s.fields {
                Fields::Named(n) => n,
                _ => {
                    return Err(syn::Error::new(
                        ident.span(),
                        "only structs with named fields are supported",
                    ))
                }
            };
            let mut fields = Vec::new();
            let mut checks = Vec::new();
            for f in &named.named {
                let fname = f
                    .ident
                    .as_ref()
                    .expect("named field")
                    .to_string()
                    .trim_start_matches("r#")
                    .to_string();
                let attrs = parse_attrs(&f.attrs, FIELD_KEYS, "a field")?;
                let fspan = f.span();
                let since_v = attrs.since.map(|(v, _)| v).unwrap_or(1);
                if since_v == 0 || since_v > version {
                    return Err(syn::Error::new(
                        attrs.since.map(|(_, s)| s).unwrap_or(fspan),
                        format!("field `{fname}` has since = {since_v}, outside 1..={version}"),
                    ));
                }
                let (removed, migrate_check) = removal(
                    &fname,
                    fspan,
                    version,
                    since_v,
                    attrs.removed_in,
                    &attrs.migrate,
                    "field",
                )?;
                checks.extend(migrate_check);

                let ty = type_ref(&f.ty)?;
                let default = attrs.default.as_ref().map(|(d, _)| d.as_str());
                let renames: Vec<&str> = attrs.renames.iter().map(|(r, _)| r.as_str()).collect();
                let access = access_tokens(&attrs, &fname)?;
                let constraints = constraint_tokens(&attrs);
                let migrate = opt_str(attrs.migrate.as_ref().map(|(m, _)| m.as_str()));
                let default = opt_str(default);
                fields.push(quote! {
                    #p::Field {
                        name: #fname,
                        ty: #ty,
                        since: #since_v,
                        removed_in: #removed,
                        default: #default,
                        renames: &[#(#renames),*],
                        access: #access,
                        migrate: #migrate,
                        constraints: #constraints,
                    }
                });
            }
            (quote!(#p::TypeDef::Struct(&[#(#fields),*])), checks)
        }
        Data::Enum(e) => {
            let mut variants = Vec::new();
            let mut checks = Vec::new();
            for v in &e.variants {
                let vname = v.ident.to_string();
                let attrs = parse_attrs(&v.attrs, VARIANT_KEYS, "a variant")?;
                let vspan = v.span();
                let since_v = attrs.since.map(|(x, _)| x).unwrap_or(1);
                if since_v == 0 || since_v > version {
                    return Err(syn::Error::new(
                        attrs.since.map(|(_, s)| s).unwrap_or(vspan),
                        format!("variant `{vname}` has since = {since_v}, outside 1..={version}"),
                    ));
                }
                let (removed, migrate_check) = removal(
                    &vname,
                    vspan,
                    version,
                    since_v,
                    attrs.removed_in,
                    &attrs.migrate,
                    "variant",
                )?;
                checks.extend(migrate_check);
                let payload = match &v.fields {
                    Fields::Unit => quote!(::core::option::Option::None),
                    Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                        let ty = type_ref(&u.unnamed[0].ty)?;
                        quote!(::core::option::Option::Some(#ty))
                    }
                    _ => {
                        return Err(syn::Error::new(
                            vspan,
                            "only unit and single-field tuple variants are supported",
                        ))
                    }
                };
                let migrate = opt_str(attrs.migrate.as_ref().map(|(m, _)| m.as_str()));
                variants.push(quote! {
                    #p::Variant {
                        name: #vname,
                        since: #since_v,
                        removed_in: #removed,
                        migrate: #migrate,
                        payload: #payload,
                    }
                });
            }
            (quote!(#p::TypeDef::Enum(&[#(#variants),*])), checks)
        }
        Data::Union(_) => {
            return Err(syn::Error::new(
                ident.span(),
                "#[derive(Model)] does not support unions",
            ))
        }
    };

    let schema_const = format_ident!("__TPT_SCHEMA_{}", ident);
    let schema_static = format_ident!("__TPT_SCHEMA_STATIC_{}", ident);
    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        const #schema_const: #p::Schema = #p::Schema {
            name: #name,
            version: #version,
            def: #body,
        };

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        static #schema_static: #p::Schema = #schema_const;

        impl ::tpt_model::Model for #ident {
            const SCHEMA_ID: ::tpt_model::SchemaId = ::tpt_model::SchemaId::of(&#schema_const);
            const SCHEMA_VERSION: u32 = #version;

            fn schema() -> &'static #p::Schema {
                &#schema_static
            }
        }

        #(#checks)*
    })
}

/// Emits `Option<Access>` for a field.
fn access_tokens(attrs: &Attrs, fname: &str) -> syn::Result<TokenStream2> {
    let p = private_path();
    if attrs.intent.is_none() && attrs.capability.is_none() {
        return Ok(quote!(::core::option::Option::None));
    }
    let intent = match &attrs.intent {
        None => quote!(#p::Intent::Read),
        Some((s, span)) => match s.as_str() {
            "read" => quote!(#p::Intent::Read),
            "write" => quote!(#p::Intent::Write),
            other => {
                return Err(syn::Error::new(
                    *span,
                    format!(
                        "field `{fname}`: intent must be \"read\" or \"write\", not \"{other}\""
                    ),
                ))
            }
        },
    };
    let capability = opt_str(attrs.capability.as_ref().map(|(c, _)| c.as_str()));
    Ok(
        quote!(::core::option::Option::Some(#p::Access { intent: #intent, capability: #capability })),
    )
}

/// Parses an integer attribute value, accepting a leading minus sign.
fn parse_i64(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<i64> {
    let expr: syn::Expr = meta.value()?.parse()?;
    let (negative, lit) = match &expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(i),
            ..
        }) => (false, i),
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => match &**expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(i),
                ..
            }) => (true, i),
            _ => return Err(syn::Error::new(expr.span(), "expected an integer literal")),
        },
        _ => return Err(syn::Error::new(expr.span(), "expected an integer literal")),
    };
    let magnitude: i64 = lit.base10_parse()?;
    Ok(if negative { -magnitude } else { magnitude })
}

/// Emits the `&[Constraint]` list for a field.
fn constraint_tokens(attrs: &Attrs) -> TokenStream2 {
    let p = private_path();
    let mut items = Vec::new();
    if let Some((v, _)) = attrs.min {
        items.push(quote!(#p::Constraint::Min(#v)));
    }
    if let Some((v, _)) = attrs.max {
        items.push(quote!(#p::Constraint::Max(#v)));
    }
    quote!(&[#(#items),*])
}
