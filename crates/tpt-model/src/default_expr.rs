//! Evaluates `#[tpt(default = "...")]` expressions into IR nodes.
//!
//! Grammar (whitespace-trimmed):
//!
//! ```text
//! expr   := "null" | "None" | "true" | "false"
//!         | integer                  e.g. -3, 42      -> Int (or UInt above i64::MAX)
//!         | float                    e.g. 1.5, -0.25, 1e5 -> Float
//!         | "\"" text "\""           string literal   -> Str
//!         | path "::" ident          enum variant     -> Str(variant)
//!         | "[" "]"                  empty sequence   -> Seq([])
//!         | "{}"                     empty map        -> Map([])
//! ```
//!
//! String literals accept the escapes `\\`, `\"`, `\n`, `\r` and `\t`. Any other escape is
//! an error. A path such as `crate::Material::Steel` yields its last segment, `Steel`.
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
        return unescape(&s[1..s.len() - 1]).map(UirNode::Str).map_err(fail);
    }

    if s.contains("::") {
        let segments: Vec<&str> = s.split("::").collect();
        if segments.len() >= 2 && segments.iter().all(|seg| is_ident(seg)) {
            let variant = segments[segments.len() - 1];
            return Ok(UirNode::Str(variant.to_string()));
        }
        return Err(fail("expected `Type::Variant` or `path::Type::Variant`"));
    }

    if looks_numeric(s) {
        // Exponent notation is a float, so it must not reach the integer parse below.
        if s.contains(['.', 'e', 'E']) {
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

/// Resolves the body of a string literal (without its quotes).
fn unescape(inner: &str) -> Result<String, &'static str> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Err("unescaped quote in string literal"),
            '\\' => out.push(match chars.next() {
                Some('\\') => '\\',
                Some('"') => '"',
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                _ => {
                    return Err("unknown escape in string literal; use \\\\, \\\", \\n, \\r or \\t")
                }
            }),
            other => out.push(other),
        }
    }
    Ok(out)
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
    fn string_escapes_are_resolved() {
        assert_eq!(
            eval(r#""a\"b\\c\n\t""#).unwrap(),
            UirNode::Str("a\"b\\c\n\t".into())
        );
        assert!(eval(r#""bad \x""#).is_err());
        assert!(eval(r#""dangling\""#).is_err());
    }

    #[test]
    fn exponent_notation_is_a_float() {
        assert_eq!(eval("1e5").unwrap(), UirNode::Float(100_000.0));
        assert_eq!(eval("-2.5E-1").unwrap(), UirNode::Float(-0.25));
        assert_eq!(
            eval("1e").unwrap_err().message,
            "bad float literal",
            "a broken exponent is reported as a float, not as an out-of-range integer"
        );
    }

    #[test]
    fn qualified_variant_paths_yield_the_variant() {
        assert_eq!(
            eval("crate::Material::Steel").unwrap(),
            UirNode::Str("Steel".into())
        );
        assert!(eval("crate::::Steel").is_err());
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
