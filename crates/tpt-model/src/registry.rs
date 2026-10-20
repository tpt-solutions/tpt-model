//! Scoped schema registry: maps [`SchemaId`] to schema definitions and keeps the
//! version chain for each logical type name.

use crate::hash::SchemaId;
use crate::schema::{Schema, SchemaError, TypeDef, TypeRef};
use crate::Model;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Problems found while registering or checking schemas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Invalid(SchemaError),
    /// Two different structures claim the same name and version.
    VersionConflict {
        name: &'static str,
        version: u32,
    },
    /// A field or variant refers to a type that is not registered.
    DanglingRef {
        schema: &'static str,
        reference: &'static str,
    },
    /// Named types reference each other in a loop.
    Cycle {
        path: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::Invalid(e) => write!(f, "{e}"),
            RegistryError::VersionConflict { name, version } => {
                write!(f, "conflicting schemas registered for {name} v{version}")
            }
            RegistryError::DanglingRef { schema, reference } => {
                write!(
                    f,
                    "schema {schema} references unregistered type {reference}"
                )
            }
            RegistryError::Cycle { path } => write!(f, "cycle between named types: {path}"),
        }
    }
}

/// A set of schemas, keyed by id.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    schemas: BTreeMap<SchemaId, &'static Schema>,
    /// Index from logical name to version to schema, kept in step with `schemas`.
    by_name: BTreeMap<&'static str, BTreeMap<u32, &'static Schema>>,
}

impl Registry {
    pub const fn new() -> Self {
        Registry {
            schemas: BTreeMap::new(),
            by_name: BTreeMap::new(),
        }
    }

    /// Registers `T::schema()` and returns its id. Registering the same schema twice is a no-op.
    pub fn register<T: Model>(&mut self) -> Result<SchemaId, RegistryError> {
        self.register_schema(T::schema())
    }

    /// Registers a schema after validating it on its own.
    pub fn register_schema(&mut self, schema: &'static Schema) -> Result<SchemaId, RegistryError> {
        schema.validate().map_err(RegistryError::Invalid)?;
        let id = SchemaId::of(schema);
        if let Some(existing) = self.by_name_version(schema.name, schema.version) {
            if SchemaId::of(existing) != id {
                return Err(RegistryError::VersionConflict {
                    name: schema.name,
                    version: schema.version,
                });
            }
            return Ok(id);
        }
        self.schemas.insert(id, schema);
        self.by_name
            .entry(schema.name)
            .or_default()
            .insert(schema.version, schema);
        Ok(id)
    }

    pub fn get(&self, id: &SchemaId) -> Option<&'static Schema> {
        self.schemas.get(id).copied()
    }

    pub fn by_name_version(&self, name: &str, version: u32) -> Option<&'static Schema> {
        self.by_name.get(name)?.get(&version).copied()
    }

    /// Versions registered for `name`, ascending.
    pub fn versions(&self, name: &str) -> Vec<u32> {
        self.by_name
            .get(name)
            .map(|versions| versions.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Highest registered version for `name`.
    pub fn latest(&self, name: &str) -> Option<&'static Schema> {
        self.by_name.get(name)?.values().next_back().copied()
    }

    /// Highest registered version of `name` that is at most `version`.
    ///
    /// A type that did not change between versions need not be registered again, so this
    /// is the schema that applied when data was written at `version`.
    pub fn at_or_before(&self, name: &str, version: u32) -> Option<&'static Schema> {
        self.by_name
            .get(name)?
            .range(..=version)
            .next_back()
            .map(|(_, schema)| *schema)
    }

    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Iterates over all registered schemas in id order.
    pub fn iter(&self) -> impl Iterator<Item = &'static Schema> + '_ {
        self.schemas.values().copied()
    }

    /// Checks the whole graph: every named reference resolves, and there are no cycles.
    pub fn check_graph(&self) -> Result<(), RegistryError> {
        let names: Vec<&'static str> = {
            let mut n: Vec<&'static str> = self.schemas.values().map(|s| s.name).collect();
            n.sort_unstable();
            n.dedup();
            n
        };

        // Edges between logical names, merged across versions.
        let mut edges: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
        for schema in self.schemas.values() {
            let mut refs = Vec::new();
            collect_refs(schema, &mut refs);
            for r in refs {
                if !names.contains(&r) {
                    return Err(RegistryError::DanglingRef {
                        schema: schema.name,
                        reference: r,
                    });
                }
                edges.entry(schema.name).or_default().push(r);
            }
        }

        // Depth-first search with colouring: 0 = unvisited, 1 = on stack, 2 = done.
        let mut colour: BTreeMap<&'static str, u8> = BTreeMap::new();
        let mut stack: Vec<&'static str> = Vec::new();
        for name in &names {
            if colour.get(name).copied().unwrap_or(0) == 0 {
                visit(name, &edges, &mut colour, &mut stack)?;
            }
        }
        Ok(())
    }
}

fn visit(
    node: &'static str,
    edges: &BTreeMap<&'static str, Vec<&'static str>>,
    colour: &mut BTreeMap<&'static str, u8>,
    stack: &mut Vec<&'static str>,
) -> Result<(), RegistryError> {
    colour.insert(node, 1);
    stack.push(node);
    if let Some(next) = edges.get(node) {
        for &n in next {
            match colour.get(n).copied().unwrap_or(0) {
                1 => {
                    let start = stack.iter().position(|s| *s == n).unwrap_or(0);
                    let mut path: Vec<&str> = stack[start..].to_vec();
                    path.push(n);
                    return Err(RegistryError::Cycle {
                        path: path.join(" -> ").to_string(),
                    });
                }
                0 => visit(n, edges, colour, stack)?,
                _ => {}
            }
        }
    }
    stack.pop();
    colour.insert(node, 2);
    Ok(())
}

fn collect_refs(schema: &Schema, out: &mut Vec<&'static str>) {
    match schema.def {
        TypeDef::Struct(fields) => {
            for f in fields {
                collect_type_refs(&f.ty, out);
            }
        }
        TypeDef::Enum(variants) => {
            for v in variants {
                if let Some(p) = &v.payload {
                    collect_type_refs(p, out);
                }
            }
        }
        TypeDef::Union(members) => {
            for m in members {
                collect_type_refs(m, out);
            }
        }
    }
}

fn collect_type_refs(t: &TypeRef, out: &mut Vec<&'static str>) {
    match t {
        TypeRef::Prim(_) => {}
        TypeRef::Named(n) => out.push(n),
        TypeRef::Option(inner) | TypeRef::Array(inner) | TypeRef::Map(inner) => {
            collect_type_refs(inner, out)
        }
    }
}

#[cfg(feature = "global-registry")]
pub mod global {
    //! Process-wide registry. Registration is explicit (see [`register`]); there is
    //! no link-time auto-registration.

    use super::{Registry, RegistryError};
    use crate::hash::SchemaId;
    use crate::Model;
    use std::sync::{Mutex, MutexGuard};

    static GLOBAL: Mutex<Registry> = Mutex::new(Registry::new());

    /// Locks the global registry.
    pub fn lock() -> MutexGuard<'static, Registry> {
        GLOBAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registers `T` in the global registry.
    pub fn register<T: Model>() -> Result<SchemaId, RegistryError> {
        lock().register::<T>()
    }
}
