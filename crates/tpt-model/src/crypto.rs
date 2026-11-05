//! Digests for envelopes: the [`Digest`] trait, unkeyed [`Sha256`] and keyed [`KeyedSha256`] (HMAC-SHA256).
//!
//! Both use pure-Rust backends. Unkeyed SHA-256 detects corruption only; the keyed
//! digest also detects deliberate tampering.

use hmac::{Hmac, Mac};
use sha2::{Digest as _, Sha256 as Sha256Hasher};

/// A 32-byte digest.
pub type Hash32 = [u8; 32];

/// Identifies a digest algorithm in stored envelopes.
pub mod id {
    /// Unkeyed SHA-256, as [`Sha256`](crate::Sha256) computes it.
    pub const SHA256: u8 = 1;
    /// HMAC-SHA256 with a secret key, as [`KeyedSha256`](crate::KeyedSha256) computes it.
    pub const HMAC_SHA256: u8 = 2;
}

/// Keyed-or-unkeyed 32-byte digest over a sequence of parts.
///
/// Implementations must be deterministic and must hash the concatenation of
/// `parts` (callers rely on this for envelope tamper checks). [`Digest::id`]
/// names the algorithm, and it is stored in every envelope.
pub trait Digest {
    /// Computes the digest of the concatenation of `parts`.
    fn digest(&self, parts: &[&[u8]]) -> Hash32;

    /// The algorithm identifier, one of the constants in [`id`].
    fn id(&self) -> u8;
}

/// Unkeyed SHA-256 (pure-Rust backend).
#[derive(Debug, Default, Clone, Copy)]
pub struct Sha256;

impl Digest for Sha256 {
    fn digest(&self, parts: &[&[u8]]) -> Hash32 {
        let mut hasher = Sha256Hasher::new();
        for part in parts {
            hasher.update(part);
        }
        hasher.finalize().into()
    }

    fn id(&self) -> u8 {
        id::SHA256
    }
}

/// HMAC-SHA256 (pure-Rust backend).
///
/// Without the key nobody can produce a matching hash, so this detects deliberate tampering
/// as well as corruption. [`Sha256`] only detects corruption: anyone who can rewrite a payload
/// can also recompute its hash.
#[derive(Clone)]
pub struct KeyedSha256 {
    key: [u8; 32],
}

impl KeyedSha256 {
    /// Creates a keyed digest. The key is copied; keep it secret.
    pub const fn new(key: [u8; 32]) -> Self {
        KeyedSha256 { key }
    }
}

impl core::fmt::Debug for KeyedSha256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The key never appears in debug output or logs.
        f.write_str("KeyedSha256(<key redacted>)")
    }
}

impl Digest for KeyedSha256 {
    fn digest(&self, parts: &[&[u8]]) -> Hash32 {
        // HMAC accepts keys of any length, so construction cannot fail.
        let mut mac = <Hmac<Sha256Hasher> as Mac>::new_from_slice(&self.key)
            .expect("HMAC accepts keys of any length");
        for part in parts {
            mac.update(part);
        }
        mac.finalize().into_bytes().into()
    }

    fn id(&self) -> u8 {
        id::HMAC_SHA256
    }
}

/// Compares two digests without early exit on the first differing byte.
pub fn ct_eq(a: &Hash32, b: &Hash32) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("abc")
        let expected: Hash32 = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(Sha256.digest(&[b"abc"]), expected);
    }

    #[test]
    fn parts_hash_like_concatenation() {
        let d = Sha256;
        assert_eq!(d.digest(&[b"ab", b"cd"]), d.digest(&[b"abcd"]));
        assert_ne!(d.digest(&[b"abcd"]), d.digest(&[b"abce"]));
        let k = KeyedSha256::new([9; 32]);
        assert_eq!(k.digest(&[b"ab", b"cd"]), k.digest(&[b"abcd"]));
    }

    #[test]
    fn keyed_digest_depends_on_key() {
        let a = KeyedSha256::new([1; 32]).digest(&[b"payload"]);
        let b = KeyedSha256::new([2; 32]).digest(&[b"payload"]);
        assert_ne!(a, b);
        assert_ne!(a, Sha256.digest(&[b"payload"]));
    }

    #[test]
    fn ids_name_the_algorithm() {
        assert_eq!(Sha256.id(), id::SHA256);
        assert_eq!(KeyedSha256::new([0; 32]).id(), id::HMAC_SHA256);
    }

    #[test]
    fn ct_eq_works() {
        let a = [1u8; 32];
        let mut b = a;
        assert!(ct_eq(&a, &b));
        b[31] ^= 1;
        assert!(!ct_eq(&a, &b));
    }
}
