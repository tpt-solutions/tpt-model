//! The meta-model: schema graph types.
//!
//! Every type is described by a [`Schema`] value. Schemas are plain `const` data
//! so the derive macro can emit them as statics without allocating.

use crate::hash::{
    absorb_bytes, absorb_lanes, absorb_opt_str, absorb_opt_u32, absorb_str, absorb_u64, combine,
    seed, Lanes,
};
use alloc::format;
use alloc::string::{String, ToString};
use tpt_primitives_stub::Intent;

/// Primitive leaf types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    Bool,
    I64,
    U64,
    F64,
    String,
    Bytes,
    /// A 16-byte UUIDv7 (see [`crate::entity::EntityId`]).
    Uuid,
}

impl Prim {
    pub const fn name(self) -> &'static str {
        match self {
            Prim::Bool => "bool",
            Prim::I64 => "i64",
            Prim::U64 => "u64",
            Prim::F64 => "f64",
            Prim::String => "string",
            Prim::Bytes => "bytes",
            Prim::Uuid => "uuid",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Prim::Bool => 1,
            Prim::I64 => 2,
            Prim::U64 => 3,
            Prim::F64 => 4,
            Prim::String => 5,
            Prim::Bytes => 6,
            Prim::Uuid => 7,
        }
    }
}

/// A reference to a type from inside a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRef {
    Prim(Prim),
    /// A named schema, resolved through the registry by name (latest version).
    Named(&'static str),
    Option(&'static TypeRef),
    /// Homogeneous sequence.
    Array(&'static TypeRef),
    /// String-keyed map with values of the given type.
    Map(&'static TypeRef),
}

impl TypeRef {
    /// Human-readable form, used by exporters and error messages.
    pub fn describe(&self) -> String {
        match self {
            TypeRef::Prim(p) => p.name().to_string(),
            TypeRef::Named(n) => (*n).to_string(),
            TypeRef::Option(t) => format!("option<{}>", t.describe()),
            TypeRef::Array(t) => format!("array<{}>", t.describe()),
            TypeRef::Map(t) => format!("map<string, {}>", t.describe()),
        }
    }
}

/// Field-level access rule (see [`crate::access`]).
///
/// An operation of at least `intent` on the field needs `capability` (when set).
/// `Intent::Read` therefore guards reads and writes; `Intent::Write` guards writes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    pub intent: Intent,
    pub capability: Option<&'static str>,
}

/// A struct field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// Current (wire) name.
    pub name: &'static str,
    pub ty: TypeRef,
    /// Schema version in which the field was added.
    pub since: u32,
    /// Version in which the field is removed. Set on the struct version that still has it.
    pub removed_in: Option<u32>,
    /// Default expression, evaluated by [`crate::default_expr`] when a field is added.
    pub default: Option<&'static str>,
    /// Former names of this field, used for automatic renames.
    pub renames: &'static [&'static str],
    pub access: Option<Access>,
    /// Name of the migration function that handles this field's removal.
    pub migrate: Option<&'static str>,
    /// Value constraints, checked when a tree is validated against the schema.
    pub constraints: &'static [Constraint],
}

/// A bound on a field's value.
///
/// For numbers the bound applies to the value. For strings, bytes and sequences it applies to
/// the length (characters for strings, bytes for `Bytes`, elements for sequences).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    Min(i64),
    Max(i64),
}

impl core::fmt::Display for Constraint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Constraint::Min(v) => write!(f, "min {v}"),
            Constraint::Max(v) => write!(f, "max {v}"),
        }
    }
}

/// An enum variant. Only unit and newtype variants are supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variant {
    pub name: &'static str,
    pub since: u32,
    pub removed_in: Option<u32>,
    pub migrate: Option<&'static str>,
    pub payload: Option<TypeRef>,
}

/// The body of a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDef {
    Struct(&'static [Field]),
    Enum(&'static [Variant]),
    /// Untagged union: a value matches when it matches any member.
    Union(&'static [TypeRef]),
}

/// A versioned type description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schema {
    /// Logical type name, shared by all versions (`CadPart` for `CadPartV1`, `CadPartV2`).
    pub name: &'static str,
    pub version: u32,
    pub def: TypeDef,
}

/// A structural problem found by [`Schema::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub schema: &'static str,
    pub message: String,
}

