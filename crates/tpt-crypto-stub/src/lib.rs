//! Local stand-in for `tpt-crypto`.
//!
//! `tpt-model` only depends on the [`Digest`] trait. [`Blake3`] is the default
//! implementation and uses the pure-Rust BLAKE3 backend (no C code).

#![no_std]
#![forbid(unsafe_code)]

/// A 32-byte digest.
pub type Hash32 = [u8; 32];

/// Keyed-or-unkeyed 32-byte digest over a sequence of parts.
///
/// Implementations must be deterministic and must hash the concatenation of
/// `parts` (callers rely on this for envelope tamper checks).
pub trait Digest {
    fn digest(&self, parts: &[&[u8]]) -> Hash32;
}

/// BLAKE3 via the pure-Rust backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct Blake3;

impl Digest for Blake3 {
    fn digest(&self, parts: &[&[u8]]) -> Hash32 {
        let mut hasher = blake3::Hasher::new();
        for part in parts {
            hasher.update(part);
        }
        *hasher.finalize().as_bytes()
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
    fn parts_hash_like_concatenation() {
        let d = Blake3;
        assert_eq!(d.digest(&[b"ab", b"cd"]), d.digest(&[b"abcd"]));
        assert_ne!(d.digest(&[b"abcd"]), d.digest(&[b"abce"]));
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
