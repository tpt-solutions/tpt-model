//! `ModelEnvelope`: the wrapper stored on disk or sent over the wire.
//!
//! Layout (all integers little-endian):
//!
//! ```text
//! magic        4   "TPTM"
//! format       1   2
//! digest       1   algorithm id from `crate::crypto::id` (1 = SHA-256, 2 = HMAC-SHA256)
//! schema_id   32
//! version      4
//! entity_id   16
//! payload_len  4
//! payload      payload_len
//! hash        32   digest(header ++ payload), from the Digest implementation
//! ```
//!
//! Format 1 envelopes used SHA-256 and are refused. Their hash cannot be checked
//! with the current digests.

use crate::entity::EntityId;
use crate::hash::SchemaId;
use alloc::vec::Vec;
use core::fmt;
use crate::crypto::{ct_eq, Digest, Hash32};

pub const MAGIC: [u8; 4] = *b"TPTM";
pub const FORMAT: u8 = 2;
/// Largest payload accepted by [`Envelope::decode`].
pub const MAX_PAYLOAD: usize = 64 * 1024 * 1024;
const FORMAT_AT: usize = 4;
const DIGEST_AT: usize = 5;
const SCHEMA_AT: usize = 6;
const VERSION_AT: usize = 38;
const ENTITY_AT: usize = 42;
const PAYLOAD_LEN_AT: usize = 58;
const HEADER_LEN: usize = PAYLOAD_LEN_AT + 4;
const TRAILER_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    Truncated {
        needed: usize,
        available: usize,
    },
    BadMagic,
    UnsupportedFormat(u8),
    PayloadTooLarge(usize),
    /// Bytes remained after the trailing hash.
    TrailingBytes(usize),
    /// The stored hash does not match the header and payload.
    Tampered,
    /// The envelope names a different digest algorithm than the one used to check it.
    DigestMismatch {
        stored: u8,
        expected: u8,
    },
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::Truncated { needed, available } => {
                write!(
                    f,
                    "envelope truncated: need {needed} bytes, have {available}"
                )
            }
            EnvelopeError::BadMagic => f.write_str("not a TPT envelope (bad magic)"),
            EnvelopeError::UnsupportedFormat(v) => write!(f, "unsupported envelope format {v}"),
            EnvelopeError::PayloadTooLarge(n) => {
                write!(f, "payload of {n} bytes exceeds {MAX_PAYLOAD}")
            }
            EnvelopeError::TrailingBytes(n) => write!(f, "{n} unexpected trailing bytes"),
            EnvelopeError::Tampered => f.write_str("envelope hash mismatch: data was modified"),
            EnvelopeError::DigestMismatch { stored, expected } => write!(
                f,
                "envelope was sealed with digest {stored}, but the checker uses digest {expected}"
            ),
        }
    }
}

/// The name the design spec uses for [`Envelope`].
pub type ModelEnvelope = Envelope;

/// A sealed entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Algorithm id of the digest that sealed this envelope.
    pub digest: u8,
    pub schema_id: SchemaId,
    pub version: u32,
    pub entity_id: EntityId,
    pub payload: Vec<u8>,
    pub hash: Hash32,
}

