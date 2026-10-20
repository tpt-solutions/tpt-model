//! Breaking-change lint over two registries: typically the code being released and the
//! previous release.
//!
//! It reports changes that would leave data written under the old registry unreadable by the
//! new one, without a migration to handle them. Migrations are not consulted: a removed field
//! is reported even if a migration is planned, because the new registry no longer describes it.
//! Changes that a migration covers should be read as "needs a migration", not as silent breaks.

use crate::hash::SchemaId;
use crate::registry::Registry;
use crate::schema::{Field, Prim, Schema, TypeDef, TypeRef};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// One change that breaks data written under the old registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Breaking {
    /// A logical type exists in the old registry and not in the new one.
    TypeRemoved { name: &'static str },
    /// A version exists in both registries, with a different structure. Old data carries the
    /// old `SchemaId`, which the new registry no longer knows.
    ChangedWithoutBump { name: &'static str, version: u32 },
    /// A field of the latest version is gone, and no field takes over its name as a rename.
    FieldDropped {
        name: &'static str,
        field: &'static str,
    },
    /// A field of the new latest version has no value in old data: no default and not optional.
    FieldAddedWithoutDefault {
        name: &'static str,
        field: &'static str,
    },
    /// A field changed type in a way that is not widening.
    FieldTypeChanged {
        name: &'static str,
        field: &'static str,
        from: String,
        to: String,
    },
    /// An enum variant of the latest version is gone.
    VariantRemoved {
        name: &'static str,
        variant: &'static str,
    },
    /// The latest version changed from a struct to an enum, or the other way round.
    KindChanged { name: &'static str },
}

impl fmt::Display for Breaking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Breaking::TypeRemoved { name } => write!(f, "{name}: type removed"),
            Breaking::ChangedWithoutBump { name, version } => write!(
                f,
                "{name} v{version}: structure changed without a version bump (old data cannot be read)"
            ),
            Breaking::FieldDropped { name, field } => {
                write!(f, "{name}: field `{field}` removed without a migration")
            }
            Breaking::FieldAddedWithoutDefault { name, field } => write!(
                f,
                "{name}: field `{field}` added without a default or Option type"
            ),
            Breaking::FieldTypeChanged {
                name,
                field,
                from,
                to,
            } => write!(f, "{name}: field `{field}` changed type from {from} to {to}"),
            Breaking::VariantRemoved { name, variant } => {
                write!(f, "{name}: variant `{variant}` removed")
            }
            Breaking::KindChanged { name } => {
                write!(f, "{name}: changed between struct and enum")
            }
        }
    }
}

/// Compares `old` and `new` and returns every breaking change, in a deterministic order.
pub fn breaking_changes(old: &Registry, new: &Registry) -> Vec<Breaking> {
    let mut out = Vec::new();
    let mut names: Vec<&'static str> = old.iter().map(|s| s.name).collect();
    names.sort_unstable();
    names.dedup();

    for name in names {
        if new.versions(name).is_empty() {
            out.push(Breaking::TypeRemoved { name });
            continue;
        }

        // Every version the two registries share must have the same structure.
        for version in old.versions(name) {
            let (Some(before), Some(after)) = (
                old.by_name_version(name, version),
                new.by_name_version(name, version),
            ) else {
                continue;
            };
            if SchemaId::of(before) != SchemaId::of(after) {
                out.push(Breaking::ChangedWithoutBump { name, version });
            }
        }

        // The latest versions decide what the new code can read from old data.
        if let (Some(before), Some(after)) = (old.latest(name), new.latest(name)) {
            compare_latest(before, after, &mut out);
        }
    }
    out
}

fn compare_latest(before: &Schema, after: &Schema, out: &mut Vec<Breaking>) {
    let name = after.name;
    match (before.def, after.def) {
        (TypeDef::Struct(old_fields), TypeDef::Struct(new_fields)) => {
            compare_fields(name, old_fields, new_fields, out);
        }
        (TypeDef::Enum(old_variants), TypeDef::Enum(new_variants)) => {
            for v in old_variants {
                if !new_variants.iter().any(|n| n.name == v.name) {
                    out.push(Breaking::VariantRemoved {
                        name,
                        variant: v.name,
                    });
                }
            }
        }
        _ => out.push(Breaking::KindChanged { name }),
    }
}

fn compare_fields(
    name: &'static str,
    old_fields: &[Field],
    new_fields: &[Field],
    out: &mut Vec<Breaking>,
) {
    // A new field name that appears in an old version's rename history is a rename, not an add.
    let old_name_of = |new: &Field| {
        old_fields
            .iter()
            .find(|o| o.name == new.name || new.renames.contains(&o.name))
    };

    for old in old_fields {
        let found = new_fields
            .iter()
            .find(|n| n.name == old.name || n.renames.contains(&old.name));
        match found {
            None => out.push(Breaking::FieldDropped {
                name,
                field: old.name,
            }),
            Some(new) => {
                if !type_compatible(&old.ty, &new.ty) {
                    out.push(Breaking::FieldTypeChanged {
                        name,
                        field: new.name,
                        from: old.ty.describe(),
                        to: new.ty.describe(),
                    });
                }
            }
        }
    }

    for new in new_fields {
        if old_name_of(new).is_some() {
            continue;
        }
        let optional = matches!(new.ty, TypeRef::Option(_));
        if new.default.is_none() && !optional {
            out.push(Breaking::FieldAddedWithoutDefault {
                name,
                field: new.name,
            });
        }
    }
}

/// Whether data of type `old` can be read as `new`: equal types, or a widening primitive.
fn type_compatible(old: &TypeRef, new: &TypeRef) -> bool {
    match (old, new) {
        (TypeRef::Prim(a), TypeRef::Prim(b)) => {
            a == b || matches!((a, b), (Prim::I64 | Prim::U64, Prim::F64))
        }
        (TypeRef::Option(a), TypeRef::Option(b))
        | (TypeRef::Array(a), TypeRef::Array(b))
        | (TypeRef::Map(a), TypeRef::Map(b)) => type_compatible(a, b),
        (TypeRef::Named(a), TypeRef::Named(b)) => a == b,
        _ => false,
    }
}

/// Human-readable report, one line per change, or a single line when there are none.
pub fn report(changes: &[Breaking]) -> String {
    if changes.is_empty() {
        return String::from("no breaking changes\n");
    }
    let mut out = String::new();
    for c in changes {
        out.push_str(&format!("breaking: {c}\n"));
    }
    out
}
