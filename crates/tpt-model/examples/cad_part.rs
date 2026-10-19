//! End-to-end example: a CAD part saved by an old build and loaded by a new one.
//!
//! This stands in for a tpt-vertex document adapter: the document layer calls
//! [`save`] when writing and [`Loader::load`] when reading, and never sees versions.
//!
//! Run with `cargo run -p tpt-model --example cad_part`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
pub enum Material {
    Steel,
    Titanium,
}

/// Version 1, as written by the first release of the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "CadPart")]
pub struct CadPartV1 {
    pub id: EntityId,
    pub name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    pub weight_kg: f32,
}

/// Version 2: weight is stored in pounds, and material is new.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "CadPart")]
pub struct CadPartV2 {
    pub id: EntityId,
    pub name: String,
    #[tpt(since = 2, default = "Material::Titanium")]
    pub material: Material,
    #[tpt(since = 2)]
    pub mass_lbs: f64,
}

const LB_PER_KG: f64 = 2.204_62;

/// The programmatic migration the derive asks for: converts `weight_kg` to `mass_lbs`.
pub fn migrate_legacy_weight(
    old: &UirNode,
    _ctx: &MigrationContext,
) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let kg = match node.remove("weight_kg") {
        Some(UirNode::Float(v)) => v,
        _ => return Err(MigrationError::custom("weight_kg must be a float")),
    };
    node.insert("mass_lbs", UirNode::Float(kg * LB_PER_KG));
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
    registry.register::<Material>().expect("material");
    let mut migrations = Migrations::new();
    migrations
        .register::<CadPartStep>(&mut registry)
        .expect("migration");
    registry.check_graph().expect("graph");

    let id = EntityId::new_v7(1_700_000_000_000, [5; 10]);

    // What an old build wrote to disk.
    let old_bytes = save(
        &Blake3,
        id,
        &CadPartV1 {
            id,
            name: "bracket".into(),
            weight_kg: 2.0,
        },
    )
    .expect("save v1");
    println!("stored {} bytes as CadPart v1", old_bytes.len());

    // What the new build reads.
    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let part: CadPartV2 = loader.load(&old_bytes).expect("load as v2");
    println!("loaded: {part:?}");

    assert_eq!(part.id, id);
    assert_eq!(part.name, "bracket");
    assert_eq!(
        part.material,
        Material::Titanium,
        "default applied to the new field"
    );
    assert!(
        (part.mass_lbs - 2.0 * LB_PER_KG).abs() < 1e-9,
        "weight converted to pounds"
    );

    // Current-version data round-trips without any migration.
    let current = save(&Blake3, id, &part).expect("save v2");
    let again: CadPartV2 = loader.load(&current).expect("load v2");
    assert_eq!(again, part);

    // Tampering is caught before any migration runs.
    let mut corrupted = old_bytes;
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    assert!(loader.load::<CadPartV2>(&corrupted).is_err());

    println!("ok");
}
