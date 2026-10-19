#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_crypto_stub::Blake3;
use tpt_model::{EntityId, Envelope, Migrations, Registry};
use tpt_validex_stub::BoundsValidator;

// Arbitrary bytes through envelope parsing and the load pipeline must return an error or a value.
fuzz_target!(|data: &[u8]| {
    let registry = Registry::new();
    let migrations = Migrations::new();
    let loader = tpt_model::Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let _ = loader.load::<FuzzTarget>(data).map(|t| t.value);
    let _ = Envelope::decode(data).map(|e| e.verify(&Blake3));
    let _ = EntityId::from_bytes([0; 16]);
});

#[derive(serde::Deserialize, tpt_model::Model)]
#[tpt(version = 1)]
struct FuzzTarget {
    value: u32,
}
