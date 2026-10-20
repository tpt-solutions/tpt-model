//! Redaction walks nested structs, enum payloads and sequences, not just top-level fields.

use serde::{Deserialize, Serialize};
use tpt_model::access;
use tpt_model::{Model, Registry, UirNode};
use tpt_primitives_stub::Intent;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Note")]
struct NoteV1 {
    title: String,
    #[tpt(intent = "read", capability = "notes.private")]
    secret: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Board")]
struct BoardV1 {
    name: String,
    notes: Vec<NoteV1>,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Entry")]
enum EntryV1 {
    Plain(String),
    Note(NoteV1),
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Wrapper")]
struct WrapperV1 {
    entry: EntryV1,
}

fn node_of<T: Serialize>(value: &T) -> UirNode {
    tpt_model::ir_serde::to_node(value).unwrap()
}

#[test]
fn redacts_fields_inside_sequences_of_structs() {
    let mut registry = Registry::new();
    registry.register::<BoardV1>().unwrap();
    registry.register::<NoteV1>().unwrap();
    let board = BoardV1 {
        name: "b".into(),
        notes: vec![NoteV1 {
            title: "t".into(),
            secret: "hidden".into(),
        }],
    };
    let out = access::redact(
        &registry,
        BoardV1::schema(),
        &node_of(&board),
        Intent::Read,
        &[],
    );
    let note = match out.get("notes") {
        Some(UirNode::Seq(items)) => &items[0],
        other => panic!("expected a sequence, got {other:?}"),
    };
    assert_eq!(note.get("title"), Some(&UirNode::Str("t".into())));
    assert!(
        note.get("secret").is_none(),
        "nested secret must be removed"
    );
}

#[test]
fn redacts_fields_inside_enum_payloads() {
    let mut registry = Registry::new();
    registry.register::<WrapperV1>().unwrap();
    registry.register::<EntryV1>().unwrap();
    registry.register::<NoteV1>().unwrap();
    let wrapper = WrapperV1 {
        entry: EntryV1::Note(NoteV1 {
            title: "t".into(),
            secret: "hidden".into(),
        }),
    };
    let out = access::redact(
        &registry,
        WrapperV1::schema(),
        &node_of(&wrapper),
        Intent::Read,
        &[],
    );
    let note = out
        .get("entry")
        .and_then(|e| e.get("Note"))
        .expect("Note payload");
    assert!(note.get("secret").is_none());
}

#[test]
fn granted_capability_keeps_nested_fields() {
    let mut registry = Registry::new();
    registry.register::<BoardV1>().unwrap();
    registry.register::<NoteV1>().unwrap();
    let board = BoardV1 {
        name: "b".into(),
        notes: vec![NoteV1 {
            title: "t".into(),
            secret: "visible".into(),
        }],
    };
    let granted = [tpt_primitives_stub::Capability("notes.private")];
    let out = access::redact(
        &registry,
        BoardV1::schema(),
        &node_of(&board),
        Intent::Read,
        &granted,
    );
    let note = match out.get("notes") {
        Some(UirNode::Seq(items)) => &items[0],
        other => panic!("expected a sequence, got {other:?}"),
    };
    assert_eq!(note.get("secret"), Some(&UirNode::Str("visible".into())));
}

#[test]
fn unresolvable_nested_type_fails_closed() {
    // BoardV1 is registered, NoteV1 is not, so its contents must not pass through.
    let mut registry = Registry::new();
    registry.register::<BoardV1>().unwrap();
    let board = BoardV1 {
        name: "b".into(),
        notes: vec![NoteV1 {
            title: "t".into(),
            secret: "hidden".into(),
        }],
    };
    let out = access::redact(
        &registry,
        BoardV1::schema(),
        &node_of(&board),
        Intent::Read,
        &[],
    );
    assert_eq!(out.get("notes"), Some(&UirNode::Seq(vec![UirNode::Null])));
}
