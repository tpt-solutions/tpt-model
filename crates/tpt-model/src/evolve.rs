//! The evolution engine.
//!
//! Given a persisted tree at version `a` and a target version `b`, the engine:
//!
//! 1. plans a path of steps through the version chain (see [`plan`]), detecting gaps,
//!    ambiguity and versions below the supported minimum;
//! 2. runs each step, either a registered programmatic [`Migration`] or the automatic
//!    structural step ([`auto_evolve`]);
//! 3. validates the result of every step against the step's target schema.

use crate::default_expr;
use crate::registry::{Registry, RegistryError};
use crate::schema::{Field, Schema, TypeDef, TypeRef};
use crate::shape::{self, prim_accepts, ShapeError};
use crate::Model;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Per-version bookkeeping during the shortest-path search: distance, path count, parent.
type SeenEntry = (u32, u32, Option<(u32, usize)>);
use core::fmt;
use tpt_uir_stub::UirNode;

/// Context passed to a [`Migration`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationContext {
    /// Logical type name.
    pub name: &'static str,
    pub from_version: u32,
    pub to_version: u32,
}

/// Everything that can go wrong while planning or running migrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
    Registry(RegistryError),
    NameMismatch {
        from: &'static str,
        to: &'static str,
    },
    NotForward {
        name: &'static str,
        from: u32,
        to: u32,
    },
    DuplicateMigration {
        name: &'static str,
        from: u32,
        to: u32,
    },
    NotRegistered {
        name: &'static str,
        version: u32,
    },
    /// Persisted data is newer than the code reading it.
    Downgrade {
        name: &'static str,
        from: u32,
        to: u32,
    },
    /// A version between source and target is not registered, and no migration bridges it.
    Gap {
        name: &'static str,
        missing: u32,
    },
    /// More than one shortest path exists between the two versions.
    Ambiguous {
        name: &'static str,
        from: u32,
        to: u32,
        paths: u32,
    },
    /// No path exists for a reason other than a single missing version.
    NoPath {
        name: &'static str,
        from: u32,
        to: u32,
    },
    BelowMinSupported {
        name: &'static str,
        found: u32,
        min: u32,
    },
    /// A new field has no default and no value to take from the source.
    MissingField {
        name: &'static str,
        field: &'static str,
        from: u32,
        to: u32,
    },
    /// A removed field carries a `migrate` attribute, so a registered migration is required.
    RequiresMigration {
        name: &'static str,
        field: &'static str,
        from: u32,
        to: u32,
    },
    /// The source tree has a field that the target does not know and that was not removed.
    UnknownField {
        name: &'static str,
        field: String,
        from: u32,
        to: u32,
    },
    /// A field changed type in a way that is not a widening conversion.
    TypeChange {
        name: &'static str,
        field: &'static str,
        from: u32,
        to: u32,
        expected: String,
        found: &'static str,
    },
    /// The tree produced by a step does not match the target schema.
    InvalidResult {
        name: &'static str,
        from: u32,
        to: u32,
        error: ShapeError,
    },
    /// A default expression could not be evaluated.
    Default {
        name: &'static str,
        field: &'static str,
        error: default_expr::DefaultError,
    },
    /// Automatic evolution applies to structs and enums only.
    Unsupported {
        name: &'static str,
        reason: &'static str,
    },
    /// Returned by user migration code.
    Custom(String),
}

impl MigrationError {
    /// Convenience constructor for user migration code.
    pub fn custom(message: impl Into<String>) -> Self {
        MigrationError::Custom(message.into())
    }
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use MigrationError::*;
        match self {
            Registry(e) => write!(f, "{e}"),
            NameMismatch { from, to } => write!(f, "migration joins different types `{from}` and `{to}`"),
            NotForward { name, from, to } => write!(f, "{name}: migration must move forward (v{from} -> v{to})"),
            DuplicateMigration { name, from, to } => write!(f, "{name}: duplicate migration v{from} -> v{to}"),
            NotRegistered { name, version } => write!(f, "{name} v{version} is not registered"),
            Downgrade { name, from, to } => write!(f, "{name}: data at v{from} is newer than target v{to}"),
            Gap { name, missing } => write!(f, "{name}: version v{missing} is missing from the chain"),
            Ambiguous { name, from, to, paths } => {
                write!(f, "{name}: {paths} equally short paths from v{from} to v{to}")
            }
            NoPath { name, from, to } => write!(f, "{name}: no migration path from v{from} to v{to}"),
            BelowMinSupported { name, found, min } => {
                write!(f, "{name}: data at v{found} is older than the minimum supported version v{min}")
            }
            MissingField { name, field, from, to } => {
                write!(f, "{name}: field `{field}` added in v{to} has no default (from v{from})")
            }
            RequiresMigration { name, field, from, to } => {
                write!(f, "{name}: field `{field}` is removed in v{to}; a migration v{from} -> v{to} must handle it")
            }
            UnknownField { name, field, from, to } => {
                write!(f, "{name}: field `{field}` is not in v{to} (from v{from})")
            }
            TypeChange { name, field, from, to, expected, found } => write!(
                f,
                "{name}: field `{field}` changes type in v{to} (from v{from}): expected {expected}, found {found}; an explicit migration is required"
            ),
            InvalidResult { name, from, to, error } => {
                write!(f, "{name}: result of v{from} -> v{to} is invalid: {error}")
            }
            Default { name, field, error } => write!(f, "{name}: field `{field}`: {error}"),
            Unsupported { name, reason } => write!(f, "{name}: {reason}"),
            Custom(message) => write!(f, "migration failed: {message}"),
        }
    }
}

