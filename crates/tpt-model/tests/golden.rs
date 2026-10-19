//! Golden-file snapshots of schema exports, plus SchemaId stability.
//!
//! If one of these fails after an intentional schema change, regenerate with
//! `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-model --test golden` and review the diff.

use serde::{Deserialize, Serialize};
use tpt_model::export::{registry_value, to_json, to_yaml};
use tpt_model::{EntityId, Model, Registry, Schema, SchemaId, TypeDef};
use tpt_testkit_stub::assert_golden;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
enum Material {
    Steel,
    Titanium,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "CadPart")]
struct CadPartV1 {
    id: EntityId,
    name: String,
    #[tpt(removed_in = 2, migrate = "migrate_legacy_weight")]
    legacy_weight_kg: Option<f32>,
}

fn migrate_legacy_weight(
    old: &tpt_model::UirNode,
    _ctx: &tpt_model::MigrationContext,
) -> Result<tpt_model::UirNode, tpt_model::MigrationError> {
    Ok(old.clone())
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "CadPart")]
struct CadPartV2 {
    id: EntityId,
    name: String,
    #[tpt(since = 2, default = "Material::Titanium")]
    material: Material,
    #[tpt(since = 2, intent = "write", capability = "cad.edit")]
    mass_g: Option<f64>,
    #[tpt(rename = "label")]
    title: String,
}

fn export_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Material>().unwrap();
    registry.register::<CadPartV1>().unwrap();
    registry.register::<CadPartV2>().unwrap();
    registry
}

#[test]
fn export_json_matches_golden() {
    let registry = export_registry();
    let json = to_json(&registry_value(&registry));
    assert_golden("tests/golden/registry.json", &json);
}

#[test]
fn export_yaml_matches_golden() {
    let registry = export_registry();
    let yaml = to_yaml(&registry_value(&registry));
    assert_golden("tests/golden/registry.yaml", &yaml);
}

#[test]
fn schema_ids_are_pinned() {
    let registry = export_registry();
    // Sorted by name and version so the file reads the same whatever the id order is.
    let mut schemas: Vec<_> = registry.iter().collect();
    schemas.sort_by(|a, b| a.name.cmp(b.name).then(a.version.cmp(&b.version)));
    let mut lines = String::new();
    for schema in schemas {
        lines.push_str(&format!(
            "{} v{} {}\n",
            schema.name,
            schema.version,
            SchemaId::of(schema)
        ));
    }
    assert_golden("tests/golden/schema_ids.txt", &lines);
}

// Same fields, declared in a different order: the id must not change.
static FIELDS_AB: [tpt_model::Field; 2] = [field("a", 1), field("b", 2)];
static FIELDS_BA: [tpt_model::Field; 2] = [field("b", 2), field("a", 1)];

const fn field(name: &'static str, since: u32) -> tpt_model::Field {
    tpt_model::Field {
        name,
        ty: tpt_model::TypeRef::Prim(tpt_model::Prim::U64),
        since,
        removed_in: None,
        default: None,
        renames: &[],
        access: None,
        migrate: None,
        constraints: &[],
    }
}

static AB: Schema = Schema {
    name: "Order",
    version: 1,
    def: TypeDef::Struct(&FIELDS_AB),
};
static BA: Schema = Schema {
    name: "Order",
    version: 1,
    def: TypeDef::Struct(&FIELDS_BA),
};

#[test]
fn field_declaration_order_does_not_change_the_id() {
    assert_eq!(SchemaId::of(&AB), SchemaId::of(&BA));
}

#[test]
fn renaming_a_type_changes_the_id() {
    static RENAMED: Schema = Schema {
        name: "Other",
        version: 1,
        def: TypeDef::Struct(&FIELDS_AB),
    };
    assert_ne!(SchemaId::of(&AB), SchemaId::of(&RENAMED));
}
