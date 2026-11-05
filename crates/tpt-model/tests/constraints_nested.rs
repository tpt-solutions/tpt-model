//! Constraints declared on fields of nested structs are enforced when the outer value is
//! validated.

use serde::{Deserialize, Serialize};
use tpt_model::crypto::Sha256;
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Model, Registry};
use tpt_model::validex::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Limits")]
struct LimitsV1 {
    #[tpt(max = 10)]
    width: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Part")]
struct PartV1 {
    limits: LimitsV1,
}

#[test]
fn nested_struct_constraints_are_enforced() {
    let mut registry = Registry::new();
    registry.register::<LimitsV1>().unwrap();
    registry.register::<PartV1>().unwrap();
    let migrations = Migrations::new();
    let id = EntityId::new_v7(1_700_000_000_000, [3; 10]);
    let bytes = save(
        &Sha256,
        id,
        &PartV1 {
            limits: LimitsV1 { width: 11 },
        },
    )
    .unwrap();
    let err = Loader::new(&registry, &migrations, Sha256, BoundsValidator::default())
        .load::<PartV1>(&bytes)
        .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("`$.limits.width`") && message.contains("violates"),
        "unexpected error: {message}"
    );
}
