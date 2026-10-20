//! Schema snapshots: a JSON file per registry, compared with the code by a test.
//!
//! This is the test-helper form of the `snapshot` command. The CLI cannot load compiled types, so
//! the check runs inside the crate's own tests. Each snapshot holds every schema with its
//! `SchemaId`, so a change to a type's structure, defaults or migrations shows up as a new id.

use crate::export::{registry_value, to_json};
use crate::registry::Registry;
use alloc::format;
use alloc::string::String;
use std::fs;
use std::path::Path;

/// Compares the registry with the snapshot at `path`.
///
/// With `TPT_UPDATE_SNAPSHOTS` set, the snapshot is written instead, so review the diff after
/// regenerating. Otherwise a missing or different snapshot is an error that lists the schema ids
/// the code currently produces.
pub fn check(registry: &Registry, path: &Path) -> Result<(), String> {
    let actual = to_json(&registry_value(registry));
    if std::env::var_os("TPT_UPDATE_SNAPSHOTS").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        return fs::write(path, actual).map_err(|e| format!("{}: {e}", path.display()));
    }
    let expected = fs::read_to_string(path).map_err(|e| {
        format!(
            "missing schema snapshot {} ({e}); run with TPT_UPDATE_SNAPSHOTS=1 to create it",
            path.display()
        )
    })?;
    if expected.replace("\r\n", "\n") == actual {
        return Ok(());
    }
    let mut ids = String::new();
    for schema in registry.iter() {
        ids.push_str(&format!(
            "\n  {} v{} {}",
            schema.name,
            schema.version,
            crate::hash::SchemaId::of(schema)
        ));
    }
    Err(format!(
        "schema snapshot {} differs from the code. If the change is intended, regenerate with \
         TPT_UPDATE_SNAPSHOTS=1 and review the diff. Current schema ids:{ids}",
        path.display()
    ))
}
