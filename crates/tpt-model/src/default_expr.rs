//! Evaluates `#[tpt(default = "...")]` expressions into IR nodes.
//!
//! Grammar (whitespace-trimmed):
//!
//! ```text
//! expr   := "null" | "None" | "true" | "false"
//!         | integer                  e.g. -3, 42      -> Int (or UInt above i64::MAX)
//!         | float                    e.g. 1.5, -0.25  -> Float
//!         | "\"" text "\""           string literal   -> Str
//!         | ident "::" ident         enum variant    -> Str(variant)
//!         | "[" "]"                  empty sequence  -> Seq([])
//!         | "{}"                     empty map       -> Map([])
//! ```
//!
//! Enum variants are stored as their name, matching the serde mapping of unit variants.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use tpt_uir_stub::UirNode;

/// A malformed default expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultError {
    pub expr: String,
    pub message: &'static str,
}

impl fmt::Display for DefaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid default `{}`: {}", self.expr, self.message)
    }
}

/// Parses and evaluates a default expression.
pub fn eval(expr: &str) -> Result<UirNode, DefaultError> {
    let s = expr.trim();
    let fail = |message: &'static str| DefaultError {
        expr: expr.to_string(),
        message,
    };

    match s {
        "null" | "None" => return Ok(UirNode::Null),
        "true" => return Ok(UirNode::Bool(true)),
        "false" => return Ok(UirNode::Bool(false)),
        "[]" => return Ok(UirNode::Seq(Vec::new())),
        "{}" => return Ok(UirNode::Map(Vec::new())),
        _ => {}
    }

    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let inner = &s[1..s.len() - 1];
        if inner.contains('"') {
            return Err(fail("unescaped quote in string literal"));
        }
        return Ok(UirNode::Str(inner.to_string()));
    }

    if let Some((ty, variant)) = s.split_once("::") {
        if is_ident(ty) && is_ident(variant) {
            return Ok(UirNode::Str(variant.to_string()));
        }
        return Err(fail("expected `Type::Variant`"));
    }

    if looks_numeric(s) {
        if s.contains('.') {
            return s
                .parse::<f64>()
                .map(UirNode::Float)
                .map_err(|_| fail("bad float literal"));
        }
        if let Ok(v) = s.parse::<i64>() {
            return Ok(UirNode::Int(v));
        }
        if let Ok(v) = s.parse::<u64>() {
            return Ok(UirNode::UInt(v));
        }
        return Err(fail("integer out of range"));
    }

    Err(fail("unrecognised expression"))
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn looks_numeric(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    !body.is_empty()
        && body.chars().next().is_some_and(|c| c.is_ascii_digit())
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '-' || c == '+')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        assert_eq!(eval("None").unwrap(), UirNode::Null);
        assert_eq!(eval(" true ").unwrap(), UirNode::Bool(true));
        assert_eq!(eval("42").unwrap(), UirNode::Int(42));
        assert_eq!(eval("-7").unwrap(), UirNode::Int(-7));
        assert_eq!(eval("1.5").unwrap(), UirNode::Float(1.5));
        assert_eq!(
            eval("\"hi there\"").unwrap(),
            UirNode::Str("hi there".into())
        );
        assert_eq!(
            eval("Material::Titanium").unwrap(),
            UirNode::Str("Titanium".into())
        );
        assert_eq!(eval("[]").unwrap(), UirNode::Seq(Vec::new()));
    }

    #[test]
    fn rejects_garbage() {
        assert!(eval("").is_err());
        assert!(eval("1.2.3.4").is_err());
        assert!(eval("\"a\"b\"").is_err());
        assert!(eval("Foo::").is_err());
        assert!(eval("hello world").is_err());
        assert!(eval("99999999999999999999999").is_err());
    }
}