impl From<RegistryError> for MigrationError {
    fn from(e: RegistryError) -> Self {
        MigrationError::Registry(e)
    }
}

/// Programmatic migration from one version to the next.
///
/// Implement this for each step that automatic evolution cannot handle, such as a
/// removed field whose data must be preserved.
pub trait Migration {
    type From: Model;
    type To: Model;

    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError>;
}

/// Erased form of [`Migration::migrate`].
pub type MigrateFn = fn(&UirNode, &MigrationContext) -> Result<UirNode, MigrationError>;

/// A registered programmatic migration.
#[derive(Clone, Copy)]
pub struct MigrationStep {
    pub name: &'static str,
    pub from: &'static Schema,
    pub to: &'static Schema,
    run: MigrateFn,
}

impl fmt::Debug for MigrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MigrationStep({} v{} -> v{})",
            self.name, self.from.version, self.to.version
        )
    }
}

/// The set of programmatic migrations.
#[derive(Debug, Default, Clone)]
pub struct Migrations {
    steps: Vec<MigrationStep>,
}

impl Migrations {
    pub const fn new() -> Self {
        Migrations { steps: Vec::new() }
    }

    /// Registers migration `M`, and both of its schemas, in `registry`.
    pub fn register<M: Migration>(
        &mut self,
        registry: &mut Registry,
    ) -> Result<(), MigrationError> {
        let from = M::From::schema();
        let to = M::To::schema();
        if from.name != to.name {
            return Err(MigrationError::NameMismatch {
                from: from.name,
                to: to.name,
            });
        }
        if to.version <= from.version {
            return Err(MigrationError::NotForward {
                name: from.name,
                from: from.version,
                to: to.version,
            });
        }
        if self.steps.iter().any(|s| {
            s.name == from.name && s.from.version == from.version && s.to.version == to.version
        }) {
            return Err(MigrationError::DuplicateMigration {
                name: from.name,
                from: from.version,
                to: to.version,
            });
        }
        registry.register_schema(from)?;
        registry.register_schema(to)?;
        self.steps.push(MigrationStep {
            name: from.name,
            from,
            to,
            run: M::migrate,
        });
        Ok(())
    }

    pub fn steps(&self) -> &[MigrationStep] {
        &self.steps
    }
}

/// One step of a resolved plan.
#[derive(Debug, Clone, Copy)]
pub enum Step<'a> {
    /// Automatic structural evolution between two adjacent versions.
    Auto {
        from: &'static Schema,
        to: &'static Schema,
    },
    /// A registered migration.
    Custom(&'a MigrationStep),
}

impl Step<'_> {
    fn endpoints(&self) -> (&'static Schema, &'static Schema) {
        match self {
            Step::Auto { from, to } => (from, to),
            Step::Custom(m) => (m.from, m.to),
        }
    }
}

#[derive(Clone, Copy)]
enum Edge {
    Auto,
    Custom(usize),
}

