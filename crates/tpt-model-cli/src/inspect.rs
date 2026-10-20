//! `inspect`: shows the envelope header and the stored tree of one sealed entity.

use std::fmt::Write as _;
use tpt_crypto_stub::{Blake3, KeyedBlake3};
use tpt_model::load::read_tree;
use tpt_model::UirNode;

/// Renders a sealed entity. The hash is checked with the keyed digest when `key` is given,
/// and with unkeyed BLAKE3 otherwise. A failed check is an error, not a warning.
pub fn render(bytes: &[u8], key: Option<[u8; 32]>) -> Result<String, String> {
    let (env, tree) = match key {
        Some(key) => read_tree(&KeyedBlake3::new(key), bytes).map_err(|e| e.to_string())?,
        None => read_tree(&Blake3, bytes).map_err(|e| e.to_string())?,
    };
    let digest_name = if key.is_some() {
        "keyed BLAKE3"
    } else {
        "unkeyed BLAKE3"
    };
    let mut out = String::new();
    let _ = writeln!(out, "schema id: {}", env.schema_id);
    let _ = writeln!(out, "version:   {}", env.version);
    let _ = writeln!(out, "entity:    {}", env.entity_id);
    let _ = writeln!(out, "payload:   {} bytes", env.payload.len());
    let _ = writeln!(out, "hash:      verified ({digest_name})");
    let _ = writeln!(out, "tree:");
    dump("", &tree, 1, &mut out);
    Ok(out)
}

/// Writes `node` as an indented outline. Containers open a block, scalars are one line.
fn dump(label: &str, node: &UirNode, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let prefix = if label.is_empty() {
        String::new()
    } else {
        format!("{label}: ")
    };
    match node {
        UirNode::Map(entries) => {
            let _ = writeln!(out, "{pad}{prefix}{{");
            for (key, value) in entries {
                dump(key, value, depth + 1, out);
            }
            let _ = writeln!(out, "{pad}}}");
        }
        UirNode::Seq(items) => {
            let _ = writeln!(out, "{pad}{prefix}[");
            for (i, item) in items.iter().enumerate() {
                dump(&format!("[{i}]"), item, depth + 1, out);
            }
            let _ = writeln!(out, "{pad}]");
        }
        scalar => {
            let _ = writeln!(out, "{pad}{prefix}{}", scalar_text(scalar));
        }
    }
}

fn scalar_text(node: &UirNode) -> String {
    match node {
        UirNode::Null => "null".to_string(),
        UirNode::Bool(v) => v.to_string(),
        UirNode::Int(v) => v.to_string(),
        UirNode::UInt(v) => v.to_string(),
        UirNode::Float(v) => v.to_string(),
        UirNode::Str(s) => format!("{s:?}"),
        UirNode::Bytes(b) => {
            let hex: String = b
                .iter()
                .take(16)
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let more = if b.len() > 16 { "..." } else { "" };
            format!("bytes[{}] {hex}{more}", b.len())
        }
        // Containers are handled by `dump`.
        UirNode::Seq(_) | UirNode::Map(_) => String::new(),
    }
}
