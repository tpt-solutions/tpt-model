//! `ModelEnvelope`: the wrapper stored on disk or sent over the wire.
//!
//! Layout (all integers little-endian):
//!
//! ```text
//! magic        4   "TPTM"
//! format       1   1
//! schema_id   32
//! version      4
//! entity_id   16
//! payload_len  4
//! payload      payload_len
//! hash        32   digest(header ++ payload), from the Digest implementation
//! ```

use crate::entity::EntityId;
use crate::hash::SchemaId;
use alloc::vec::Vec;
use core::fmt;
use tpt_crypto_stub::{ct_eq, Digest, Hash32};

pub const MAGIC: [u8; 4] = *b"TPTM";
pub const FORMAT: u8 = 1;
/// Largest payload accepted by [`Envelope::decode`].
pub const MAX_PAYLOAD: usize = 64 * 1024 * 1024;
const HEADER_LEN: usize = 4 + 1 + 32 + 4 + 16 + 4;
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
        }
    }
}

/// The name the design spec uses for [`Envelope`].
pub type ModelEnvelope = Envelope;

/// A sealed entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
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
        let header = header_bytes(schema_id, version, entity_id, payload.len());
        let hash = digest.digest(&[&header, &payload]);
        Ok(Envelope {
            schema_id,
            version,
            entity_id,
            payload,
            hash,
        })
    }

    /// Serializes the envelope.
    pub fn encode(&self) -> Vec<u8> {
        let header = header_bytes(
            self.schema_id,
            self.version,
            self.entity_id,
            self.payload.len(),
        );
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
        if bytes[4] != FORMAT {
            return Err(EnvelopeError::UnsupportedFormat(bytes[4]));
        }
        let mut schema = [0u8; 32];
        schema.copy_from_slice(&bytes[5..37]);
        let version = u32::from_le_bytes([bytes[37], bytes[38], bytes[39], bytes[40]]);
        let mut entity = [0u8; 16];
        entity.copy_from_slice(&bytes[41..57]);
        let payload_len = u32::from_le_bytes([bytes[57], bytes[58], bytes[59], bytes[60]]) as usize;
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
            schema_id: SchemaId(schema),
            version,
            entity_id: EntityId::from_bytes(entity),
            payload,
            hash,
        })
    }

    /// Recomputes the hash and compares it in constant time.
    pub fn verify<D: Digest>(&self, digest: &D) -> Result<(), EnvelopeError> {
        let header = header_bytes(
            self.schema_id,
            self.version,
            self.entity_id,
            self.payload.len(),
        );
        let expected = digest.digest(&[&header, &self.payload]);
        if ct_eq(&expected, &self.hash) {
            Ok(())
        } else {
            Err(EnvelopeError::Tampered)
        }
    }
}

fn header_bytes(
    schema_id: SchemaId,
    version: u32,
    entity_id: EntityId,
    payload_len: usize,
) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[0..4].copy_from_slice(&MAGIC);
    h[4] = FORMAT;
    h[5..37].copy_from_slice(schema_id.as_bytes());
    h[37..41].copy_from_slice(&version.to_le_bytes());
    h[41..57].copy_from_slice(entity_id.as_bytes());
    h[57..61].copy_from_slice(&(payload_len as u32).to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_crypto_stub::Blake3;

    fn sample() -> Envelope {
        Envelope::seal(
            &Blake3,
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
        back.verify(&Blake3).unwrap();
    }

    #[test]
    fn any_flipped_byte_is_detected() {
        let bytes = sample().encode();
        for i in HEADER_LEN..bytes.len() - TRAILER_LEN {
            let mut bad = bytes.clone();
            bad[i] ^= 0x01;
            let env = Envelope::decode(&bad).unwrap();
            assert_eq!(
                env.verify(&Blake3),
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
        bytes[57..61].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            Envelope::decode(&bytes),
            Err(EnvelopeError::PayloadTooLarge(u32::MAX as usize))
        );
    }
}
