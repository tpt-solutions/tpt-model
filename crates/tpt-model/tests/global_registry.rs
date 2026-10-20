//! The process-wide registry, behind the `global-registry` feature.

#![cfg(feature = "global-registry")]

use serde::{Deserialize, Serialize};
use tpt_model::registry::global;
use tpt_model::Model;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "GlobalProbe")]
struct GlobalProbeV1 {
    n: u32,
}

#[test]
fn explicit_registration_is_visible_through_the_global_lock() {
    global::register::<GlobalProbeV1>().unwrap();
    let registry = global::lock();
    let schema = registry
        .latest("GlobalProbe")
        .expect("registered in the global registry");
    assert_eq!(schema.version, 1);
    assert_eq!(schema.name, "GlobalProbe");
}
