//! Generated tests: random values round-trip across versions, for each evolution shape.

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use tpt_model::evolve::Migrations;
use tpt_model::{EntityId, Model, Registry};

// Added field with a default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "PropAdd")]
struct AddV1 {
    first: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "PropAdd")]
struct AddV2 {
    first: u32,
    #[tpt(since = 2, default = "5")]
    second: u32,
}

// Renamed field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "PropRename")]
struct RenameV1 {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "PropRename")]
struct RenameV2 {
    #[tpt(rename = "name")]
    title: String,
}

// Widening: integer to float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "PropWiden")]
struct WidenV1 {
    x: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "PropWiden")]
struct WidenV2 {
    x: f64,
}

// Optional field added, which becomes null in old data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "PropOpt")]
struct OptV1 {
    id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "PropOpt")]
struct OptV2 {
    id: u32,
    note: Option<String>,
}

fn two_versions<A: Model, B: Model>() -> Registry {
    let mut registry = Registry::new();
    registry.register::<A>().unwrap();
    registry.register::<B>().unwrap();
    registry
}

proptest! {
    #[test]
    fn added_field_takes_its_default(first in any::<u32>()) {
        let registry = two_versions::<AddV1, AddV2>();
        let migrations = Migrations::new();
        tpt_model::assert_evolves!(
            registry,
            migrations,
            AddV1 { first },
            AddV2,
            AddV2 { first, second: 5 }
        );
    }

    #[test]
    fn renamed_field_keeps_its_value(name in "[ -~]{0,64}") {
        let registry = two_versions::<RenameV1, RenameV2>();
        let migrations = Migrations::new();
        tpt_model::assert_evolves!(
            registry,
            migrations,
            RenameV1 { name: name.clone() },
            RenameV2,
            RenameV2 { title: name }
        );
    }

    #[test]
    fn integer_widens_to_float_exactly(x in any::<u32>()) {
        let registry = two_versions::<WidenV1, WidenV2>();
        let migrations = Migrations::new();
        tpt_model::assert_evolves!(registry, migrations, WidenV1 { x }, WidenV2, WidenV2 { x: f64::from(x) });
    }

    #[test]
    fn new_option_is_null_and_old_value_survives(id in any::<u32>()) {
        let registry = two_versions::<OptV1, OptV2>();
        let migrations = Migrations::new();
        tpt_model::assert_evolves!(registry, migrations, OptV1 { id }, OptV2, OptV2 { id, note: None });
    }

    #[test]
    fn entity_ids_survive_every_evolution(bytes in any::<[u8; 10]>(), first in any::<u32>()) {
        // The entity id is stored in the envelope, not in the payload, so it must come back unchanged.
        let registry = two_versions::<AddV1, AddV2>();
        let migrations = Migrations::new();
        let id = EntityId::new_v7(1_700_000_000_000, bytes);
        let digest = tpt_model::__private::Blake3;
        let bytes = tpt_model::load::save(&digest, id, &AddV1 { first }).unwrap();
        let envelope = tpt_model::Envelope::decode(&bytes).unwrap();
        prop_assert_eq!(envelope.entity_id, id);
        let loader = tpt_model::load::Loader::new(
            &registry,
            &migrations,
            tpt_model::__private::Blake3,
            tpt_model::__private::BoundsValidator::default(),
        );
        let got: AddV2 = loader.load(&bytes).unwrap();
        prop_assert_eq!(got, AddV2 { first, second: 5 });
    }
}