impl core::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "schema {}: {}", self.schema, self.message)
    }
}

impl Schema {
    /// Looks up a struct field by its current name.
    pub fn field(&self, name: &str) -> Option<&'static Field> {
        match self.def {
            TypeDef::Struct(fields) => fields.iter().find(|f| f.name == name),
            _ => None,
        }
    }

    /// Looks up an enum variant by name.
    pub fn variant(&self, name: &str) -> Option<&'static Variant> {
        match self.def {
            TypeDef::Enum(vs) => vs.iter().find(|v| v.name == name),
            _ => None,
        }
    }

    /// Checks the schema on its own: unique names, sane version ranges, non-empty unions.
    ///
    /// Cross-type checks (dangling references, cycles) are in [`crate::registry::Registry::check_graph`].
    pub fn validate(&self) -> Result<(), SchemaError> {
        let err = |message: String| SchemaError {
            schema: self.name,
            message,
        };
        if self.name.is_empty() {
            return Err(err("empty name".to_string()));
        }
        if self.version == 0 {
            return Err(err("version must be at least 1".to_string()));
        }
        match self.def {
            TypeDef::Struct(fields) => {
                for (i, f) in fields.iter().enumerate() {
                    if fields[..i].iter().any(|g| g.name == f.name) {
                        return Err(err(format!("duplicate field `{}`", f.name)));
                    }
                    for old in f.renames {
                        if fields.iter().any(|g| g.name == *old) {
                            return Err(err(format!(
                                "field `{}` was renamed from `{}`, which is still a field",
                                f.name, old
                            )));
                        }
                    }
                    if f.since == 0 || f.since > self.version {
                        return Err(err(format!(
                            "field `{}` has since = {} outside 1..={}",
                            f.name, f.since, self.version
                        )));
                    }
                    if let Some(r) = f.removed_in {
                        if r <= self.version || r <= f.since {
                            return Err(err(format!(
                                "field `{}` has removed_in = {} which must be greater than version {} and since {}",
                                f.name, r, self.version, f.since
                            )));
                        }
                    }
                    let mut min: Option<i64> = None;
                    let mut max: Option<i64> = None;
                    for c in f.constraints {
                        match *c {
                            Constraint::Min(v) => min = Some(v),
                            Constraint::Max(v) => max = Some(v),
                        }
                    }
                    if let (Some(lo), Some(hi)) = (min, max) {
                        if lo > hi {
                            return Err(err(format!(
                                "field `{}` has min {lo} above max {hi}",
                                f.name
                            )));
                        }
                    }
                }
            }
            TypeDef::Enum(variants) => {
                for (i, v) in variants.iter().enumerate() {
                    if variants[..i].iter().any(|w| w.name == v.name) {
                        return Err(err(format!("duplicate variant `{}`", v.name)));
                    }
                    if v.since == 0 || v.since > self.version {
                        return Err(err(format!(
                            "variant `{}` has since = {} outside 1..={}",
                            v.name, v.since, self.version
                        )));
                    }
                    if let Some(r) = v.removed_in {
                        if r <= self.version || r <= v.since {
                            return Err(err(format!(
                                "variant `{}` has an invalid removed_in = {}",
                                v.name, r
                            )));
                        }
                    }
                }
            }
            TypeDef::Union(members) => {
                if members.is_empty() {
                    return Err(err("union has no members".to_string()));
                }
            }
        }
        Ok(())
    }
}

// ---- const digest ---------------------------------------------------------

const fn absorb_type(s: Lanes, t: &TypeRef) -> Lanes {
    match t {
        TypeRef::Prim(p) => absorb_bytes(s, &[1, p.tag()]),
        TypeRef::Named(n) => absorb_str(absorb_bytes(s, &[2]), n),
        TypeRef::Option(inner) => absorb_type(absorb_bytes(s, &[3]), inner),
        TypeRef::Array(inner) => absorb_type(absorb_bytes(s, &[4]), inner),
        TypeRef::Map(inner) => absorb_type(absorb_bytes(s, &[5]), inner),
    }
}