/// Resolves the shortest chain of steps from `from` to `to`.
///
/// Edges are: a registered migration between any two versions, and an automatic
/// step between consecutive registered versions that no migration covers.
pub fn plan<'a>(
    registry: &Registry,
    migrations: &'a Migrations,
    name: &'static str,
    from: u32,
    to: u32,
    min_supported: Option<u32>,
) -> Result<Vec<Step<'a>>, MigrationError> {
    if from > to {
        return Err(MigrationError::Downgrade { name, from, to });
    }
    if let Some(min) = min_supported {
        if from < min {
            return Err(MigrationError::BelowMinSupported {
                name,
                found: from,
                min,
            });
        }
    }
    if from == to {
        return Ok(Vec::new());
    }
    let present = registry.versions(name);
    for v in [from, to] {
        if !present.contains(&v) {
            return Err(MigrationError::NotRegistered { name, version: v });
        }
    }

    let mut edges: Vec<(u32, u32, Edge)> = Vec::new();
    for (idx, m) in migrations.steps().iter().enumerate() {
        if m.name == name {
            edges.push((m.from.version, m.to.version, Edge::Custom(idx)));
        }
    }
    for pair in present.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b == a + 1 && !edges.iter().any(|e| e.0 == a && e.1 == b) {
            edges.push((a, b, Edge::Auto));
        }
    }

    // Breadth-first search that also counts shortest paths.
    // Value: (distance, number of shortest paths, parent (version, edge index)).
    let mut seen: BTreeMap<u32, SeenEntry> = BTreeMap::new();
    seen.insert(from, (0, 1, None));
    let mut queue = alloc::collections::VecDeque::new();
    queue.push_back(from);
    while let Some(u) = queue.pop_front() {
        let (du, cu, _) = seen[&u];
        for (idx, &(a, b, _)) in edges.iter().enumerate() {
            if a != u {
                continue;
            }
            match seen.get_mut(&b) {
                None => {
                    seen.insert(b, (du + 1, cu, Some((u, idx))));
                    queue.push_back(b);
                }
                Some(entry) if entry.0 == du + 1 => entry.1 = entry.1.saturating_add(cu),
                Some(_) => {}
            }
        }
    }

    let (_, paths, _) = match seen.get(&to) {
        Some(entry) => *entry,
        None => {
            if let Some(missing) = (from + 1..to).find(|v| !present.contains(v)) {
                return Err(MigrationError::Gap { name, missing });
            }
            return Err(MigrationError::NoPath { name, from, to });
        }
    };
    if paths > 1 {
        return Err(MigrationError::Ambiguous {
            name,
            from,
            to,
            paths,
        });
    }

    // Walk parents back from the target.
    let mut chain = Vec::new();
    let mut cursor = to;
    while cursor != from {
        let (_, _, parent) = seen[&cursor];
        let (prev, idx) = parent.ok_or(MigrationError::NoPath { name, from, to })?;
        chain.push((prev, cursor, edges[idx].2));
        cursor = prev;
    }
    chain.reverse();

    let mut steps = Vec::with_capacity(chain.len());
    for (a, b, edge) in chain {
        match edge {
            Edge::Custom(idx) => steps.push(Step::Custom(&migrations.steps()[idx])),
            Edge::Auto => {
                let from_schema = registry
                    .by_name_version(name, a)
                    .ok_or(MigrationError::NotRegistered { name, version: a })?;
                let to_schema = registry
                    .by_name_version(name, b)
                    .ok_or(MigrationError::NotRegistered { name, version: b })?;
                steps.push(Step::Auto {
                    from: from_schema,
                    to: to_schema,
                });
            }
        }
    }
    Ok(steps)
}

