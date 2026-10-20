//! JSON and YAML export of schemas, for `tpt-validex` and golden-file tests.
//!
//! Output is deterministic: keys appear in a fixed order and nothing depends on the
//! host, so exports can be compared byte-for-byte.

use crate::registry::Registry;
use crate::schema::{Constraint, Field, Prim, Schema, TypeDef, TypeRef, Variant};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Format-neutral value tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(&'static str, Value)>),
}

fn opt_u32(v: Option<u32>) -> Value {
    match v {
        Some(n) => Value::Int(n as i64),
        None => Value::Null,
    }
}

fn opt_str(v: Option<&str>) -> Value {
    match v {
        Some(s) => Value::Str(s.to_string()),
        None => Value::Null,
    }
}

fn type_value(t: &TypeRef) -> Value {
    Value::Str(t.describe())
}

fn field_value(f: &Field) -> Value {
    let access = match f.access {
        Some(a) => Value::Map(alloc::vec![
            ("intent", Value::Str(a.intent.to_string())),
            ("capability", opt_str(a.capability)),
        ]),
        None => Value::Null,
    };
    Value::Map(alloc::vec![
        ("name", Value::Str(f.name.to_string())),
        ("type", type_value(&f.ty)),
        ("since", Value::Int(f.since as i64)),
        ("removed_in", opt_u32(f.removed_in)),
        ("default", opt_str(f.default)),
        (
            "renames",
            Value::List(
                f.renames
                    .iter()
                    .map(|r| Value::Str(r.to_string()))
                    .collect()
            )
        ),
        ("access", access),
        ("migrate", opt_str(f.migrate)),
        (
            "constraints",
            Value::List(
                f.constraints
                    .iter()
                    .map(|c| Value::Str(c.to_string()))
                    .collect()
            )
        ),
    ])
}

fn variant_value(v: &Variant) -> Value {
    Value::Map(alloc::vec![
        ("name", Value::Str(v.name.to_string())),
        ("since", Value::Int(v.since as i64)),
        ("removed_in", opt_u32(v.removed_in)),
        ("migrate", opt_str(v.migrate)),
        (
            "payload",
            match &v.payload {
                Some(t) => type_value(t),
                None => Value::Null,
            }
        ),
    ])
}

/// Converts a schema to a value tree.
pub fn schema_value(schema: &Schema) -> Value {
    let (kind, body) = match schema.def {
        TypeDef::Struct(fields) => (
            "struct",
            Value::List(fields.iter().map(field_value).collect()),
        ),
        TypeDef::Enum(variants) => (
            "enum",
            Value::List(variants.iter().map(variant_value).collect()),
        ),
        TypeDef::Union(members) => (
            "union",
            Value::List(members.iter().map(type_value).collect()),
        ),
    };
    let body_key = match schema.def {
        TypeDef::Struct(_) => "fields",
        TypeDef::Enum(_) => "variants",
        TypeDef::Union(_) => "members",
    };
    Value::Map(alloc::vec![
        ("name", Value::Str(schema.name.to_string())),
        ("version", Value::Int(schema.version as i64)),
        (
            "id",
            Value::Str(crate::hash::SchemaId::of(schema).to_string())
        ),
        ("kind", Value::Str(kind.to_string())),
        (body_key, body),
    ])
}

/// Converts every schema in a registry, sorted by name then version.
pub fn registry_value(registry: &Registry) -> Value {
    let mut schemas: Vec<&Schema> = registry.iter().collect();
    schemas.sort_by(|a, b| a.name.cmp(b.name).then(a.version.cmp(&b.version)));
    Value::Map(alloc::vec![(
        "schemas",
        Value::List(schemas.into_iter().map(schema_value).collect()),
    )])
}

/// Pretty JSON with two-space indentation.
pub fn to_json(value: &Value) -> String {
    let mut out = String::new();
    json_into(value, 0, &mut out);
    out.push('\n');
    out
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_into(value: &Value, indent: usize, out: &mut String) {
    let pad = |n: usize| " ".repeat(n);
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Str(s) => json_string(s, out),
        Value::List(items) if items.is_empty() => out.push_str("[]"),
        Value::List(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad(indent + 2));
                json_into(item, indent + 2, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push(']');
        }
        Value::Map(entries) if entries.is_empty() => out.push_str("{}"),
        Value::Map(entries) => {
            out.push_str("{\n");
            for (i, (k, v)) in entries.iter().enumerate() {
                out.push_str(&pad(indent + 2));
                json_string(k, out);
                out.push_str(": ");
                json_into(v, indent + 2, out);
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push('}');
        }
    }
}

/// Block-style YAML. Strings are always double-quoted, which avoids YAML's implicit typing.
pub fn to_yaml(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::List(items) if !items.is_empty() => yaml_block(value, 0, &mut out),
        Value::Map(entries) if !entries.is_empty() => yaml_block(value, 0, &mut out),
        other => {
            yaml_scalar(other, &mut out);
            out.push('\n');
        }
    }
    out
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::List(_) | Value::Map(_))
}

