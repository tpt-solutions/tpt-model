//! The README quickstart as a complete program.
//!
//! Run with `cargo run -p tpt-model --example quickstart`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
enum Material {
    Steel,
    Titanium,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CadPartV1 {
    id: EntityId,
    name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    weight_kg: f32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct CadPartV2 {
    id: EntityId,
    name: String,
    #[tpt(since = 2, default = "Material::Titanium")]
    material: Material,
    #[tpt(since = 2)]
    mass_lbs: f64,
}

/// Turns the removed `weight_kg` into `mass_lbs`.
fn migrate_legacy_weight(
    old: &UirNode,
    _ctx: &MigrationContext,
) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let kg = match node.remove("weight_kg") {
        Some(UirNode::Float(v)) => v,
        _ => return Err(MigrationError::custom("weight_kg must be a float")),
    };
    node.insert("mass_lbs", UirNode::Float(kg * 2.204_62));
    Ok(node)
}

struct CadPartStep;

impl Migration for CadPartStep {
    type From = CadPartV1;
    type To = CadPartV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        migrate_legacy_weight(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    registry.register::<Material>().expect("register Material");
    let mut migrations = Migrations::new();
    migrations
        .register::<CadPartStep>(&mut registry)
        .expect("register migration");

    let id = EntityId::new_v7(1_700_000_000_000, [7; 10]);

    // Written by the old build.
    let stored = save(
        &Blake3,
        id,
        &CadPartV1 {
            id,
            name: "bracket".into(),
            weight_kg: 2.0,
        },
    )
    .expect("save");

    // Read by the new build.
    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let part: CadPartV2 = loader.load(&stored).expect("load");
    println!("{part:?}");
    assert_eq!(part.material, Material::Titanium);
}