/// Runs a plan against a tree, validating after every step.
pub fn execute(
    registry: &Registry,
    steps: &[Step<'_>],
    node: UirNode,
) -> Result<UirNode, MigrationError> {
    let mut node = node;
    for step in steps {
        let (from, to) = step.endpoints();
        node = match step {
            Step::Auto { from, to } => auto_evolve(&node, from, to)?,
            Step::Custom(m) => {
                let ctx = MigrationContext {
                    name: m.name,
                    from_version: from.version,
                    to_version: to.version,
                };
                let migrated = (m.run)(&node, &ctx)?;
                // Automatic structural work still applies around the custom step.
                complete_after_migration(&migrated, from, to)?
            }
        };
        shape::validate_node(registry, to, &node).map_err(|error| {
            MigrationError::InvalidResult {
                name: to.name,
                from: from.version,
                to: to.version,
                error,
            }
        })?;
    }
    Ok(node)
}

/// Plans and runs the evolution of `node` from `from` to `to` in one call.
pub fn migrate(
    registry: &Registry,
    migrations: &Migrations,
    name: &'static str,
    from: u32,
    to: u32,
    min_supported: Option<u32>,
    node: UirNode,
) -> Result<UirNode, MigrationError> {
    let steps = plan(registry, migrations, name, from, to, min_supported)?;
    if steps.is_empty() {
        // Nothing to run, but the tree still has to match the schema it claims.
        let target = registry
            .by_name_version(name, to)
            .ok_or(MigrationError::NotRegistered { name, version: to })?;
        shape::validate_node(registry, target, &node).map_err(|error| {
            MigrationError::InvalidResult {
                name,
                from,
                to,
                error,
            }
        })?;
        return Ok(node);
    }
    execute(registry, &steps, node)
}

/// Automatic structural step from `from` to `to`.
///
/// * Fields present in both: kept (primitive types must be compatible).
/// * Fields renamed (`rename` history): moved to the new name.
/// * New fields: filled from `default`, or `null` for `Option`. Otherwise an error.
/// * Removed fields: dropped, except that a removed field with `migrate` set is an error,
///   because a registered migration must decide what happens to its data.
/// * Anything else unknown to the target is an error.
pub fn auto_evolve(node: &UirNode, from: &Schema, to: &Schema) -> Result<UirNode, MigrationError> {
    let name = to.name;
    let (from_v, to_v) = (from.version, to.version);
    match (from.def, to.def) {
        (TypeDef::Enum(_), TypeDef::Enum(_)) => Ok(node.clone()),
        (TypeDef::Struct(from_fields), TypeDef::Struct(to_fields)) => {
            let entries = match node {
                UirNode::Map(entries) => entries,
                _ => return Ok(node.clone()), // the caller's validation reports the mismatch
            };
            // Pass 1: source keys the target does not keep.
            for (key, _) in entries {
                let kept = to_fields.iter().any(|f| f.name == key);
                let renamed = to_fields.iter().any(|f| f.renames.contains(&key.as_str()));
                if kept || renamed {
                    continue;
                }
                match from_fields.iter().find(|f| f.name == key) {
                    Some(f) if f.removed_in == Some(to_v) => {
                        if f.migrate.is_some() {
                            return Err(MigrationError::RequiresMigration {
                                name,
                                field: f.name,
                                from: from_v,
                                to: to_v,
                            });
                        }
                    }
                    _ => {
                        return Err(MigrationError::UnknownField {
                            name,
                            field: key.clone(),
                            from: from_v,
                            to: to_v,
                        })
                    }
                }
            }

            fill_fields(entries, to_fields, from_v, to_v, name)
        }
        _ => Err(MigrationError::Unsupported {
            name,
            reason: "automatic evolution cannot change between struct and enum",
        }),
    }
}

/// Completes a tree produced by a registered migration.
///
/// The migration has already handled removed data, so this does not re-check removals.
/// It still fills defaults and renames for target fields the migration left unset, and it
/// rejects keys that neither version knows about.
pub fn complete_after_migration(
    node: &UirNode,
    from: &Schema,
    to: &Schema,
) -> Result<UirNode, MigrationError> {
    let name = to.name;
    let (TypeDef::Struct(from_fields), TypeDef::Struct(to_fields)) = (from.def, to.def) else {
        return Ok(node.clone());
    };
    let entries = match node {
        UirNode::Map(entries) => entries,
        _ => return Ok(node.clone()),
    };
    let mut kept: Vec<(String, UirNode)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if to_fields.iter().any(|f| f.name == key) {
            kept.push((key.clone(), value.clone()));
        } else if to_fields.iter().any(|f| f.renames.contains(&key.as_str())) {
            // Left under an old name: `fill_fields` moves it to the renamed field.
            kept.push((key.clone(), value.clone()));
        } else if from_fields.iter().any(|f| {
            f.name == key
                && f.removed_in
                    .is_some_and(|r| r > from.version && r <= to.version)
        }) {
            // Removed at or before the target (possibly mid-skip): the migration has
            // consumed it, so drop it here.
        } else {
            return Err(MigrationError::UnknownField {
                name,
                field: key.clone(),
                from: from.version,
                to: to.version,
            });
        }
    }
    fill_fields(&kept, to_fields, from.version, to.version, name)
}

/// Builds the target field list: keeps present values, applies renames, then defaults,
/// then `null` for `Option`. Anything else is an error.
fn fill_fields(
    entries: &[(String, UirNode)],
    to_fields: &[Field],
    from_v: u32,
    to_v: u32,
    name: &'static str,
) -> Result<UirNode, MigrationError> {
    let has = |key: &str| entries.iter().any(|(k, _)| k == key);
    let value_of = |key: &str| entries.iter().find(|(k, _)| k == key).map(|(_, v)| v);

    let mut out: Vec<(String, UirNode)> = Vec::with_capacity(to_fields.len());
    for f in to_fields {
        let value = if has(f.name) {
            value_of(f.name).cloned()
        } else if let Some(old) = f.renames.iter().find(|r| has(r)) {
            value_of(old).cloned()
        } else if let Some(expr) = f.default {
            Some(
                default_expr::eval(expr).map_err(|error| MigrationError::Default {
                    name,
                    field: f.name,
                    error,
                })?,
            )
        } else if matches!(f.ty, TypeRef::Option(_)) {
            Some(UirNode::Null)
        } else {
            return Err(MigrationError::MissingField {
                name,
                field: f.name,
                from: from_v,
                to: to_v,
            });
        };
        if let Some(value) = value {
            check_compatible(name, f, &value, from_v, to_v)?;
            out.push((f.name.to_string(), value));
        }
    }
    Ok(UirNode::Map(out))
}

