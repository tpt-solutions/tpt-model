//! JSON and YAML export of schemas, for `tpt-validex` and golden-file tests.
//!
//! Output is deterministic: keys appear in a fixed order and nothing depends on the
//! host, so exports can be compared byte-for-byte.

use crate::registry::Registry;
use crate::schema::{Field, Schema, TypeDef, TypeRef, Variant};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Format-neutral value tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(&'static str, Value)>),
}

fn opt_u32(v: Option<u32>) -> Value {
    match v {
        Some(n) => Value::Int(n as i64),
        None => Value::Null,
    }
}

fn opt_str(v: Option<&str>) -> Value {
    match v {
        Some(s) => Value::Str(s.to_string()),
        None => Value::Null,
    }
}

fn type_value(t: &TypeRef) -> Value {
    Value::Str(t.describe())
}

fn field_value(f: &Field) -> Value {
    let access = match f.access {
        Some(a) => Value::Map(alloc::vec![
            ("intent", Value::Str(a.intent.to_string())),
            ("capability", opt_str(a.capability)),
        ]),
        None => Value::Null,
    };
    Value::Map(alloc::vec![
        ("name", Value::Str(f.name.to_string())),
        ("type", type_value(&f.ty)),
        ("since", Value::Int(f.since as i64)),
        ("removed_in", opt_u32(f.removed_in)),
        ("default", opt_str(f.default)),
        (
            "renames",
            Value::List(
                f.renames
                    .iter()
                    .map(|r| Value::Str(r.to_string()))
                    .collect()
            )
        ),
        ("access", access),
        ("migrate", opt_str(f.migrate)),
        (
            "constraints",
            Value::List(
                f.constraints
                    .iter()
                    .map(|c| Value::Str(c.to_string()))
                    .collect()
            )
        ),
    ])
}

fn variant_value(v: &Variant) -> Value {
    Value::Map(alloc::vec![
        ("name", Value::Str(v.name.to_string())),
        ("since", Value::Int(v.since as i64)),
        ("removed_in", opt_u32(v.removed_in)),
        ("migrate", opt_str(v.migrate)),
        (
            "payload",
            match &v.payload {
                Some(t) => type_value(t),
                None => Value::Null,
            }
        ),
    ])
}

/// Converts a schema to a value tree.
pub fn schema_value(schema: &Schema) -> Value {
    let (kind, body) = match schema.def {
        TypeDef::Struct(fields) => (
            "struct",
            Value::List(fields.iter().map(field_value).collect()),
        ),
        TypeDef::Enum(variants) => (
            "enum",
            Value::List(variants.iter().map(variant_value).collect()),
        ),
        TypeDef::Union(members) => (
            "union",
            Value::List(members.iter().map(type_value).collect()),
        ),
    };
    let body_key = match schema.def {
        TypeDef::Struct(_) => "fields",
        TypeDef::Enum(_) => "variants",
        TypeDef::Union(_) => "members",
    };
    Value::Map(alloc::vec![
        ("name", Value::Str(schema.name.to_string())),
        ("version", Value::Int(schema.version as i64)),
        (
            "id",
            Value::Str(crate::hash::SchemaId::of(schema).to_string())
        ),
        ("kind", Value::Str(kind.to_string())),
        (body_key, body),
    ])
}

/// Converts every schema in a registry, sorted by name then version.
pub fn registry_value(registry: &Registry) -> Value {
    let mut schemas: Vec<&Schema> = registry.iter().collect();
    schemas.sort_by(|a, b| a.name.cmp(b.name).then(a.version.cmp(&b.version)));
    Value::Map(alloc::vec![(
        "schemas",
        Value::List(schemas.into_iter().map(schema_value).collect()),
    )])
}

/// Pretty JSON with two-space indentation.
pub fn to_json(value: &Value) -> String {
    let mut out = String::new();
    json_into(value, 0, &mut out);
    out.push('\n');
    out
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_into(value: &Value, indent: usize, out: &mut String) {
    let pad = |n: usize| " ".repeat(n);
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Str(s) => json_string(s, out),
        Value::List(items) if items.is_empty() => out.push_str("[]"),
        Value::List(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad(indent + 2));
                json_into(item, indent + 2, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push(']');
        }
        Value::Map(entries) if entries.is_empty() => out.push_str("{}"),
        Value::Map(entries) => {
            out.push_str("{\n");
            for (i, (k, v)) in entries.iter().enumerate() {
                out.push_str(&pad(indent + 2));
                json_string(k, out);
                out.push_str(": ");
                json_into(v, indent + 2, out);
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push('}');
        }
    }
}

/// Block-style YAML. Strings are always double-quoted, which avoids YAML's implicit typing.
pub fn to_yaml(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::List(items) if !items.is_empty() => yaml_block(value, 0, &mut out),
        Value::Map(entries) if !entries.is_empty() => yaml_block(value, 0, &mut out),
        other => {
            yaml_scalar(other, &mut out);
            out.push('\n');
        }
    }
    out
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::List(_) | Value::Map(_))
}

fn yaml_scalar(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Str(s) => json_string(s, out),
        Value::List(_) => out.push_str("[]"),
        Value::Map(_) => out.push_str("{}"),
    }
}

fn yaml_block(value: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Map(entries) => {
            for (k, v) in entries {
                out.push_str(&pad);
                out.push_str(k);
                out.push(':');
                if is_scalar(v)
                    || matches!(v, Value::List(l) if l.is_empty())
                    || matches!(v, Value::Map(m) if m.is_empty())
                {
                    out.push(' ');
                    yaml_scalar(v, out);
                    out.push('\n');
                } else {
                    out.push('\n');
                    yaml_block(v, indent + 2, out);
                }
            }
        }
        Value::List(items) => {
            for item in items {
                out.push_str(&pad);
                out.push('-');
                if is_scalar(item)
                    || matches!(item, Value::List(l) if l.is_empty())
                    || matches!(item, Value::Map(m) if m.is_empty())
                {
                    out.push(' ');
                    yaml_scalar(item, out);
                    out.push('\n');
                } else {
                    out.push('\n');
                    yaml_block(item, indent + 2, out);
                }
            }
        }
        other => {
            out.push_str(&pad);
            yaml_scalar(other, out);
            out.push('\n');
        }
    }
}