const fn absorb_opt_type(s: Lanes, t: Option<TypeRef>) -> Lanes {
    match t {
        None => absorb_bytes(s, &[0]),
        Some(t) => absorb_type(absorb_bytes(s, &[1]), &t),
    }
}

const fn absorb_strs(s: Lanes, items: &[&str]) -> Lanes {
    let mut s = absorb_u64(s, items.len() as u64);
    let mut i = 0;
    while i < items.len() {
        s = absorb_str(s, items[i]);
        i += 1;
    }
    s
}

const fn field_digest(f: &Field) -> Lanes {
    let s = seed(0x31);
    let s = absorb_str(s, f.name);
    let s = absorb_type(s, &f.ty);
    let s = absorb_u64(s, f.since as u64);
    let s = absorb_opt_u32(s, f.removed_in);
    let s = absorb_opt_str(s, f.default);
    let s = absorb_strs(s, f.renames);
    let s = match f.access {
        None => absorb_bytes(s, &[0]),
        Some(a) => {
            let intent = match a.intent {
                Intent::Read => 0u8,
                Intent::Write => 1u8,
            };
            absorb_opt_str(absorb_bytes(s, &[1, intent]), a.capability)
        }
    };
    let s = absorb_opt_str(s, f.migrate);
    absorb_constraints(s, f.constraints)
}

const fn absorb_constraints(s: Lanes, cs: &[Constraint]) -> Lanes {
    let mut s = absorb_u64(s, cs.len() as u64);
    let mut i = 0;
    while i < cs.len() {
        s = match cs[i] {
            Constraint::Min(v) => absorb_u64(absorb_bytes(s, &[1]), v as u64),
            Constraint::Max(v) => absorb_u64(absorb_bytes(s, &[2]), v as u64),
        };
        i += 1;
    }
    s
}

const fn variant_digest(v: &Variant) -> Lanes {
    let s = seed(0x32);
    let s = absorb_str(s, v.name);
    let s = absorb_u64(s, v.since as u64);
    let s = absorb_opt_u32(s, v.removed_in);
    let s = absorb_opt_str(s, v.migrate);
    absorb_opt_type(s, v.payload)
}

/// Order-normalized digest of a struct body: fields combine commutatively.
const fn fields_digest(fields: &[Field]) -> Lanes {
    let mut acc: Lanes = [0; 4];
    let mut i = 0;
    while i < fields.len() {
        acc = combine(acc, field_digest(&fields[i]));
        i += 1;
    }
    absorb_u64(absorb_lanes(seed(0x33), acc), fields.len() as u64)
}

const fn variants_digest(variants: &[Variant]) -> Lanes {
    let mut acc: Lanes = [0; 4];
    let mut i = 0;
    while i < variants.len() {
        acc = combine(acc, variant_digest(&variants[i]));
        i += 1;
    }
    absorb_u64(absorb_lanes(seed(0x34), acc), variants.len() as u64)
}

const fn union_digest(members: &[TypeRef]) -> Lanes {
    let mut acc: Lanes = [0; 4];
    let mut i = 0;
    while i < members.len() {
        acc = combine(acc, absorb_type(seed(0x35), &members[i]));
        i += 1;
    }
    absorb_u64(absorb_lanes(seed(0x36), acc), members.len() as u64)
}

/// Digest of a whole schema, used for [`crate::SchemaId`].
pub const fn schema_digest(schema: &Schema) -> Lanes {
    let s = seed(0x30);
    let s = absorb_str(s, schema.name);
    let s = absorb_u64(s, schema.version as u64);
    match schema.def {
        TypeDef::Struct(fields) => absorb_lanes(absorb_bytes(s, &[1]), fields_digest(fields)),
        TypeDef::Enum(variants) => absorb_lanes(absorb_bytes(s, &[2]), variants_digest(variants)),
        TypeDef::Union(members) => absorb_lanes(absorb_bytes(s, &[3]), union_digest(members)),
    }
}
