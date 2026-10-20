//! Custom migrations that rename fields or skip versions.
//!
//! A registered migration consumes the data it handles, but the automatic completion
//! step still has to accept keys that the target expresses through `rename` history or
//! through a removal that happened in a skipped version.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

fn id() -> EntityId {
    EntityId::new_v7(1_700_000_000_000, [4; 10])
}

fn identity(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    Ok(old.clone())
}

// ---- rename after a custom migration ----------------------------------------------

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Ren")]
struct RenV1 {
    name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Ren")]
struct RenV2 {
    #[tpt(rename = "name")]
    title: String,
}

/// Leaves the old `name` key in place, so the rename is left to the completion step.
struct RenStep;
impl Migration for RenStep {
    type From = RenV1;
    type To = RenV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        identity(old, ctx)
    }
}

#[test]
fn custom_migration_may_leave_a_renamed_key_for_completion() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations.register::<RenStep>(&mut registry).unwrap();
    let bytes = save(
        &Blake3,
        id(),
        &RenV1 {
            name: "bolt".into(),
        },
    )
    .unwrap();
    let got: RenV2 = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default())
        .load(&bytes)
        .unwrap();
    assert_eq!(
        got,
        RenV2 {
            title: "bolt".into()
        }
    );
}

// ---- removal in a skipped version ---------------------------------------------------

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Skip")]
struct SkipV1 {
    keep: u32,
    #[tpt(removed_in = 2, migrate = "identity")]
    old: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Skip")]
struct SkipV3 {
    keep: u32,
}

/// Jumps from v1 to v3 in one step. Version 2 is never registered.
struct SkipStep;
impl Migration for SkipStep {
    type From = SkipV1;
    type To = SkipV3;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        identity(old, ctx)
    }
}

#[test]
fn field_removed_in_a_skipped_version_is_dropped() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations.register::<SkipStep>(&mut registry).unwrap();
    let bytes = save(&Blake3, id(), &SkipV1 { keep: 5, old: 9 }).unwrap();
    let got: SkipV3 = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default())
        .load(&bytes)
        .unwrap();
    assert_eq!(got, SkipV3 { keep: 5 });
}

#[test]
fn unexpected_key_is_still_rejected_after_a_custom_migration() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations.register::<SkipStep>(&mut registry).unwrap();
    let node = UirNode::Map(vec![
        ("keep".into(), UirNode::UInt(1)),
        ("stranger".into(), UirNode::UInt(2)),
    ]);
    let err =
        tpt_model::evolve::migrate(&registry, &migrations, "Skip", 1, 3, None, node).unwrap_err();
    assert!(matches!(err, MigrationError::UnknownField { ref field, .. } if field == "stranger"));
}
