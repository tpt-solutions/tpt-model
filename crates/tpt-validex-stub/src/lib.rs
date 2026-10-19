//! Local stand-in for `tpt-validex`.
//!
//! The real validator runs over raw bytes against a schema description. The
//! stub provides the same shape: a [`Validator`] trait and a [`BoundsValidator`]
//! that checks that the bytes are a well-formed IR encoding within limits.

#![no_std]
#![forbid(unsafe_code)]

use core::fmt;
use tpt_uir_stub::{decode, UirError};

/// Identifies the schema a byte payload is supposed to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedSchema<'a> {
    pub name: &'a str,
    pub version: u32,
}

/// Why a payload was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Payload is larger than the validator allows.
    TooLarge { len: usize, max: usize },
    /// Payload is not a well-formed encoding.
    Malformed(UirError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::TooLarge { len, max } => {
                write!(f, "payload of {len} bytes exceeds limit of {max}")
            }
            ValidationError::Malformed(e) => write!(f, "malformed payload: {e}"),
        }
    }
}

/// Validates raw payload bytes before they are decoded.
pub trait Validator {
    fn validate(&self, expected: ExpectedSchema<'_>, bytes: &[u8]) -> Result<(), ValidationError>;
}

/// Checks size and well-formedness. Does not check field types; that is the
/// schema layer's job in `tpt-model`.
#[derive(Debug, Clone, Copy)]
pub struct BoundsValidator {
    pub max_bytes: usize,
}

impl Default for BoundsValidator {
    fn default() -> Self {
        BoundsValidator {
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

impl Validator for BoundsValidator {
    fn validate(&self, _expected: ExpectedSchema<'_>, bytes: &[u8]) -> Result<(), ValidationError> {
        if bytes.len() > self.max_bytes {
            return Err(ValidationError::TooLarge {
                len: bytes.len(),
                max: self.max_bytes,
            });
        }
        decode(bytes)
            .map(|_| ())
            .map_err(ValidationError::Malformed)
    }
}
