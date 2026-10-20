//! Tamper evidence with a keyed digest.
//!
//! The unkeyed [`Blake3`] digest catches corruption, but anyone who can rewrite the stored
//! bytes can also recompute the hash. A keyed digest stops that: the forger needs the key.
//!
//! Run with `cargo run -p tpt-model --example keyed_digest`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::{Blake3, KeyedBlake3};
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Envelope, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Ledger")]
struct LedgerV1 {
    balance: u64,
}

fn main() {
    // In a real application the key comes from a secret store, not from source code.
    let key = KeyedBlake3::new([0x42; 32]);
    let id = EntityId::new_v7(1_700_000_000_000, [1; 10]);

    let mut registry = Registry::new();
    registry.register::<LedgerV1>().expect("schema registers");
    let migrations = Migrations::new();

    let stored = save(&key, id, &LedgerV1 { balance: 10 }).expect("save");
    let loader = Loader::new(
        &registry,
        &migrations,
        key.clone(),
        BoundsValidator::default(),
    );
    println!(
        "genuine data loads: {:?}",
        loader.load::<LedgerV1>(&stored).expect("load")
    );

    // An attacker rewrites the balance and re-seals it with the public, unkeyed digest.
    let node = UirNode::Map(vec![("balance".into(), UirNode::UInt(1_000_000))]);
    let forged = Envelope::seal(
        &Blake3,
        LedgerV1::SCHEMA_ID,
        1,
        id,
        tpt_uir_stub::encode(&node),
    )
    .expect("seal")
    .encode();
    match loader.load::<LedgerV1>(&forged) {
        Ok(_) => println!("forgery accepted (this should not happen with a keyed digest)"),
        Err(e) => println!("forgery rejected: {e}"),
    }
}