impl Envelope {
    /// Seals a payload, computing its hash with `digest`.
    pub fn seal<D: Digest>(
        digest: &D,
        schema_id: SchemaId,
        version: u32,
        entity_id: EntityId,
        payload: Vec<u8>,
    ) -> Result<Envelope, EnvelopeError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(EnvelopeError::PayloadTooLarge(payload.len()));
        }
        let digest_id = digest.id();
        let header = header_bytes(digest_id, schema_id, version, entity_id, payload.len());
        let hash = digest.digest(&[&header, &payload]);
        Ok(Envelope {
            digest: digest_id,
            schema_id,
            version,
            entity_id,
            payload,
            hash,
        })
    }

    /// Serializes the envelope.
    pub fn encode(&self) -> Vec<u8> {
        let header = self.header();
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len() + TRAILER_LEN);
        out.extend_from_slice(&header);
        out.extend_from_slice(&self.payload);
        out.extend_from_slice(&self.hash);
        out
    }

    /// Parses an envelope. Checks lengths and magic. Does not check the hash; see [`Envelope::verify`].
    pub fn decode(bytes: &[u8]) -> Result<Envelope, EnvelopeError> {
        if bytes.len() < HEADER_LEN {
            return Err(EnvelopeError::Truncated {
                needed: HEADER_LEN,
                available: bytes.len(),
            });
        }
        if bytes[0..4] != MAGIC {
            return Err(EnvelopeError::BadMagic);
        }
        if bytes[FORMAT_AT] != FORMAT {
            return Err(EnvelopeError::UnsupportedFormat(bytes[FORMAT_AT]));
        }
        let digest = bytes[DIGEST_AT];
        let mut schema = [0u8; 32];
        schema.copy_from_slice(&bytes[SCHEMA_AT..VERSION_AT]);
        let version = u32::from_le_bytes([
            bytes[VERSION_AT],
            bytes[VERSION_AT + 1],
            bytes[VERSION_AT + 2],
            bytes[VERSION_AT + 3],
        ]);
        let mut entity = [0u8; 16];
        entity.copy_from_slice(&bytes[ENTITY_AT..PAYLOAD_LEN_AT]);
        let payload_len = u32::from_le_bytes([
            bytes[PAYLOAD_LEN_AT],
            bytes[PAYLOAD_LEN_AT + 1],
            bytes[PAYLOAD_LEN_AT + 2],
            bytes[PAYLOAD_LEN_AT + 3],
        ]) as usize;
        if payload_len > MAX_PAYLOAD {
            return Err(EnvelopeError::PayloadTooLarge(payload_len));
        }
        let needed = HEADER_LEN + payload_len + TRAILER_LEN;
        if bytes.len() < needed {
            return Err(EnvelopeError::Truncated {
                needed,
                available: bytes.len(),
            });
        }
        if bytes.len() > needed {
            return Err(EnvelopeError::TrailingBytes(bytes.len() - needed));
        }
        let payload = bytes[HEADER_LEN..HEADER_LEN + payload_len].to_vec();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes[needed - TRAILER_LEN..]);
        Ok(Envelope {
            digest,
            schema_id: SchemaId(schema),
            version,
            entity_id: EntityId::from_bytes(entity),
            payload,
            hash,
        })
    }

    /// The header bytes this envelope is hashed with.
    fn header(&self) -> [u8; HEADER_LEN] {
        header_bytes(
            self.digest,
            self.schema_id,
            self.version,
            self.entity_id,
            self.payload.len(),
        )
    }

    /// Recomputes the hash with `digest` and compares it in constant time.
    ///
    /// Returns [`EnvelopeError::DigestMismatch`] if the envelope was sealed with a
    /// different algorithm, for example an unkeyed envelope checked with a keyed digest.
    pub fn verify<D: Digest>(&self, digest: &D) -> Result<(), EnvelopeError> {
        if self.digest != digest.id() {
            return Err(EnvelopeError::DigestMismatch {
                stored: self.digest,
                expected: digest.id(),
            });
        }
        let expected = digest.digest(&[&self.header(), &self.payload]);
        if ct_eq(&expected, &self.hash) {
            Ok(())
        } else {
            Err(EnvelopeError::Tampered)
        }
    }
}

