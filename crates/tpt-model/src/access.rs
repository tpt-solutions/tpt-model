//! Field-level access control, driven by `#[tpt(intent = "...", capability = "...")]`.
//!
//! A field with `Access { intent, capability: Some(c) }` requires the caller to hold `c`
//! for any operation at or above `intent`. `read` guards reads and writes; `write`
//! guards writes only. Fields without an access rule are always allowed.

use crate::registry::Registry;
use crate::schema::{Field, Schema, TypeDef, TypeRef};
use alloc::string::String;
use core::fmt;
use tpt_primitives_stub::{is_granted, Capability, Intent};
use tpt_uir_stub::UirNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    UnknownField {
        field: String,
    },
    Denied {
        field: &'static str,
        operation: Intent,
        capability: &'static str,
    },
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessError::UnknownField { field } => write!(f, "no field `{field}`"),
            AccessError::Denied {
                field,
                operation,
                capability,
            } => {
                write!(
                    f,
                    "{operation} of `{field}` requires capability `{capability}`"
                )
            }
        }
    }
}

/// Checks whether `granted` allows `operation` on `field`.
pub fn check(
    schema: &Schema,
    field: &str,
    operation: Intent,
    granted: &[Capability],
) -> Result<(), AccessError> {
    let f = schema
        .field(field)
        .ok_or_else(|| AccessError::UnknownField {
            field: field.into(),
        })?;
    match f.access {
        Some(access) if operation >= access.intent => {
            let required = access.capability.map(Capability);
            if is_granted(required, granted) {
                Ok(())
            } else {
                Err(AccessError::Denied {
                    field: f.name,
                    operation,
                    capability: access.capability.unwrap_or(""),
                })
            }
        }
        _ => Ok(()),
    }
}

/// Removes every field, at any depth, that the caller may not `operation`. Used to redact a
/// tree before it leaves a trust boundary.
///
/// Nested struct values are redacted against their own fields, using the version of the
/// nested type that applies at the containing schema's version. Enum payloads are walked
/// the same way. A nested type the registry cannot resolve is replaced by `null`, so that
/// its contents are not exposed. Unknown keys are dropped.
pub fn redact(
    registry: &Registry,
    schema: &Schema,
    node: &UirNode,
    operation: Intent,
    granted: &[Capability],
) -> UirNode {
    match schema.def {
        TypeDef::Struct(fields) => match node {
            UirNode::Map(entries) => UirNode::Map(
                entries
                    .iter()
                    .filter_map(|(key, value)| {
                        let field = fields.iter().find(|f| f.name == key)?;
                        if !allowed(field, operation, granted) {
                            return None;
                        }
                        let value = redact_type(
                            registry,
                            &field.ty,
                            value,
                            schema.version,
                            operation,
                            granted,
                        );
                        Some((key.clone(), value))
                    })
                    .collect(),
            ),
            other => other.clone(),
        },
        TypeDef::Enum(variants) => match node {
            UirNode::Map(entries) if entries.len() == 1 => {
                let (name, value) = &entries[0];
                match variants
                    .iter()
                    .find(|v| v.name == name)
                    .and_then(|v| v.payload.as_ref())
                {
                    Some(ty) => UirNode::Map(alloc::vec![(
                        name.clone(),
                        redact_type(registry, ty, value, schema.version, operation, granted),
                    )]),
                    None => node.clone(),
                }
            }
            other => other.clone(),
        },
        // A union's member is not known from the value alone, so it is passed through.
        TypeDef::Union(_) => node.clone(),
    }
}

/// Whether the field may be used for `operation` by a caller holding `granted`.
fn allowed(field: &Field, operation: Intent, granted: &[Capability]) -> bool {
    match field.access {
        Some(access) if operation >= access.intent => {
            is_granted(access.capability.map(Capability), granted)
        }
        _ => true,
    }
}

fn redact_type(
    registry: &Registry,
    ty: &TypeRef,
    node: &UirNode,
    at: u32,
    operation: Intent,
    granted: &[Capability],
) -> UirNode {
    match ty {
        TypeRef::Prim(_) => node.clone(),
        TypeRef::Named(name) => match registry.at_or_before(name, at) {
            Some(schema) => redact(registry, schema, node, operation, granted),
            None => UirNode::Null,
        },
        TypeRef::Option(inner) => match node {
            UirNode::Null => UirNode::Null,
            other => redact_type(registry, inner, other, at, operation, granted),
        },
        TypeRef::Array(inner) => match node {
            UirNode::Seq(items) => UirNode::Seq(
                items
                    .iter()
                    .map(|item| redact_type(registry, inner, item, at, operation, granted))
                    .collect(),
            ),
            other => other.clone(),
        },
        TypeRef::Map(inner) => match node {
            UirNode::Map(entries) => UirNode::Map(
                entries
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            redact_type(registry, inner, v, at, operation, granted),
                        )
                    })
                    .collect(),
            ),
            other => other.clone(),
        },
    }
}
