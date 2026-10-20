//! Local stand-in for `tpt-uir`.
//!
//! Provides the IR tree ([`UirNode`]) that `tpt-model` migrates, plus a
//! strict, bounds-checked byte encoding. The real encoding belongs to
//! `tpt-uir`; this stub only has to be deterministic and safe on garbage input.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Maximum nesting depth accepted by [`decode`].
pub const MAX_DEPTH: usize = 64;
/// Maximum length of any string, byte string, sequence or map accepted by [`decode`].
pub const MAX_LEN: usize = 16 * 1024 * 1024;
/// Largest up-front allocation for a sequence or map. Larger collections still decode;
/// their storage grows as the elements arrive, so a short input cannot reserve much memory.
const PREALLOC_LIMIT: usize = 1024;

/// A node of the intermediate representation tree.
///
/// Maps keep insertion order; equality therefore depends on order. Use
/// [`UirNode::canonicalize`] when an order-independent form is needed.
#[derive(Debug, Clone, PartialEq)]
pub enum UirNode {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Seq(Vec<UirNode>),
    Map(Vec<(String, UirNode)>),
}

impl UirNode {
    /// Returns the value of `key` when this node is a map.
    pub fn get(&self, key: &str) -> Option<&UirNode> {
        match self {
            UirNode::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Removes and returns the value of `key` when this node is a map.
    pub fn remove(&mut self, key: &str) -> Option<UirNode> {
        match self {
            UirNode::Map(entries) => {
                let idx = entries.iter().position(|(k, _)| k == key)?;
                Some(entries.remove(idx).1)
            }
            _ => None,
        }
    }

    /// Inserts or replaces `key` when this node is a map. Returns `false` if this is not a map.
    pub fn insert(&mut self, key: &str, value: UirNode) -> bool {
        match self {
            UirNode::Map(entries) => {
                if let Some(slot) = entries.iter_mut().find(|(k, _)| k == key) {
                    slot.1 = value;
                } else {
                    entries.push((String::from(key), value));
                }
                true
            }
            _ => false,
        }
    }

    /// Returns a copy with map keys sorted, recursively.
    pub fn canonicalize(&self) -> UirNode {
        match self {
            UirNode::Seq(items) => UirNode::Seq(items.iter().map(UirNode::canonicalize).collect()),
            UirNode::Map(entries) => {
                let mut sorted: Vec<(String, UirNode)> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.canonicalize()))
                    .collect();
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                UirNode::Map(sorted)
            }
            other => other.clone(),
        }
    }
}

/// Errors produced by [`decode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UirError {
    /// Input ended before a complete node was read.
    Truncated { offset: usize },
    /// An unknown tag byte was found.
    BadTag { offset: usize, tag: u8 },
    /// A length prefix exceeded [`MAX_LEN`] or the remaining input.
    BadLength { offset: usize },
    /// Nesting exceeded [`MAX_DEPTH`].
    TooDeep { offset: usize },
    /// A string was not valid UTF-8.
    BadUtf8 { offset: usize },
    /// A boolean payload was neither 0 nor 1.
    BadBool { offset: usize },
    /// Bytes remained after the root node.
    TrailingBytes { offset: usize },
}

impl fmt::Display for UirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UirError::Truncated { offset } => write!(f, "truncated input at byte {offset}"),
            UirError::BadTag { offset, tag } => write!(f, "bad tag 0x{tag:02x} at byte {offset}"),
            UirError::BadLength { offset } => write!(f, "bad length at byte {offset}"),
            UirError::TooDeep { offset } => write!(f, "nesting too deep at byte {offset}"),
            UirError::BadUtf8 { offset } => write!(f, "invalid utf-8 at byte {offset}"),
            UirError::BadBool { offset } => write!(f, "invalid bool at byte {offset}"),
            UirError::TrailingBytes { offset } => write!(f, "trailing bytes at byte {offset}"),
        }
    }
}

