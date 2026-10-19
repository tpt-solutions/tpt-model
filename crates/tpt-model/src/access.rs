//! Field-level access control, driven by `#[tpt(intent = "...", capability = "...")]`.
//!
//! A field with `Access { intent, capability: Some(c) }` requires the caller to hold `c`
//! for any operation at or above `intent`. `read` guards reads and writes; `write`
//! guards writes only. Fields without an access rule are always allowed.

use crate::schema::{Schema, TypeDef};
use alloc::string::String;
use alloc::vec::Vec;
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

/// Removes top-level fields the caller may not `operation`. Used to redact a struct
/// tree before it leaves a trust boundary. Non-map nodes are returned unchanged.
pub fn redact(
    schema: &Schema,
    node: &UirNode,
    operation: Intent,
    granted: &[Capability],
) -> UirNode {
    let TypeDef::Struct(_) = schema.def else {
        return node.clone();
    };
    match node {
        UirNode::Map(entries) => {
            let kept: Vec<(String, UirNode)> = entries
                .iter()
                .filter(|(key, _)| check(schema, key, operation, granted).is_ok())
                .cloned()
                .collect();
            UirNode::Map(kept)
        }
        other => other.clone(),
    }
}
