//! Tamper evidence: unkeyed SHA-256 detects corruption, keyed SHA-256 also detects forgery.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::{KeyedSha256, Sha256};
use tpt_model::envelope::EnvelopeError;
use tpt_model::evolve::Migrations;
use tpt_model::load::{save, LoadError, Loader};
use tpt_model::{EntityId, Envelope, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

const KEY: [u8; 32] = [0x42; 32];

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Ledger")]
struct LedgerV1 {
    balance: u64,
}

fn id() -> EntityId {
    EntityId::new_v7(1_700_000_000_000, [6; 10])
}

/// What an attacker who can rewrite storage does: re-seal a changed payload with the public
/// unkeyed digest.
fn forged(balance: u64) -> Vec<u8> {
    let node = UirNode::Map(vec![("balance".into(), UirNode::UInt(balance))]);
    let payload = tpt_uir_stub::encode(&node);
    Envelope::seal(&Sha256, LedgerV1::SCHEMA_ID, 1, id(), payload)
        .unwrap()
        .encode()
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<LedgerV1>().unwrap();
    registry
}

#[test]
fn keyed_digest_accepts_genuine_data() {
    let registry = registry();
    let migrations = Migrations::new();
    let bytes = save(&KeyedSha256::new(KEY), id(), &LedgerV1 { balance: 10 }).unwrap();
    let loader = Loader::new(
        &registry,
        &migrations,
        KeyedSha256::new(KEY),
        BoundsValidator::default(),
    );
    assert_eq!(
        loader.load::<LedgerV1>(&bytes).unwrap(),
        LedgerV1 { balance: 10 }
    );
}

#[test]
fn keyed_digest_rejects_a_forgery_made_with_the_public_digest() {
    let registry = registry();
    let migrations = Migrations::new();
    let loader = Loader::new(
        &registry,
        &migrations,
        KeyedSha256::new(KEY),
        BoundsValidator::default(),
    );
    let err = loader.load::<LedgerV1>(&forged(1_000_000)).unwrap_err();
    // The forgery names the unkeyed digest, so the keyed check reports the algorithm mismatch.
    assert_eq!(
        err,
        LoadError::Envelope(EnvelopeError::DigestMismatch {
            stored: tpt_crypto_stub::id::SHA256,
            expected: tpt_crypto_stub::id::HMAC_SHA256,
        })
    );
}

#[test]
fn keyed_digest_rejects_data_sealed_with_another_key() {
    let registry = registry();
    let migrations = Migrations::new();
    let bytes = save(&KeyedSha256::new([1; 32]), id(), &LedgerV1 { balance: 10 }).unwrap();
    let loader = Loader::new(
        &registry,
        &migrations,
        KeyedSha256::new(KEY),
        BoundsValidator::default(),
    );
    assert_eq!(
        loader.load::<LedgerV1>(&bytes).unwrap_err(),
        LoadError::Envelope(EnvelopeError::Tampered)
    );
}

#[test]
fn unkeyed_digest_accepts_a_forgery_documenting_its_limit() {
    // Why the keyed digest exists: the unkeyed one cannot tell a forgery from real data.
    let registry = registry();
    let migrations = Migrations::new();
    let loader = Loader::new(&registry, &migrations, Sha256, BoundsValidator::default());
    assert_eq!(
        loader.load::<LedgerV1>(&forged(1_000_000)).unwrap(),
        LedgerV1 { balance: 1_000_000 }
    );
}
