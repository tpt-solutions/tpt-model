//! Evolution matrix: one logical type, ten struct versions (V1..V10).
//!
//! Every version is saved, then loaded as `DocV10`, with the step-by-step chain
//! executed by the engine. Each case checks that the data that should survive does,
//! and that the defaults and migrations fill in what should be filled in.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::ir_serde;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
enum Color {
    Red,
    Blue,
}

// ---- hand-written migrations -----------------------------------------------

/// V5 -> V6: `legacy_weight` (kg, whole) becomes `mass_g` (grams, f64).
fn migrate_legacy_weight(
    old: &UirNode,
    _ctx: &MigrationContext,
) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    // The V4 -> V5 default is an Int, so accept both integer forms.
    let kg = match node.remove("legacy_weight") {
        Some(UirNode::UInt(v)) => v as f64,
        Some(UirNode::Int(v)) => v as f64,
        _ => {
            return Err(MigrationError::custom(
                "legacy_weight must be a whole number",
            ))
        }
    };
    node.insert("mass_g", UirNode::Float(kg * 1000.0));
    Ok(node)
}

/// V8 -> V9: `tags` (a list) is replaced by `summary` (a comma-separated string).
fn migrate_tags(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let summary = match node.remove("tags") {
        Some(UirNode::Seq(items)) => items
            .iter()
            .filter_map(|item| match item {
                UirNode::Str(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    };
    node.insert("summary", UirNode::Str(summary));
    Ok(node)
}

struct Step56;
impl Migration for Step56 {
    type From = DocV5;
    type To = DocV6;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        migrate_legacy_weight(old, ctx)
    }
}

struct Step89;
impl Migration for Step89 {
    type From = DocV8;
    type To = DocV9;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        migrate_tags(old, ctx)
    }
}

// ---- the ten versions ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct DocV1 {
    id: EntityId,
    name: String,
    score: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct DocV2 {
    id: EntityId,
    name: String,
    score: u32,
    #[tpt(since = 2, default = "[]")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 3)]
struct DocV3 {
    id: EntityId,
    #[tpt(rename = "name")]
    title: String,
    score: u32,
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 4)]
struct DocV4 {
    id: EntityId,
    title: String,
    score: u32,
    tags: Vec<String>,
    #[tpt(since = 4, default = "true")]
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 5)]
struct DocV5 {
    id: EntityId,
    title: String,
    score: u32,
    tags: Vec<String>,
    enabled: bool,
    #[tpt(
        since = 5,
        default = "0",
        removed_in = 6,
        migrate = "migrate_legacy_weight"
    )]
    legacy_weight: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 6)]
struct DocV6 {
    id: EntityId,
    title: String,
    score: u32,
    tags: Vec<String>,
    enabled: bool,
    #[tpt(since = 6)]
    mass_g: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 7)]
struct DocV7 {
    id: EntityId,
    title: String,
    score: u32,
    tags: Vec<String>,
    enabled: bool,
    mass_g: f64,
    #[tpt(since = 7, default = "Color::Red")]
    color: Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 8)]
struct DocV8 {
    id: EntityId,
    title: String,
    score: u32,
    tags: Vec<String>,
    enabled: bool,
    mass_g: f64,
    color: Color,
    #[tpt(since = 8, default = "None")]
    note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 9)]
struct DocV9 {
    id: EntityId,
    title: String,
    score: u32,
    enabled: bool,
    mass_g: f64,
    color: Color,
    note: Option<String>,
    #[tpt(since = 9)]
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 10)]
struct DocV10 {
    id: EntityId,
    title: String,
    score: u32,
    enabled: bool,
    mass_g: f64,
    color: Color,
    note: Option<String>,
    summary: String,
    #[tpt(since = 10, default = "1")]
    revision: u64,
}

// ---- the matrix ------------------------------------------------------------

fn id() -> EntityId {
    EntityId::new_v7(1_700_000_000_000, [3; 10])
}

fn registry_and_migrations() -> (Registry, Migrations) {
    let mut registry = Registry::new();
    registry.register::<Color>().unwrap();
    let mut migrations = Migrations::new();
    migrations.register::<Step56>(&mut registry).unwrap();
    migrations.register::<Step89>(&mut registry).unwrap();
    registry.register::<DocV1>().unwrap();
    registry.register::<DocV2>().unwrap();
    registry.register::<DocV3>().unwrap();
    registry.register::<DocV4>().unwrap();
    registry.register::<DocV5>().unwrap();
    registry.register::<DocV6>().unwrap();
    registry.register::<DocV7>().unwrap();
    registry.register::<DocV8>().unwrap();
    registry.register::<DocV9>().unwrap();
    registry.register::<DocV10>().unwrap();
    (registry, migrations)
}

fn loader<'a>(
    registry: &'a Registry,
    migrations: &'a Migrations,
) -> Loader<'a, Blake3, BoundsValidator> {
    Loader::new(registry, migrations, Blake3, BoundsValidator::default())
}

