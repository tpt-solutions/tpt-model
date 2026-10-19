//! Checks an IR tree against a schema.

use crate::registry::Registry;
use crate::schema::{Constraint, Prim, Schema, TypeDef, TypeRef};
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;
use tpt_uir_stub::UirNode;

/// Deepest nesting the shape checker will follow (guards against cyclic data).
const MAX_DEPTH: usize = 64;

/// Why a tree does not match a schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeError {
    TypeMismatch {
        path: String,
        expected: String,
        found: &'static str,
    },
    MissingField {
        path: String,
    },
    UnknownField {
        path: String,
    },
    UnknownVariant {
        path: String,
        variant: String,
    },
    UnknownType {
        name: &'static str,
    },
    TooDeep {
        path: String,
    },
    /// Value fits none of the members of a union.
    NoUnionMatch {
        path: String,
    },
    /// A value is outside its declared min or max.
    Constraint {
        path: String,
        constraint: String,
    },
}

impl fmt::Display for ShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeError::TypeMismatch {
                path,
                expected,
                found,
            } => {
                write!(f, "at `{path}`: expected {expected}, found {found}")
            }
            ShapeError::MissingField { path } => write!(f, "missing field `{path}`"),
            ShapeError::UnknownField { path } => write!(f, "unknown field `{path}`"),
            ShapeError::UnknownVariant { path, variant } => {
                write!(f, "at `{path}`: unknown variant `{variant}`")
            }
            ShapeError::UnknownType { name } => write!(f, "type `{name}` is not registered"),
            ShapeError::TooDeep { path } => write!(f, "nesting too deep at `{path}`"),
            ShapeError::NoUnionMatch { path } => write!(f, "at `{path}`: matches no union member"),
            ShapeError::Constraint { path, constraint } => {
                write!(f, "at `{path}`: violates {constraint}")
            }
        }
    }
}

/// Name of the IR variant, used in mismatch messages.
pub fn kind_of(node: &UirNode) -> &'static str {
    match node {
        UirNode::Null => "null",
        UirNode::Bool(_) => "bool",
        UirNode::Int(_) => "int",
        UirNode::UInt(_) => "uint",
        UirNode::Float(_) => "float",
        UirNode::Str(_) => "string",
        UirNode::Bytes(_) => "bytes",
        UirNode::Seq(_) => "sequence",
        UirNode::Map(_) => "map",
    }
}

/// Validates `node` against `schema`. Named references are resolved to the latest
/// registered version of that name.
pub fn validate_node(
    registry: &Registry,
    schema: &Schema,
    node: &UirNode,
) -> Result<(), ShapeError> {
    validate_schema(registry, schema, node, "$", 0)
}

fn validate_schema(
    registry: &Registry,
    schema: &Schema,
    node: &UirNode,
    path: &str,
    depth: usize,
) -> Result<(), ShapeError> {
    if depth > MAX_DEPTH {
        return Err(ShapeError::TooDeep {
            path: path.to_string(),
        });
    }
    match schema.def {
        TypeDef::Struct(fields) => {
            let entries = match node {
                UirNode::Map(entries) => entries,
                other => {
                    return Err(ShapeError::TypeMismatch {
                        path: path.to_string(),
                        expected: format!("struct {}", schema.name),
                        found: kind_of(other),
                    })
                }
            };
            for (key, _) in entries {
                if !fields.iter().any(|f| f.name == key) {
                    return Err(ShapeError::UnknownField {
                        path: child(path, key),
                    });
                }
            }
            for f in fields {
                let sub = child(path, f.name);
                match node.get(f.name) {
                    Some(value) => {
                        validate_type(registry, &f.ty, value, &sub, depth + 1)?;
                        check_constraints(f.constraints, value, &sub)?;
                    }
                    None => {
                        if !matches!(f.ty, TypeRef::Option(_)) {
                            return Err(ShapeError::MissingField { path: sub });
                        }
                    }
                }
            }
            Ok(())
        }
        TypeDef::Enum(variants) => match node {
            UirNode::Str(name) => match variants.iter().find(|v| v.name == name) {
                Some(v) if v.payload.is_none() => Ok(()),
                Some(_) => Err(ShapeError::TypeMismatch {
                    path: path.to_string(),
                    expected: format!("{} variant with payload", name),
                    found: "string",
                }),
                None => Err(ShapeError::UnknownVariant {
                    path: path.to_string(),
                    variant: name.clone(),
                }),
            },
            UirNode::Map(entries) if entries.len() == 1 => {
                let (name, value) = &entries[0];
                match variants.iter().find(|v| v.name == name) {
                    Some(v) => match &v.payload {
                        Some(ty) => {
                            validate_type(registry, ty, value, &child(path, name), depth + 1)
                        }
                        None => Err(ShapeError::TypeMismatch {
                            path: child(path, name),
                            expected: "unit variant".to_string(),
                            found: kind_of(value),
                        }),
                    },
                    None => Err(ShapeError::UnknownVariant {
                        path: path.to_string(),
                        variant: name.clone(),
                    }),
                }
            }
            other => Err(ShapeError::TypeMismatch {
                path: path.to_string(),
                expected: format!("enum {}", schema.name),
                found: kind_of(other),
            }),
        },
        TypeDef::Union(members) => {
            for m in members {
                if validate_type(registry, m, node, path, depth + 1).is_ok() {
                    return Ok(());
                }
            }
            Err(ShapeError::NoUnionMatch {
                path: path.to_string(),
            })
        }
    }
}

