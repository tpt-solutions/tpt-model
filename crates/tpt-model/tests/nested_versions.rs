//! Nested named types are checked at the version in flight, not the latest version.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Model, Registry};
use tpt_validex_stub::BoundsValidator;

fn id() -> EntityId {
    EntityId::new_v7(1_700_000_000_000, [5; 10])
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct InnerV1 {
    a: u32,
}

/// A later version of `Inner`, which `OuterV1` never saw.
#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2)]
struct InnerV2 {
    a: u32,
    #[tpt(since = 2, default = "0")]
    b: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct OuterV1 {
    inner: InnerV1,
}

#[test]
fn nested_type_is_checked_at_the_outer_version() {
    let mut registry = Registry::new();
    registry.register::<InnerV1>().unwrap();
    registry.register::<InnerV2>().unwrap();
    registry.register::<OuterV1>().unwrap();
    let migrations = Migrations::new();
    let value = OuterV1 {
        inner: InnerV1 { a: 1 },
    };
    let bytes = save(&Blake3, id(), &value).unwrap();
    let got: OuterV1 = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default())
        .load(&bytes)
        .unwrap();
    assert_eq!(got, value);
}
