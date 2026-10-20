//! One field becomes two.
//!
//! `full_name` in v1 is split into `first_name` and `last_name` in v2. Splitting needs a
//! registered migration; the automatic steps cannot guess where the name divides.
//!
//! Run with `cargo run -p tpt-model --example rename_and_split`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Person")]
struct PersonV1 {
    #[tpt(removed_in = 2, migrate = "split_name")]
    full_name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Person")]
struct PersonV2 {
    first_name: String,
    last_name: String,
}

/// Splits at the first space. A name without a space becomes a first name with an empty last name.
fn split_name(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let full = match node.remove("full_name") {
        Some(UirNode::Str(s)) => s,
        _ => return Err(MigrationError::custom("full_name must be a string")),
    };
    let (first, last) = full.split_once(' ').unwrap_or((full.as_str(), ""));
    node.insert("first_name", UirNode::Str(first.to_string()));
    node.insert("last_name", UirNode::Str(last.to_string()));
    Ok(node)
}

struct PersonStep;

impl Migration for PersonStep {
    type From = PersonV1;
    type To = PersonV2;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        split_name(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations
        .register::<PersonStep>(&mut registry)
        .expect("person v1 -> v2");

    let id = EntityId::new_v7(1_700_000_000_000, [11; 10]);
    let stored = save(
        &Blake3,
        id,
        &PersonV1 {
            full_name: "Grace Brewster Hopper".into(),
        },
    )
    .expect("save v1");

    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let person: PersonV2 = loader.load(&stored).expect("load as v2");
    println!("{person:?}");
    assert_eq!(
        person,
        PersonV2 {
            first_name: "Grace".into(),
            last_name: "Brewster Hopper".into(),
        }
    );
}
