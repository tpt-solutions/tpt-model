//! Deterministic smoke test for the fuzz target `envelope_valid`: many pseudo-random payloads,
//! each sealed in an envelope with a correct hash, go through the loader. Every outcome must
//! be a value or an error. A panic fails the test.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::Migrations;
use tpt_model::load::Loader;
use tpt_model::{EntityId, Envelope, Model, Registry};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Smoke")]
struct SmokeV1 {
    value: u32,
    label: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Smoke")]
struct SmokeV2 {
    value: u32,
    label: String,
    #[tpt(since = 2)]
    extra: Option<u32>,
}

/// Small xorshift generator, so the inputs are the same on every run and platform.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }
}

#[test]
fn sealed_random_payloads_never_panic_the_loader() {
    let mut registry = Registry::new();
    registry.register::<SmokeV1>().unwrap();
    registry.register::<SmokeV2>().unwrap();
    let migrations = Migrations::new();
    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let id = EntityId::new_v7(1_700_000_000_000, [1; 10]);

    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    let mut accepted = 0usize;
    for round in 0..2_000u32 {
        let len = (rng.next() % 96) as usize;
        let payload = rng.bytes(len);
        let version = (round % 3) + 1;
        let envelope = Envelope::seal(&Blake3, SmokeV1::SCHEMA_ID, version, id, payload).unwrap();
        let bytes = envelope.encode();
        if loader.load::<SmokeV1>(&bytes).is_ok() || loader.load::<SmokeV2>(&bytes).is_ok() {
            accepted += 1;
        }
    }
    // Most random payloads are malformed; the test only asserts that nothing panicked.
    assert!(accepted < 2_000);
}