const TAG_NULL: u8 = 0x00;
const TAG_FALSE: u8 = 0x01;
const TAG_TRUE: u8 = 0x02;
const TAG_INT: u8 = 0x03;
const TAG_UINT: u8 = 0x04;
const TAG_FLOAT: u8 = 0x05;
const TAG_STR: u8 = 0x06;
const TAG_BYTES: u8 = 0x07;
const TAG_SEQ: u8 = 0x08;
const TAG_MAP: u8 = 0x09;

/// Encodes a node as tag-length-value bytes. Output is deterministic for a given tree.
pub fn encode(node: &UirNode) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(node, &mut out);
    out
}

fn put_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u32).to_le_bytes());
}

fn encode_into(node: &UirNode, out: &mut Vec<u8>) {
    match node {
        UirNode::Null => out.push(TAG_NULL),
        UirNode::Bool(false) => out.push(TAG_FALSE),
        UirNode::Bool(true) => out.push(TAG_TRUE),
        UirNode::Int(v) => {
            out.push(TAG_INT);
            out.extend_from_slice(&v.to_le_bytes());
        }
        UirNode::UInt(v) => {
            out.push(TAG_UINT);
            out.extend_from_slice(&v.to_le_bytes());
        }
        UirNode::Float(v) => {
            out.push(TAG_FLOAT);
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        UirNode::Str(s) => {
            out.push(TAG_STR);
            put_len(out, s.len());
            out.extend_from_slice(s.as_bytes());
        }
        UirNode::Bytes(b) => {
            out.push(TAG_BYTES);
            put_len(out, b.len());
            out.extend_from_slice(b);
        }
        UirNode::Seq(items) => {
            out.push(TAG_SEQ);
            put_len(out, items.len());
            for item in items {
                encode_into(item, out);
            }
        }
        UirNode::Map(entries) => {
            out.push(TAG_MAP);
            put_len(out, entries.len());
            for (k, v) in entries {
                put_len(out, k.len());
                out.extend_from_slice(k.as_bytes());
                encode_into(v, out);
            }
        }
    }
}

/// Decodes a complete node. Never panics; every malformed input yields a [`UirError`].
pub fn decode(bytes: &[u8]) -> Result<UirNode, UirError> {
    let mut reader = Reader { bytes, pos: 0 };
    let node = reader.node(0)?;
    if reader.pos != bytes.len() {
        return Err(UirError::TrailingBytes { offset: reader.pos });
    }
    Ok(node)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], UirError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(UirError::Truncated { offset: self.pos })?;
        if end > self.bytes.len() {
            return Err(UirError::Truncated { offset: self.pos });
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], UirError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn len(&mut self) -> Result<usize, UirError> {
        let at = self.pos;
        let n = u32::from_le_bytes(self.array::<4>()?) as usize;
        if n > MAX_LEN {
            return Err(UirError::BadLength { offset: at });
        }
        Ok(n)
    }

    fn string(&mut self) -> Result<String, UirError> {
        let at = self.pos;
        let n = self.len()?;
        let raw = self
            .take(n)
            .map_err(|_| UirError::BadLength { offset: at })?;
        String::from_utf8(raw.to_vec()).map_err(|_| UirError::BadUtf8 { offset: at })
    }

    fn node(&mut self, depth: usize) -> Result<UirNode, UirError> {
        if depth > MAX_DEPTH {
            return Err(UirError::TooDeep { offset: self.pos });
        }
        let at = self.pos;
        let tag = self.take(1)?[0];
        Ok(match tag {
            TAG_NULL => UirNode::Null,
            TAG_FALSE => UirNode::Bool(false),
            TAG_TRUE => UirNode::Bool(true),
            TAG_INT => UirNode::Int(i64::from_le_bytes(self.array::<8>()?)),
            TAG_UINT => UirNode::UInt(u64::from_le_bytes(self.array::<8>()?)),
            TAG_FLOAT => UirNode::Float(f64::from_bits(u64::from_le_bytes(self.array::<8>()?))),
            TAG_STR => UirNode::Str(self.string()?),
            TAG_BYTES => {
                let n = self.len()?;
                UirNode::Bytes(
                    self.take(n)
                        .map_err(|_| UirError::BadLength { offset: at })?
                        .to_vec(),
                )
            }
            TAG_SEQ => {
                let n = self.len()?;
                // Each element needs at least one byte, so a larger count cannot be satisfied.
                if n > self.bytes.len() - self.pos {
                    return Err(UirError::BadLength { offset: at });
                }
                let mut items = Vec::with_capacity(n.min(PREALLOC_LIMIT));
                for _ in 0..n {
                    items.push(self.node(depth + 1)?);
                }
                UirNode::Seq(items)
            }
            TAG_MAP => {
                let n = self.len()?;
                // Each entry needs at least a key length and a value tag.
                if n.saturating_mul(5) > self.bytes.len() - self.pos {
                    return Err(UirError::BadLength { offset: at });
                }
                let mut entries = Vec::with_capacity(n.min(PREALLOC_LIMIT));
                for _ in 0..n {
                    let key = self.string()?;
                    let value = self.node(depth + 1)?;
                    entries.push((key, value));
                }
                UirNode::Map(entries)
            }
            _ => return Err(UirError::BadTag { offset: at, tag }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sample() -> UirNode {
        UirNode::Map(vec![
            ("n".into(), UirNode::Int(-2)),
            ("s".into(), UirNode::Str("hi".into())),
            ("b".into(), UirNode::Bytes(vec![0xde, 0xad])),
            (
                "l".into(),
                UirNode::Seq(vec![UirNode::Bool(true), UirNode::Null]),
            ),
        ])
    }

    /// Pins the exact bytes. Any change here changes every stored entity, so it must be deliberate.
    #[test]
    fn encoding_is_pinned() {
        let expected: &[u8] = &[
            0x09, 4, 0, 0, 0, // map, 4 entries
            1, 0, 0, 0, b'n', 0x03, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, // "n": Int(-2)
            1, 0, 0, 0, b's', 0x06, 2, 0, 0, 0, b'h', b'i', // "s": Str("hi")
            1, 0, 0, 0, b'b', 0x07, 2, 0, 0, 0, 0xde, 0xad, // "b": Bytes
            1, 0, 0, 0, b'l', 0x08, 2, 0, 0, 0, 0x02, 0x00, // "l": Seq[true, null]
        ];
        assert_eq!(encode(&sample()), expected);
    }

    #[test]
    fn round_trip() {
        assert_eq!(decode(&encode(&sample())).unwrap(), sample());
    }

    #[test]
    fn collections_larger_than_the_preallocation_limit_round_trip() {
        let n = PREALLOC_LIMIT * 4;
        let seq = UirNode::Seq(vec![UirNode::Null; n]);
        assert_eq!(decode(&encode(&seq)).unwrap(), seq);
        let map = UirNode::Map(
            (0..n)
                .map(|i| (alloc::format!("k{i}"), UirNode::UInt(i as u64)))
                .collect(),
        );
        assert_eq!(decode(&encode(&map)).unwrap(), map);
    }

    #[test]
    fn rejects_bad_input_without_panicking() {
        assert_eq!(decode(&[]), Err(UirError::Truncated { offset: 0 }));
        assert_eq!(
            decode(&[0x7f]),
            Err(UirError::BadTag {
                offset: 0,
                tag: 0x7f
            })
        );
        assert_eq!(
            decode(&[0x00, 0x00]),
            Err(UirError::TrailingBytes { offset: 1 })
        );
        // A sequence claiming a billion elements in a few bytes.
        assert!(matches!(
            decode(&[0x08, 0, 0, 0, 0x40]),
            Err(UirError::BadLength { .. })
        ));
        // Nesting deeper than MAX_DEPTH.
        let mut deep = vec![0x08, 1, 0, 0, 0];
        for _ in 0..MAX_DEPTH + 2 {
            deep.extend_from_slice(&[0x08, 1, 0, 0, 0]);
        }
        deep.push(0x00);
        assert!(matches!(decode(&deep), Err(UirError::TooDeep { .. })));
    }
}