/// Rejects primitive changes that are not widening (for example `f64` -> `i64`).
/// Widening such as `i64` -> `f64` is accepted by [`prim_accepts`].
fn check_compatible(
    name: &'static str,
    field: &Field,
    value: &UirNode,
    from: u32,
    to: u32,
) -> Result<(), MigrationError> {
    let prim = match field.ty {
        TypeRef::Prim(p) => Some(p),
        TypeRef::Option(TypeRef::Prim(p)) => {
            if matches!(value, UirNode::Null) {
                return Ok(());
            }
            Some(*p)
        }
        _ => None,
    };
    match prim {
        Some(p) if !prim_accepts(p, value) => Err(MigrationError::TypeChange {
            name,
            field: field.name,
            from,
            to,
            expected: p.name().to_string(),
            found: shape::kind_of(value),
        }),
        _ => Ok(()),
    }
}

/// Explains, step by step and field by field, how `migrate` would move data from `from` to `to`.
///
/// Automatic steps list the fate of every target field (kept, renamed, defaulted, `null`) and of
/// every source field that does not survive. Registered migrations are listed by name only,
/// because their behaviour is in user code. Nothing is run.
pub fn explain(
    registry: &Registry,
    migrations: &Migrations,
    name: &'static str,
    from: u32,
    to: u32,
    min_supported: Option<u32>,
) -> Result<String, MigrationError> {
    let steps = plan(registry, migrations, name, from, to, min_supported)?;
    if steps.is_empty() {
        return Ok(format!("{name}: v{from} is already the target; no steps\n"));
    }
    let mut out = String::new();
    for (i, step) in steps.iter().enumerate() {
        match step {
            Step::Custom(m) => out.push_str(&format!(
                "{}. {name} v{} -> v{}: registered migration\n",
                i + 1,
                m.from.version,
                m.to.version
            )),
            Step::Auto { from, to } => {
                out.push_str(&format!(
                    "{}. {name} v{} -> v{}: automatic\n",
                    i + 1,
                    from.version,
                    to.version
                ));
                explain_auto(&mut out, from, to);
            }
        }
    }
    Ok(out)
}

fn explain_auto(out: &mut String, from: &Schema, to: &Schema) {
    let (TypeDef::Struct(from_fields), TypeDef::Struct(to_fields)) = (from.def, to.def) else {
        out.push_str("   enum: variants carry over unchanged\n");
        return;
    };
    for f in to_fields {
        let line = if from_fields.iter().any(|o| o.name == f.name) {
            format!("keep {}", f.name)
        } else if let Some(old) = from_fields.iter().find(|o| f.renames.contains(&o.name)) {
            format!("rename {} -> {}", old.name, f.name)
        } else if let Some(expr) = f.default {
            format!("default {} = {expr}", f.name)
        } else if matches!(f.ty, TypeRef::Option(_)) {
            format!("null {}", f.name)
        } else {
            format!("error: {} has no value in v{} data", f.name, from.version)
        };
        out.push_str(&format!("   {line}\n"));
    }
    for f in from_fields {
        if to_fields
            .iter()
            .any(|t| t.name == f.name || t.renames.contains(&f.name))
        {
            continue;
        }
        let line = match f.migrate {
            Some(m) => format!("error: {} needs migration `{m}`", f.name),
            None => format!("drop {}", f.name),
        };
        out.push_str(&format!("   {line}\n"));
    }
}

/// Describes a plan for logs and the CLI.
pub fn describe(steps: &[Step<'_>]) -> String {
    let mut out = String::new();
    for (i, step) in steps.iter().enumerate() {
        let (from, to) = step.endpoints();
        let kind = match step {
            Step::Auto { .. } => "auto",
            Step::Custom(_) => "migration",
        };
        out.push_str(&format!(
            "{}. {} v{} -> v{} ({})\n",
            i + 1,
            from.name,
            from.version,
            to.version,
            kind
        ));
    }
    out
}
