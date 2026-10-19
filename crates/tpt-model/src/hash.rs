//! Schema identity hashing.
//!
//! `SchemaId` must be a `const` (the `Model` trait declares `const SCHEMA_ID`), so
//! the hash is a small sponge-style construction written as `const fn`. It is
//! deterministic and platform independent. It is an identifier, not a security
//! primitive: tamper detection uses the digest trait from `tpt-crypto`.
//!
//! Field and variant digests are combined with wrapping addition, which is
//! commutative. That gives order-normalized encoding without sorting in a
//! `const` context.

use core::fmt;

/// Four 64-bit lanes of sponge state.
pub type Lanes = [u64; 4];

/// Identifier of a schema version. Two versions with the same structure share an id.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaId(pub [u8; 32]);

impl SchemaId {
    /// Computes the id of a schema. Usable in `const` context.
    pub const fn of(schema: &crate::schema::Schema) -> SchemaId {
        SchemaId(finish(crate::schema::schema_digest(schema)))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SchemaId({self})")
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

const K0: u64 = 0x243f_6a88_85a3_08d3;
const K1: u64 = 0x1319_8a2e_0370_7344;
const K2: u64 = 0xa409_3822_299f_31d0;
const K3: u64 = 0x082e_fa98_ec4e_6c89;

/// Absorbs one byte into the state.
const fn mix(s: Lanes, b: u8) -> Lanes {
    let x = (b as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    let s0 = (s[0] ^ x ^ s[3])
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left(27);
    let s1 = (s[1] ^ s0)
        .wrapping_mul(0xbf58_476d_1ce4_e5b9)
        .rotate_left(31);
    let s2 = (s[2].wrapping_add(s1))
        .wrapping_mul(0x94d0_49bb_1331_11eb)
        .rotate_left(29);
    let s3 = (s[3] ^ s2)
        .wrapping_mul(0xd6e8_feb8_6659_fd93)
        .rotate_left(23)
        ^ s0;
    [s0, s1, s2, s3]
}

/// Starts a fresh digest under a domain-separation tag.
pub const fn seed(tag: u8) -> Lanes {
    mix([K0, K1, K2, K3], tag)
}

pub const fn absorb_bytes(s: Lanes, bytes: &[u8]) -> Lanes {
    let mut s = s;
    let mut i = 0;
    while i < bytes.len() {
        s = mix(s, bytes[i]);
        i += 1;
    }
    s
}

pub const fn absorb_u64(s: Lanes, v: u64) -> Lanes {
    absorb_bytes(s, &v.to_le_bytes())
}

/// Absorbs a length-prefixed string so that concatenations cannot collide.
pub const fn absorb_str(s: Lanes, v: &str) -> Lanes {
    let s = absorb_u64(s, v.len() as u64);
    absorb_bytes(s, v.as_bytes())
}

pub const fn absorb_opt_str(s: Lanes, v: Option<&str>) -> Lanes {
    match v {
        None => absorb_bytes(s, &[0]),
        Some(v) => absorb_str(absorb_bytes(s, &[1]), v),
    }
}

pub const fn absorb_opt_u32(s: Lanes, v: Option<u32>) -> Lanes {
    match v {
        None => absorb_bytes(s, &[0]),
        Some(v) => absorb_u64(absorb_bytes(s, &[1]), v as u64),
    }
}

/// Lane-wise wrapping addition. Commutative, so the order of inputs does not matter.
pub const fn combine(a: Lanes, b: Lanes) -> Lanes {
    [
        a[0].wrapping_add(b[0]),
        a[1].wrapping_add(b[1]),
        a[2].wrapping_add(b[2]),
        a[3].wrapping_add(b[3]),
    ]
}

pub const fn absorb_lanes(s: Lanes, l: Lanes) -> Lanes {
    let s = absorb_u64(s, l[0]);
    let s = absorb_u64(s, l[1]);
    let s = absorb_u64(s, l[2]);
    absorb_u64(s, l[3])
}

/// Squeezes the state into 32 bytes.
pub const fn finish(s: Lanes) -> [u8; 32] {
    let mut s = s;
    let mut i = 0u8;
    while i < 8 {
        s = mix(s, i);
        i += 1;
    }
    let a = s[0].to_le_bytes();
    let b = s[1].to_le_bytes();
    let c = s[2].to_le_bytes();
    let d = s[3].to_le_bytes();
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], //
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], //
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], //
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], //
    ]
}
