//! Evolution tests for the item model.

use std::path::PathBuf;
use tpt_model::crypto::Sha256;
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Model, Registry};
use tpt_model::validex::BoundsValidator;
use {{crate_name}}::{ItemV1, ItemV2};

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<ItemV1>().unwrap();
    registry.register::<ItemV2>().unwrap();
    registry
}

#[test]
fn v1_items_load_as_v2_with_no_quantity() {
    let registry = registry();
    let migrations = Migrations::new();
    let id = EntityId::new_v7(1_700_000_000_000, [1; 10]);
    let stored = save(
        &Sha256,
        id,
        &ItemV1 {
            id,
            name: "bolt".into(),
        },
    )
    .unwrap();
    let loader = Loader::new(&registry, &migrations, Sha256, BoundsValidator::default());
    let item: ItemV2 = loader.load(&stored).unwrap();
    assert_eq!(item.name, "bolt");
    assert_eq!(item.quantity, None);
}

#[test]
fn schema_matches_the_checked_in_snapshot() {
    // First run: TPT_UPDATE_SNAPSHOTS=1 cargo test  (then review tests/snapshots/schema.json).
    tpt_model::assert_schema_snapshot!(
        registry(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/schema.json")
    );
}