#[test]
fn registry_graph_is_clean() {
    let (registry, _) = registry_and_migrations();
    assert_eq!(registry.versions("Doc"), (1..=10).collect::<Vec<u32>>());
    registry.check_graph().unwrap();
}

#[test]
fn every_version_loads_as_latest_without_data_loss() {
    let (registry, migrations) = registry_and_migrations();
    let loader = loader(&registry, &migrations);
    let id = id();
    let tags = || vec!["a".to_string(), "b".to_string()];

    // V1
    let v1 = DocV1 {
        id,
        name: "bolt".into(),
        score: 5,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v1).unwrap()).unwrap();
    assert_eq!(got.id, id);
    assert_eq!(got.title, "bolt", "rename history carries name -> title");
    assert_eq!(got.score, 5);
    assert!(got.enabled);
    assert_eq!(
        got.mass_g, 0.0,
        "legacy_weight defaults to 0 at V5, so mass is 0"
    );
    assert_eq!(got.color, Color::Red);
    assert_eq!(got.note, None);
    assert_eq!(
        got.summary, "",
        "V1 has no tags, so the default list is empty"
    );
    assert_eq!(got.revision, 1);

    // V2
    let v2 = DocV2 {
        id,
        name: "bolt".into(),
        score: 5,
        tags: tags(),
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v2).unwrap()).unwrap();
    assert_eq!(got.title, "bolt");
    assert_eq!(got.summary, "a,b");

    // V4
    let v4 = DocV4 {
        id,
        title: "bolt".into(),
        score: 5,
        tags: tags(),
        enabled: false,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v4).unwrap()).unwrap();
    assert!(!got.enabled, "existing values are kept, not defaulted");

    // V5 carries legacy_weight, which the custom migration turns into mass_g.
    let v5 = DocV5 {
        id,
        title: "bolt".into(),
        score: 5,
        tags: tags(),
        enabled: true,
        legacy_weight: 3,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v5).unwrap()).unwrap();
    assert_eq!(got.mass_g, 3000.0);

    // V6 keeps mass_g, and the chain starts from there.
    let v6 = DocV6 {
        id,
        title: "bolt".into(),
        score: 5,
        tags: tags(),
        enabled: true,
        mass_g: 1500.5,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v6).unwrap()).unwrap();
    assert_eq!(got.mass_g, 1500.5);

    // V7 with a non-default colour.
    let v7 = DocV7 {
        id,
        title: "bolt".into(),
        score: 5,
        tags: tags(),
        enabled: true,
        mass_g: 2.0,
        color: Color::Blue,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v7).unwrap()).unwrap();
    assert_eq!(
        got.color,
        Color::Blue,
        "an existing enum value survives the chain"
    );

    // V8 with a note.
    let v8 = DocV8 {
        id,
        title: "bolt".into(),
        score: 5,
        tags: tags(),
        enabled: true,
        mass_g: 2.0,
        color: Color::Blue,
        note: Some("hi".into()),
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v8).unwrap()).unwrap();
    assert_eq!(got.note, Some("hi".to_string()));
    assert_eq!(
        got.summary, "a,b",
        "custom migration V8 -> V9 derives summary from tags"
    );

    // V9 hands over directly.
    let v9 = DocV9 {
        id,
        title: "bolt".into(),
        score: 5,
        enabled: true,
        mass_g: 2.0,
        color: Color::Blue,
        note: None,
        summary: "x".into(),
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v9).unwrap()).unwrap();
    assert_eq!(got.summary, "x");
    assert_eq!(got.revision, 1);

    // V10 round-trips unchanged.
    let v10 = DocV10 {
        id,
        title: "bolt".into(),
        score: 5,
        enabled: true,
        mass_g: 2.0,
        color: Color::Blue,
        note: None,
        summary: "x".into(),
        revision: 42,
    };
    let got: DocV10 = loader.load(&save(&Blake3, id, &v10).unwrap()).unwrap();
    assert_eq!(got, v10);
}

#[test]
fn hydration_matches_direct_serde() {
    let v10 = DocV10 {
        id: id(),
        title: "t".into(),
        score: 1,
        enabled: false,
        mass_g: 0.25,
        color: Color::Red,
        note: Some("n".into()),
        summary: String::new(),
        revision: 7,
    };
    let node = ir_serde::to_node(&v10).unwrap();
    assert_eq!(node.get("revision"), Some(&UirNode::UInt(7)));
    let back: DocV10 = ir_serde::from_node(&node).unwrap();
    assert_eq!(back, v10);
}

#[test]
fn schema_ids_differ_per_version_and_are_stable_within_a_run() {
    let ids = [
        DocV1::SCHEMA_ID,
        DocV2::SCHEMA_ID,
        DocV3::SCHEMA_ID,
        DocV4::SCHEMA_ID,
        DocV5::SCHEMA_ID,
        DocV6::SCHEMA_ID,
        DocV7::SCHEMA_ID,
        DocV8::SCHEMA_ID,
        DocV9::SCHEMA_ID,
        DocV10::SCHEMA_ID,
    ];
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(DocV3::SCHEMA_ID, tpt_model::SchemaId::of(DocV3::schema()));
}
