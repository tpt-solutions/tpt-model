#![cfg(feature = "std")]

//! Positive derive snapshot: every attribute and field type the derive accepts, pinned in one
//! schema snapshot. Any change to what the derive emits shows up as drift.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tpt_model::{EntityId, Model, Registry};
use tpt_primitives_stub::Intent;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Shape")]
enum ShapeKind {
    Round,
    #[tpt(since = 1)]
    Square,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Everything")]
struct EverythingV1 {
    id: EntityId,
    #[tpt(min = 1, max = 99)]
    count: u32,
    offset: i64,
    ratio: f32,
    flag: bool,
    label: String,
    bytes: Vec<u8>,
    tags: Vec<String>,
    maybe: Option<String>,
    kind: ShapeKind,
    table: BTreeMap<String, u32>,
    #[tpt(intent = "read", capability = "secrets.read")]
    secret: String,
    #[tpt(rename = "old_title")]
    title: Option<String>,
}

#[test]
fn derive_shapes_match_the_checked_in_snapshot() {
    let mut registry = Registry::new();
    registry.register::<ShapeKind>().unwrap();
    registry.register::<EverythingV1>().unwrap();
    let _ = Intent::Read;
    tpt_model::assert_schema_snapshot!(
        registry,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/derive_shapes.json")
    );
}
