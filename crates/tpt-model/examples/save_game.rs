//! Enum evolution inside a saved game.
//!
//! In v2 the `Sword` variant is replaced by `Blade`. The item enum is versioned, so each save
//! version sees the item type it was written with. The save itself is versioned too, and a
//! registered migration rewrites the old variant name in the stored tree.
//!
//! Run with `cargo run -p tpt-model --example save_game`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Item")]
enum ItemV1 {
    Potion,
    Sword,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Item")]
enum ItemV2 {
    Potion,
    Blade,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Save")]
struct SaveV1 {
    player: String,
    items: Vec<ItemV1>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Save")]
struct SaveV2 {
    player: String,
    items: Vec<ItemV2>,
}

/// Rewrites every `"Sword"` item to `"Blade"`, and leaves the rest of the save alone.
fn sword_to_blade(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let items = match node.remove("items") {
        Some(UirNode::Seq(items)) => items,
        _ => return Err(MigrationError::custom("items must be a sequence")),
    };
    let items = items
        .into_iter()
        .map(|item| match item {
            UirNode::Str(name) if name == "Sword" => UirNode::Str("Blade".into()),
            other => other,
        })
        .collect();
    node.insert("items", UirNode::Seq(items));
    Ok(node)
}

struct SaveStep;

impl Migration for SaveStep {
    type From = SaveV1;
    type To = SaveV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        sword_to_blade(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    registry.register::<ItemV1>().expect("item v1");
    registry.register::<ItemV2>().expect("item v2");
    let mut migrations = Migrations::new();
    migrations
        .register::<SaveStep>(&mut registry)
        .expect("save v1 -> v2");

    let id = EntityId::new_v7(1_700_000_000_000, [10; 10]);
    let stored = save(
        &Blake3,
        id,
        &SaveV1 {
            player: "ada".into(),
            items: vec![ItemV1::Potion, ItemV1::Sword],
        },
    )
    .expect("save v1");

    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let game: SaveV2 = loader.load(&stored).expect("load as v2");
    println!("{game:?}");
    assert_eq!(
        game,
        SaveV2 {
            player: "ada".into(),
            items: vec![ItemV2::Potion, ItemV2::Blade],
        }
    );
}