fn yaml_scalar(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Str(s) => json_string(s, out),
        Value::List(_) => out.push_str("[]"),
        Value::Map(_) => out.push_str("{}"),
    }
}

fn yaml_block(value: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Map(entries) => {
            for (k, v) in entries {
                out.push_str(&pad);
                out.push_str(k);
                out.push(':');
                if is_scalar(v)
                    || matches!(v, Value::List(l) if l.is_empty())
                    || matches!(v, Value::Map(m) if m.is_empty())
                {
                    out.push(' ');
                    yaml_scalar(v, out);
                    out.push('\n');
                } else {
                    out.push('\n');
                    yaml_block(v, indent + 2, out);
                }
            }
        }
        Value::List(items) => {
            for item in items {
                out.push_str(&pad);
                out.push('-');
                if is_scalar(item)
                    || matches!(item, Value::List(l) if l.is_empty())
                    || matches!(item, Value::Map(m) if m.is_empty())
                {
                    out.push(' ');
                    yaml_scalar(item, out);
                    out.push('\n');
                } else {
                    out.push('\n');
                    yaml_block(item, indent + 2, out);
                }
            }
        }
        other => {
            out.push_str(&pad);
            yaml_scalar(other, out);
            out.push('\n');
        }
    }
}

/// TypeScript declarations for every logical type in `registry`, using each type's latest
/// version. Enum and byte encodings follow serde's JSON form (externally tagged variants, bytes
/// as arrays of numbers).
///
/// `i64` and `u64` map to `number`, which is exact only up to 2^53.
pub fn typescript(registry: &Registry) -> String {
    let mut names: Vec<&'static str> = registry.iter().map(|s| s.name).collect();
    names.sort_unstable();
    names.dedup();

    let mut out = String::from("// Generated by tpt-model. Do not edit.\n");
    for name in names {
        let Some(schema) = registry.latest(name) else {
            continue;
        };
        out.push('\n');
        match schema.def {
            TypeDef::Struct(fields) => {
                out.push_str(&format!("export interface {name} {{\n"));
                for f in fields.iter().filter(|f| f.removed_in.is_none()) {
                    out.push_str(&format!("  {}: {};\n", f.name, ts_type(&f.ty)));
                }
                out.push_str("}\n");
            }
            TypeDef::Enum(variants) => {
                let arms: Vec<String> = variants
                    .iter()
                    .filter(|v| v.removed_in.is_none())
                    .map(|v| match &v.payload {
                        None => format!("\"{}\"", v.name),
                        Some(ty) => format!("{{ {}: {} }}", v.name, ts_type(ty)),
                    })
                    .collect();
                let body = if arms.is_empty() {
                    "never".to_string()
                } else {
                    arms.join(" | ")
                };
                out.push_str(&format!("export type {name} = {body};\n"));
            }
            TypeDef::Union(members) => {
                let arms: Vec<String> = members.iter().map(ts_type).collect();
                let body = if arms.is_empty() {
                    "never".to_string()
                } else {
                    arms.join(" | ")
                };
                out.push_str(&format!("export type {name} = {body};\n"));
            }
        }
    }
    out
}

fn ts_type(t: &TypeRef) -> String {
    match t {
        TypeRef::Prim(p) => match p {
            Prim::Bool => "boolean",
            Prim::I64 | Prim::U64 | Prim::F64 => "number",
            Prim::String => "string",
            // Bytes and UUIDs serialize as arrays of byte values.
            Prim::Bytes | Prim::Uuid => "number[]",
        }
        .to_string(),
        TypeRef::Named(n) => (*n).to_string(),
        TypeRef::Option(inner) => format!("{} | null", ts_type(inner)),
        TypeRef::Array(inner) => format!("{}[]", ts_type(inner)),
        TypeRef::Map(inner) => format!("Record<string, {}>", ts_type(inner)),
    }
}

