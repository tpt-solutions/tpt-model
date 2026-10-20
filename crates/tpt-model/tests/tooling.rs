//! Tooling on top of the schema graph: the breaking-change linter, `explain`, the JSON
//! Schema export, and the `assert_evolves!` macro.

use serde::{Deserialize, Serialize};
use tpt_model::check::{breaking_changes, Breaking};
use tpt_model::evolve::{explain, MigrationContext, MigrationError, Migrations};
use tpt_model::export::json_schema;
use tpt_model::{Model, Registry, UirNode};

fn registry_of<T>(register: impl FnOnce(&mut Registry) -> T) -> Registry {
    let mut registry = Registry::new();
    register(&mut registry);
    registry
}

// ---- breaking-change linter ----------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Thing")]
struct ThingA {
    a: u32,
    b: String,
}

// Same logical name and version as ThingA, different structure.
#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Thing")]
struct ThingEdited {
    a: u32,
    c: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2NoB {
    a: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2Extra {
    a: u32,
    b: String,
    #[tpt(since = 2)]
    extra: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2Defaulted {
    a: u32,
    b: String,
    #[tpt(since = 2, default = "0")]
    extra: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2Renamed {
    a: u32,
    #[tpt(rename = "b")]
    label: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2Widened {
    a: f64,
    b: String,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Thing")]
struct ThingV2Narrowed {
    a: String,
    b: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Mode")]
enum ModeV1 {
    On,
    Off,
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Mode")]
enum ModeV2 {
    On,
    Off,
}

#[test]
fn unchanged_registries_have_no_breaking_changes() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let new = registry_of(|r| r.register::<ThingA>().unwrap());
    assert_eq!(breaking_changes(&old, &new), vec![]);
}

#[test]
fn same_version_with_a_different_structure_is_flagged() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let new = registry_of(|r| r.register::<ThingEdited>().unwrap());
    let changes = breaking_changes(&old, &new);
    assert!(changes.contains(&Breaking::ChangedWithoutBump {
        name: "Thing",
        version: 1
    }));
}

#[test]
fn dropped_field_is_flagged() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let new = registry_of(|r| r.register::<ThingV2NoB>().unwrap());
    assert_eq!(
        breaking_changes(&old, &new),
        vec![Breaking::FieldDropped {
            name: "Thing",
            field: "b"
        }]
    );
}

#[test]
fn added_field_needs_a_default() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let bad = registry_of(|r| r.register::<ThingV2Extra>().unwrap());
    assert!(
        breaking_changes(&old, &bad).contains(&Breaking::FieldAddedWithoutDefault {
            name: "Thing",
            field: "extra"
        })
    );

    let ok = registry_of(|r| r.register::<ThingV2Defaulted>().unwrap());
    assert_eq!(breaking_changes(&old, &ok), vec![]);
}

#[test]
fn rename_is_not_a_drop_or_an_add() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let new = registry_of(|r| r.register::<ThingV2Renamed>().unwrap());
    assert_eq!(breaking_changes(&old, &new), vec![]);
}

#[test]
fn widening_is_fine_and_narrowing_is_not() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let widened = registry_of(|r| r.register::<ThingV2Widened>().unwrap());
    assert_eq!(breaking_changes(&old, &widened), vec![]);

    let narrowed = registry_of(|r| r.register::<ThingV2Narrowed>().unwrap());
    let changes = breaking_changes(&old, &narrowed);
    assert!(
        matches!(
            changes.as_slice(),
            [Breaking::FieldTypeChanged { field: "a", .. }]
        ),
        "{changes:?}"
    );
}

#[test]
fn removed_type_and_removed_variant_are_flagged() {
    let old = registry_of(|r| r.register::<ThingA>().unwrap());
    let empty = Registry::new();
    assert_eq!(
        breaking_changes(&old, &empty),
        vec![Breaking::TypeRemoved { name: "Thing" }]
    );

    let old = registry_of(|r| r.register::<ModeV1>().unwrap());
    let new = registry_of(|r| r.register::<ModeV2>().unwrap());
    assert_eq!(
        breaking_changes(&old, &new),
        vec![Breaking::VariantRemoved {
            name: "Mode",
            variant: "Auto"
        }]
    );
}

// ---- explain ---------------------------------------------------------------------------

fn unused(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    Ok(old.clone())
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Ex")]
struct ExV1 {
    keep: u32,
    title: String,
    #[tpt(removed_in = 2, migrate = "unused")]
    gone: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Ex")]
struct ExV2 {
    keep: u32,
    #[tpt(rename = "title")]
    name: String,
    #[tpt(since = 2, default = "7")]
    added: u32,
    opt: Option<u32>,
}

#[test]
fn explain_lists_each_field_without_running_anything() {
    let mut registry = Registry::new();
    registry.register::<ExV1>().unwrap();
    registry.register::<ExV2>().unwrap();
    let migrations = Migrations::new();
    let text = explain(&registry, &migrations, "Ex", 1, 2, None).unwrap();
    for line in [
        "1. Ex v1 -> v2: automatic",
        "keep keep",
        "rename title -> name",
        "default added = 7",
        "null opt",
        "error: gone needs migration `unused`",
    ] {
        assert!(text.contains(line), "missing `{line}` in:\n{text}");
    }
}

#[test]
fn explain_on_equal_versions_says_there_is_nothing_to_do() {
    let mut registry = Registry::new();
    registry.register::<ExV2>().unwrap();
    let text = explain(&registry, &Migrations::new(), "Ex", 2, 2, None).unwrap();
    assert!(text.contains("no steps"), "{text}");
}

// ---- JSON Schema --------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Tiny")]
struct TinyV1 {
    #[tpt(min = 1, max = 9)]
    n: u32,
    label: Option<String>,
    #[tpt(min = 2)]
    tags: Vec<String>,
}

#[test]
fn json_schema_describes_structs_with_constraints_and_required_fields() {
    let registry = registry_of(|r| r.register::<TinyV1>().unwrap());
    let text = json_schema(&registry);
    assert!(text.contains("\"$schema\": \"https://json-schema.org/draft/2020-12/schema\""));
    assert!(text.contains("\"Tiny\": {"), "{text}");
    assert!(text.contains("\"additionalProperties\": false"));
    assert!(text.contains("\"required\": [\n"), "{text}");
    assert!(text.contains("\"minimum\": 1"));
    assert!(text.contains("\"maximum\": 9"));
    assert!(text.contains("\"minItems\": 2"));
    assert!(text.contains("\"anyOf\""), "optional fields accept null");
}

#[test]
fn json_schema_describes_enums_and_references() {
    #[derive(Debug, Serialize, Deserialize, Model)]
    #[tpt(version = 1, name = "Holder")]
    struct HolderV1 {
        mode: ModeV2,
    }

    let mut registry = Registry::new();
    registry.register::<ModeV2>().unwrap();
    registry.register::<HolderV1>().unwrap();
    let text = json_schema(&registry);
    assert!(text.contains("\"const\": \"On\""), "{text}");
    assert!(text.contains("\"$ref\": \"#/$defs/Mode\""), "{text}");
}

// ---- assert_evolves! -------------------------------------------------------------------

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Pair")]
struct PairV1 {
    first: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Pair")]
struct PairV2 {
    first: u32,
    #[tpt(since = 2, default = "5")]
    second: u32,
}

#[test]
fn assert_evolves_checks_one_evolution() {
    let mut registry = Registry::new();
    registry.register::<PairV1>().unwrap();
    registry.register::<PairV2>().unwrap();
    let migrations = Migrations::new();
    tpt_model::assert_evolves!(
        registry,
        migrations,
        PairV1 { first: 3 },
        PairV2,
        PairV2 {
            first: 3,
            second: 5
        }
    );
}
