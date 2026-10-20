//! An explicit, lossy type change.
//!
//! `celsius: f64` becomes `centi_celsius: i64`. Narrowing from a float to an integer is never
//! automatic: the derive requires a migration, and the migration decides how to round.
//!
//! Run with `cargo run -p tpt-model --example custom_migration`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Reading")]
struct ReadingV1 {
    #[tpt(removed_in = 2, migrate = "to_centi")]
    celsius: f64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Reading")]
struct ReadingV2 {
    centi_celsius: i64,
}

/// Stores hundredths of a degree, rounded half away from zero.
fn to_centi(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let celsius = match node.remove("celsius") {
        Some(UirNode::Float(v)) if v.is_finite() => v,
        _ => return Err(MigrationError::custom("celsius must be a finite float")),
    };
    node.insert(
        "centi_celsius",
        UirNode::Int((celsius * 100.0).round() as i64),
    );
    Ok(node)
}

struct ReadingStep;

impl Migration for ReadingStep {
    type From = ReadingV1;
    type To = ReadingV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        to_centi(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations
        .register::<ReadingStep>(&mut registry)
        .expect("reading v1 -> v2");

    let id = EntityId::new_v7(1_700_000_000_000, [12; 10]);
    let stored = save(&Blake3, id, &ReadingV1 { celsius: 21.456 }).expect("save v1");

    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let reading: ReadingV2 = loader.load(&stored).expect("load as v2");
    println!("{reading:?}");
    assert_eq!(
        reading,
        ReadingV2 {
            centi_celsius: 2146
        }
    );
}
