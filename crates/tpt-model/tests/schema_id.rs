//! `SchemaId` sensitivity: which edits change the id, and which do not. Enum evolution by
//! adding a variant.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Model, Registry};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Same")]
struct DefaultOne {
    a: u32,
    #[tpt(since = 2, default = "1")]
    b: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Same")]
struct DefaultTwo {
    a: u32,
    #[tpt(since = 2, default = "2")]
    b: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Order")]
struct FieldsAB {
    a: u32,
    b: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Order")]
struct FieldsBA {
    b: String,
    a: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Renamed")]
struct RenamedOne {
    a: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Renamed")]
struct RenamedTwo {
    #[tpt(rename = "a")]
    x: u32,
}

#[test]
fn a_changed_default_changes_the_id() {
    assert_ne!(DefaultOne::SCHEMA_ID, DefaultTwo::SCHEMA_ID);
}

#[test]
fn field_declaration_order_does_not_change_the_id() {
    assert_eq!(FieldsAB::SCHEMA_ID, FieldsBA::SCHEMA_ID);
}

#[test]
fn a_renamed_field_changes_the_id() {
    assert_ne!(RenamedOne::SCHEMA_ID, RenamedTwo::SCHEMA_ID);
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Level")]
enum LevelV1 {
    Low,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Level")]
enum LevelV2 {
    Low,
    Mid,
    High,
}

#[test]
fn an_added_variant_reads_old_data_unchanged() {
    let mut registry = Registry::new();
    registry.register::<LevelV1>().unwrap();
    registry.register::<LevelV2>().unwrap();
    let migrations = Migrations::new();
    let id = EntityId::new_v7(1_700_000_000_000, [8; 10]);
    let bytes = save(&Blake3, id, &LevelV1::High).unwrap();
    let got: LevelV2 = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default())
        .load(&bytes)
        .unwrap();
    assert_eq!(got, LevelV2::High);
}

#[test]
fn a_new_variant_is_refused_by_the_old_version() {
    // Data written by the new build cannot be read by a build that lacks the variant. The
    // old registry has no v2 schema, so the envelope is unknown there.
    let mut new_registry = Registry::new();
    new_registry.register::<LevelV2>().unwrap();
    let id = EntityId::new_v7(1_700_000_000_000, [8; 10]);
    let bytes = save(&Blake3, id, &LevelV2::Mid).unwrap();

    let mut old_registry = Registry::new();
    old_registry.register::<LevelV1>().unwrap();
    let migrations = Migrations::new();
    let err = Loader::new(
        &old_registry,
        &migrations,
        Blake3,
        BoundsValidator::default(),
    )
    .load::<LevelV1>(&bytes)
    .unwrap_err();
    assert!(
        matches!(err, tpt_model::LoadError::UnknownSchema(_)),
        "{err:?}"
    );
}