fn header_bytes(
    digest: u8,
    schema_id: SchemaId,
    version: u32,
    entity_id: EntityId,
    payload_len: usize,
) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[0..4].copy_from_slice(&MAGIC);
    h[FORMAT_AT] = FORMAT;
    h[DIGEST_AT] = digest;
    h[SCHEMA_AT..VERSION_AT].copy_from_slice(schema_id.as_bytes());
    h[VERSION_AT..ENTITY_AT].copy_from_slice(&version.to_le_bytes());
    h[ENTITY_AT..PAYLOAD_LEN_AT].copy_from_slice(entity_id.as_bytes());
    h[PAYLOAD_LEN_AT..HEADER_LEN].copy_from_slice(&(payload_len as u32).to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Sha256;

    fn sample() -> Envelope {
        Envelope::seal(
            &Sha256,
            SchemaId([7; 32]),
            2,
            EntityId::new_v7(42, [1; 10]),
            alloc::vec![1, 2, 3],
        )
        .unwrap()
    }

    #[test]
    fn round_trip_and_verify() {
        let env = sample();
        let bytes = env.encode();
        let back = Envelope::decode(&bytes).unwrap();
        assert_eq!(back, env);
        back.verify(&Sha256).unwrap();
    }

    #[test]
    fn any_flipped_byte_is_detected() {
        let bytes = sample().encode();
        for i in HEADER_LEN..bytes.len() - TRAILER_LEN {
            let mut bad = bytes.clone();
            bad[i] ^= 0x01;
            let env = Envelope::decode(&bad).unwrap();
            assert_eq!(
                env.verify(&Sha256),
                Err(EnvelopeError::Tampered),
                "byte {i}"
            );
        }
    }

    #[test]
    fn strict_length_checks() {
        let bytes = sample().encode();
        assert!(matches!(
            Envelope::decode(&bytes[..bytes.len() - 1]),
            Err(EnvelopeError::Truncated { .. })
        ));
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(
            Envelope::decode(&long),
            Err(EnvelopeError::TrailingBytes(1))
        );
        let mut magic = bytes.clone();
        magic[0] = b'X';
        assert_eq!(Envelope::decode(&magic), Err(EnvelopeError::BadMagic));
        assert_eq!(
            Envelope::decode(&[]),
            Err(EnvelopeError::Truncated {
                needed: HEADER_LEN,
                available: 0
            })
        );
    }

    #[test]
    fn huge_declared_length_is_rejected_before_allocation() {
        let mut bytes = sample().encode();
        bytes[PAYLOAD_LEN_AT..HEADER_LEN].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            Envelope::decode(&bytes),
            Err(EnvelopeError::PayloadTooLarge(u32::MAX as usize))
        );
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use crate::crypto::{KeyedSha256, Sha256};

    fn sealed_with<D: Digest>(d: &D) -> Envelope {
        Envelope::seal(
            d,
            SchemaId([3; 32]),
            1,
            EntityId::new_v7(7, [2; 10]),
            alloc::vec![9, 9],
        )
        .unwrap()
    }

    #[test]
    fn format_one_envelopes_are_refused() {
        let mut bytes = sealed_with(&Sha256).encode();
        bytes[FORMAT_AT] = 1;
        assert_eq!(
            Envelope::decode(&bytes),
            Err(EnvelopeError::UnsupportedFormat(1))
        );
    }

    #[test]
    fn envelope_names_its_digest() {
        let env = sealed_with(&Sha256);
        assert_eq!(env.digest, crate::crypto::id::SHA256);
        let back = Envelope::decode(&env.encode()).unwrap();
        assert_eq!(back.digest, crate::crypto::id::SHA256);
        back.verify(&Sha256).unwrap();
    }

    #[test]
    fn unkeyed_and_keyed_envelopes_do_not_verify_under_each_other() {
        let keyed = KeyedSha256::new([5; 32]);
        let env = sealed_with(&keyed);
        assert_eq!(
            env.verify(&Sha256),
            Err(EnvelopeError::DigestMismatch {
                stored: crate::crypto::id::HMAC_SHA256,
                expected: crate::crypto::id::SHA256,
            })
        );
        env.verify(&keyed).unwrap();
        // A different key is a forgery, reported as tampering.
        assert_eq!(
            env.verify(&KeyedSha256::new([6; 32])),
            Err(EnvelopeError::Tampered)
        );
    }

    #[test]
    fn digest_byte_is_covered_by_the_hash() {
        // Changing the stored digest id without re-sealing must not verify.
        let mut bytes = sealed_with(&KeyedSha256::new([5; 32])).encode();
        bytes[DIGEST_AT] = crate::crypto::id::SHA256;
        let env = Envelope::decode(&bytes).unwrap();
        assert!(env.verify(&Sha256).is_err());
    }
}
