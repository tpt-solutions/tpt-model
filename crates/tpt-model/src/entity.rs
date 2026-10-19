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

impl EntityId {
    /// Builds an id from raw bytes. The caller is responsible for the version bits.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        EntityId(Uuid::from_bytes(bytes))
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
                f.write_str("16 bytes")
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<EntityId, E> {
                let bytes: [u8; 16] = v
                    .try_into()
                    .map_err(|_| E::invalid_length(v.len(), &"16 bytes"))?;
                Ok(EntityId::from_bytes(bytes))
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
}
