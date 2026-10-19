//! Local stand-in for `tpt-primitives`.
//!
//! Models what an access check needs: an [`Intent`] (what the caller wants to do)
//! and a [`Capability`] (what the caller has been granted). The real definitions
//! live in `tpt-primitives`.

#![no_std]
#![forbid(unsafe_code)]

use core::fmt;

/// What a caller intends to do with a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Intent {
    Read,
    Write,
}

impl Intent {
    /// Parses the spelling used in `#[tpt(intent = "...")]`.
    pub fn parse(s: &str) -> Option<Intent> {
        match s {
            "read" | "Read" => Some(Intent::Read),
            "write" | "Write" => Some(Intent::Write),
            _ => None,
        }
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Intent::Read => "read",
            Intent::Write => "write",
        })
    }
}

/// A named grant, e.g. `"cad.edit"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Capability(pub &'static str);

/// Returns `true` when any granted capability matches `required`.
///
/// `None` means the field is unrestricted and always allowed.
pub fn is_granted(required: Option<Capability>, granted: &[Capability]) -> bool {
    match required {
        None => true,
        Some(need) => granted.contains(&need),
    }
}
