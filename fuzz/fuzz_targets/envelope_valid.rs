#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_crypto_stub::Blake3;
use tpt_model::{EntityId, Envelope, Migrations, Model, Registry};
use tpt_validex_stub::BoundsValidator;

// Random bytes almost never carry a valid hash, so `envelope_load` rarely gets past the
// tamper check. This target seals the fuzz input in an envelope with a correct hash, so the
// validation, decoding and migration paths see arbitrary payloads. The first byte picks the
// version the envelope claims, which also exercises the version-mismatch check.
fuzz_target!(|data: &[u8]| {
    let Some((&claimed, payload)) = data.split_first() else {
        return;
    };
    let id = EntityId::new_v7(1_700_000_000_000, [1; 10]);

    let mut registry = Registry::new();
    registry.register::<FuzzV1>().expect("register");
    registry.register::<FuzzV2>().expect("register");
    let migrations = Migrations::new();
    let loader = tpt_model::Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());

    let version = u32::from(claimed % 3) + 1;
    let Ok(envelope) = Envelope::seal(&Blake3, FuzzV1::SCHEMA_ID, version, id, payload.to_vec())
    else {
        return;
    };
    let _ = loader.load::<FuzzV1>(&envelope.encode());
    let _ = loader.load::<FuzzV2>(&envelope.encode());
});

#[derive(serde::Deserialize, serde::Serialize, tpt_model::Model)]
#[tpt(version = 1, name = "Fuzz")]
struct FuzzV1 {
    value: u32,
    label: String,
}

#[derive(serde::Deserialize, serde::Serialize, tpt_model::Model)]
#[tpt(version = 2, name = "Fuzz")]
struct FuzzV2 {
    value: u32,
    label: String,
    #[tpt(since = 2)]
    extra: Option<u32>,
}