fn validate_type(
    registry: &Registry,
    ty: &TypeRef,
    node: &UirNode,
    path: &str,
    depth: usize,
) -> Result<(), ShapeError> {
    if depth > MAX_DEPTH {
        return Err(ShapeError::TooDeep {
            path: path.to_string(),
        });
    }
    match ty {
        TypeRef::Prim(p) => {
            if prim_accepts(*p, node) {
                Ok(())
            } else {
                Err(ShapeError::TypeMismatch {
                    path: path.to_string(),
                    expected: p.name().to_string(),
                    found: kind_of(node),
                })
            }
        }
        TypeRef::Named(name) => {
            let schema = registry
                .latest(name)
                .ok_or(ShapeError::UnknownType { name })?;
            validate_schema(registry, schema, node, path, depth + 1)
        }
        TypeRef::Option(inner) => match node {
            UirNode::Null => Ok(()),
            other => validate_type(registry, inner, other, path, depth + 1),
        },
        TypeRef::Array(inner) => match node {
            UirNode::Seq(items) => {
                for (i, item) in items.iter().enumerate() {
                    validate_type(registry, inner, item, &format!("{path}[{i}]"), depth + 1)?;
                }
                Ok(())
            }
            other => Err(ShapeError::TypeMismatch {
                path: path.to_string(),
                expected: ty.describe(),
                found: kind_of(other),
            }),
        },
        TypeRef::Map(inner) => match node {
            UirNode::Map(entries) => {
                for (k, v) in entries {
                    validate_type(registry, inner, v, &child(path, k), depth + 1)?;
                }
                Ok(())
            }
            other => Err(ShapeError::TypeMismatch {
                path: path.to_string(),
                expected: ty.describe(),
                found: kind_of(other),
            }),
        },
    }
}

/// Whether a primitive accepts a node, including lossless numeric widening.
pub fn prim_accepts(prim: Prim, node: &UirNode) -> bool {
    match (prim, node) {
        (Prim::Bool, UirNode::Bool(_)) => true,
        (Prim::I64, UirNode::Int(_)) | (Prim::I64, UirNode::UInt(_)) => true,
        (Prim::U64, UirNode::UInt(_)) => true,
        (Prim::U64, UirNode::Int(v)) => *v >= 0,
        (Prim::F64, UirNode::Float(_) | UirNode::Int(_) | UirNode::UInt(_)) => true,
        (Prim::String, UirNode::Str(_)) => true,
        (Prim::Bytes, UirNode::Bytes(_)) => true,
        (Prim::Uuid, UirNode::Bytes(b)) => b.len() == 16,
        _ => false,
    }
}

fn child(path: &str, key: &str) -> String {
    format!("{path}.{key}")
}

/// Checks a value against the constraints declared on its field.
///
/// Numbers are compared by value. Strings, bytes and sequences are compared by length. Other
/// nodes are not constrained.
fn check_constraints(
    constraints: &[Constraint],
    node: &UirNode,
    path: &str,
) -> Result<(), ShapeError> {
    for c in constraints {
        let ok = match node {
            UirNode::Int(v) => holds(*c, *v as i128),
            UirNode::UInt(v) => holds(*c, *v as i128),
            UirNode::Float(v) => match *c {
                Constraint::Min(m) => *v >= m as f64,
                Constraint::Max(m) => *v <= m as f64,
            },
            UirNode::Str(s) => holds(*c, s.chars().count() as i128),
            UirNode::Bytes(b) => holds(*c, b.len() as i128),
            UirNode::Seq(items) => holds(*c, items.len() as i128),
            _ => true,
        };
        if !ok {
            return Err(ShapeError::Constraint {
                path: path.to_string(),
                constraint: c.to_string(),
            });
        }
    }
    Ok(())
}

fn holds(c: Constraint, v: i128) -> bool {
    match c {
        Constraint::Min(m) => v >= m as i128,
        Constraint::Max(m) => v <= m as i128,
    }
}
