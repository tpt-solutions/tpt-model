//! Stable entity identifiers (UUIDv7).

use core::fmt;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A time-sortable, globally unique entity id.
///
/// Byte-wise ordering of the 16 bytes matches creation order for UUIDv7 values
/// created from the same clock.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(Uuid);

/// Bytes that are not a UUIDv7 (wrong version nibble or variant bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotUuidV7;

impl fmt::Display for NotUuidV7 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a UUIDv7 (version 7, RFC 4122 variant)")
    }
}

impl EntityId {
    /// Builds an id from raw bytes without checking them. Prefer [`EntityId::try_from_bytes`]
    /// for bytes that come from outside the program.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        EntityId(Uuid::from_bytes(bytes))
    }

    /// Builds an id from raw bytes, accepting only a UUIDv7.
    pub fn try_from_bytes(bytes: [u8; 16]) -> Result<Self, NotUuidV7> {
        let id = EntityId(Uuid::from_bytes(bytes));
        if id.0.get_version_num() == 7 && id.0.get_variant() == uuid::Variant::RFC4122 {
            Ok(id)
        } else {
            Err(NotUuidV7)
        }
    }

    /// Builds a UUIDv7 from a millisecond Unix timestamp and 10 bytes of randomness.
    ///
    /// The crate has no random source of its own (it is `no_std`), so the caller supplies
    /// the random bytes. The timestamp uses the low 48 bits.
    pub fn new_v7(unix_millis: u64, random: [u8; 10]) -> Self {
        let mut b = [0u8; 16];
        let ts = unix_millis.to_be_bytes();
        b[0..6].copy_from_slice(&ts[2..8]);
        b[6] = 0x70 | (random[0] & 0x0f);
        b[7] = random[1];
        b[8] = 0x80 | (random[2] & 0x3f);
        b[9..16].copy_from_slice(&random[3..10]);
        EntityId(Uuid::from_bytes(b))
    }

    /// Generates a new UUIDv7 for `unix_millis`, drawing the random part from `fill`.
    ///
    /// `fill` must return random bytes (for example from the platform's CSPRNG). It is called
    /// ten times, and the crate does not read a clock itself.
    pub fn generate(unix_millis: u64, mut fill: impl FnMut() -> u8) -> Self {
        let mut random = [0u8; 10];
        for byte in &mut random {
            *byte = fill();
        }
        Self::new_v7(unix_millis, random)
    }

    /// Millisecond timestamp, if this is a UUIDv7.
    pub fn timestamp_ms(&self) -> Option<u64> {
        let b = self.0.as_bytes();
        if b[6] >> 4 != 7 {
            return None;
        }
        let mut ts = [0u8; 8];
        ts[2..8].copy_from_slice(&b[0..6]);
        Some(u64::from_be_bytes(ts))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityId({})", self.0)
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for EntityId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdVisitor;

        impl<'de> Visitor<'de> for IdVisitor {
            type Value = EntityId;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a UUIDv7 as 16 bytes")
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<EntityId, E> {
                let bytes: [u8; 16] = v
                    .try_into()
                    .map_err(|_| E::invalid_length(v.len(), &"16 bytes"))?;
                EntityId::try_from_bytes(bytes).map_err(E::custom)
            }

            // Self-describing formats such as JSON write bytes as a sequence of numbers.
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<EntityId, A::Error> {
                let mut bytes = [0u8; 16];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = seq
                        .next_element::<u8>()?
                        .ok_or_else(|| de::Error::invalid_length(i, &"16 bytes"))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(de::Error::invalid_length(17, &"16 bytes"));
                }
                EntityId::try_from_bytes(bytes).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_bytes(IdVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v7_layout_and_timestamp() {
        let id = EntityId::new_v7(1_700_000_000_123, [0xff; 10]);
        assert_eq!(id.timestamp_ms(), Some(1_700_000_000_123));
        assert_eq!(id.as_uuid().get_version_num(), 7);
        assert_eq!(id.as_uuid().get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn ids_sort_by_time() {
        let early = EntityId::new_v7(1_000, [0xff; 10]);
        let late = EntityId::new_v7(2_000, [0x00; 10]);
        assert!(early < late);
    }

    #[test]
    fn non_v7_has_no_timestamp() {
        assert_eq!(EntityId::from_bytes([0; 16]).timestamp_ms(), None);
    }

    #[test]
    fn try_from_bytes_accepts_only_v7() {
        let v7 = EntityId::new_v7(1_700_000_000_000, [1; 10]);
        assert_eq!(EntityId::try_from_bytes(*v7.as_bytes()), Ok(v7));
        assert_eq!(EntityId::try_from_bytes([0; 16]), Err(NotUuidV7));
        let mut v4 = *v7.as_bytes();
        v4[6] = 0x40 | (v4[6] & 0x0f);
        assert_eq!(EntityId::try_from_bytes(v4), Err(NotUuidV7));
    }

    #[test]
    fn generate_draws_ten_bytes_and_stamps_the_time() {
        let mut counter = 0u8;
        let id = EntityId::generate(1_700_000_000_123, || {
            counter += 1;
            counter
        });
        assert_eq!(counter, 10);
        assert_eq!(id.timestamp_ms(), Some(1_700_000_000_123));
        assert!(EntityId::try_from_bytes(*id.as_bytes()).is_ok());
    }

    #[test]
    fn deserializes_from_a_self_describing_sequence_of_bytes() {
        use serde::de::value::{Error, SeqDeserializer};
        let id = EntityId::new_v7(1_700_000_000_000, [7; 10]);
        let seq = SeqDeserializer::<_, Error>::new(id.as_bytes().iter().copied());
        assert_eq!(EntityId::deserialize(seq), Ok(id));

        let short = SeqDeserializer::<_, Error>::new([0u8; 4].into_iter());
        assert!(EntityId::deserialize(short).is_err());
        let not_v7 = SeqDeserializer::<_, Error>::new([0u8; 16].into_iter());
        assert!(EntityId::deserialize(not_v7).is_err());
    }
}