/// JSON Schema (draft 2020-12) for every logical type in `registry`, using each type's latest
/// version. Types are placed under `$defs` and referenced by name. Structs are closed
/// (`additionalProperties: false`), and fields that are not `Option` are required.
///
/// Matches serde's JSON form: unit enum variants are strings, other variants are one-key
/// objects, and bytes and UUIDs are arrays of byte values.
pub fn json_schema(registry: &Registry) -> String {
    let mut names: Vec<&'static str> = registry.iter().map(|s| s.name).collect();
    names.sort_unstable();
    names.dedup();

    let mut defs: Vec<(&'static str, Value)> = Vec::new();
    for name in names {
        let Some(schema) = registry.latest(name) else {
            continue;
        };
        defs.push((name, json_def(schema)));
    }
    let root = Value::Map(alloc::vec![
        (
            "$schema",
            Value::Str("https://json-schema.org/draft/2020-12/schema".to_string())
        ),
        ("$defs", Value::Map(defs)),
    ]);
    to_json(&root)
}

fn json_def(schema: &Schema) -> Value {
    match schema.def {
        TypeDef::Struct(fields) => {
            let live: Vec<&Field> = fields.iter().filter(|f| f.removed_in.is_none()).collect();
            let properties: Vec<(&'static str, Value)> =
                live.iter().map(|f| (f.name, json_field(f))).collect();
            let required: Vec<Value> = live
                .iter()
                .filter(|f| !matches!(f.ty, TypeRef::Option(_)))
                .map(|f| Value::Str(f.name.to_string()))
                .collect();
            Value::Map(alloc::vec![
                ("type", Value::Str("object".to_string())),
                ("properties", Value::Map(properties)),
                ("required", Value::List(required)),
                ("additionalProperties", Value::Bool(false)),
            ])
        }
        TypeDef::Enum(variants) => {
            let arms: Vec<Value> = variants
                .iter()
                .filter(|v| v.removed_in.is_none())
                .map(|v| match &v.payload {
                    None => Value::Map(alloc::vec![
                        ("type", Value::Str("string".to_string())),
                        ("const", Value::Str(v.name.to_string())),
                    ]),
                    Some(ty) => Value::Map(alloc::vec![
                        ("type", Value::Str("object".to_string())),
                        (
                            "properties",
                            Value::Map(alloc::vec![(v.name, json_type(ty))])
                        ),
                        (
                            "required",
                            Value::List(alloc::vec![Value::Str(v.name.to_string())])
                        ),
                        ("additionalProperties", Value::Bool(false)),
                    ]),
                })
                .collect();
            Value::Map(alloc::vec![("oneOf", Value::List(arms))])
        }
        TypeDef::Union(members) => Value::Map(alloc::vec![(
            "oneOf",
            Value::List(members.iter().map(json_type).collect())
        )]),
    }
}

/// A field's schema: its type, plus any `min`/`max` constraint that applies to that type.
fn json_field(field: &Field) -> Value {
    let mut entries = match json_type(&field.ty) {
        Value::Map(entries) => entries,
        other => return other,
    };
    for c in field.constraints {
        if let Some(key) = bound_key(&field.ty, c) {
            let value = match c {
                Constraint::Min(v) | Constraint::Max(v) => *v,
            };
            entries.push((key, Value::Int(value)));
        }
    }
    Value::Map(entries)
}

/// The JSON Schema keyword for a bound on a field of type `ty`, if the bound applies to it.
fn bound_key(ty: &TypeRef, c: &Constraint) -> Option<&'static str> {
    let (min, max) = match ty {
        TypeRef::Prim(Prim::I64 | Prim::U64 | Prim::F64) => ("minimum", "maximum"),
        TypeRef::Prim(Prim::String) => ("minLength", "maxLength"),
        TypeRef::Array(_) => ("minItems", "maxItems"),
        _ => return None,
    };
    Some(match c {
        Constraint::Min(_) => min,
        Constraint::Max(_) => max,
    })
}

fn json_type(ty: &TypeRef) -> Value {
    let typed = |t: &str| Value::Map(alloc::vec![("type", Value::Str(t.to_string()))]);
    let byte_array = |len: Option<i64>| {
        let mut entries = alloc::vec![
            ("type", Value::Str("array".to_string())),
            (
                "items",
                Value::Map(alloc::vec![
                    ("type", Value::Str("integer".to_string())),
                    ("minimum", Value::Int(0)),
                    ("maximum", Value::Int(255)),
                ]),
            ),
        ];
        if let Some(n) = len {
            entries.push(("minItems", Value::Int(n)));
            entries.push(("maxItems", Value::Int(n)));
        }
        Value::Map(entries)
    };
    match ty {
        TypeRef::Prim(p) => match p {
            Prim::Bool => typed("boolean"),
            Prim::I64 | Prim::U64 => typed("integer"),
            Prim::F64 => typed("number"),
            Prim::String => typed("string"),
            Prim::Bytes => byte_array(None),
            Prim::Uuid => byte_array(Some(16)),
        },
        TypeRef::Named(n) => Value::Map(alloc::vec![("$ref", Value::Str(format!("#/$defs/{n}")))]),
        TypeRef::Option(inner) => Value::Map(alloc::vec![(
            "anyOf",
            Value::List(alloc::vec![json_type(inner), typed("null")])
        )]),
        TypeRef::Array(inner) => Value::Map(alloc::vec![
            ("type", Value::Str("array".to_string())),
            ("items", json_type(inner)),
        ]),
        TypeRef::Map(inner) => Value::Map(alloc::vec![
            ("type", Value::Str("object".to_string())),
            ("additionalProperties", json_type(inner)),
        ]),
    }
}
