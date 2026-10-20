#![cfg(feature = "std")]

//! Schema snapshots: the checked-in snapshot matches the code, and drift is reported.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tpt_model::snapshot;
use tpt_model::{Model, Registry};

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Pair")]
struct PairV1 {
    first: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Pair")]
struct PairV2 {
    first: u32,
    #[tpt(since = 2, default = "5")]
    second: u32,
}

fn pair_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<PairV1>().unwrap();
    registry.register::<PairV2>().unwrap();
    registry
}

fn snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/pair.json")
}

#[test]
fn checked_in_snapshot_matches_the_code() {
    tpt_model::assert_schema_snapshot!(pair_registry(), snapshot_path());
}

#[test]
fn a_changed_default_is_reported_as_drift() {
    let dir = std::env::temp_dir().join(format!("tpt-model-snapshot-{}", std::process::id()));
    let path = dir.join("pair.json");
    // Write the current snapshot to a scratch file, then compare a registry with one default changed.
    if std::env::var_os("TPT_UPDATE_SNAPSHOTS").is_none() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, std::fs::read_to_string(snapshot_path()).unwrap()).unwrap();
    }
    #[derive(Debug, Serialize, Deserialize, Model)]
    #[tpt(version = 2, name = "Pair")]
    struct PairV2Drifted {
        first: u32,
        #[tpt(since = 2, default = "6")]
        second: u32,
    }
    let mut drifted = Registry::new();
    drifted.register::<PairV1>().unwrap();
    drifted.register::<PairV2Drifted>().unwrap();
    let result = snapshot::check(&drifted, &path);
    let _ = std::fs::remove_dir_all(&dir);
    let message = result.expect_err("a changed default must fail the snapshot");
    assert!(message.contains("differs from the code"), "{message}");
    assert!(message.contains("Pair v2"), "{message}");
}
