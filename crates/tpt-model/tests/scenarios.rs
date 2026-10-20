//! Failure paths and rules: gaps, ambiguity, minimum supported version, downgrades,
//! tampering, strict automatic evolution, access control and graph checks.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::access;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, LoadError, Loader};
use tpt_model::registry::RegistryError;
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_primitives_stub::{Capability, Intent};
use tpt_validex_stub::BoundsValidator;

fn id() -> EntityId {
    EntityId::new_v7(1_700_000_000_000, [9; 10])
}

fn loader<'a>(
    registry: &'a Registry,
    migrations: &'a Migrations,
) -> Loader<'a, Blake3, BoundsValidator> {
    Loader::new(registry, migrations, Blake3, BoundsValidator::default())
}

fn identity(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    Ok(old.clone())
}

// ---- gaps and ambiguity -----------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Gap")]
struct GapV1 {
    x: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Gap")]
struct GapV3 {
    x: u32,
}

#[test]
fn missing_middle_version_is_a_gap() {
    let mut registry = Registry::new();
    registry.register::<GapV1>().unwrap();
    registry.register::<GapV3>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &GapV1 { x: 1 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<GapV3>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::Gap {
            name: "Gap",
            missing: 2
        })
    );
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Amb")]
struct AmbV1 {
    x: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Amb")]
struct AmbV2 {
    x: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Amb")]
struct AmbV3 {
    x: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 4, name = "Amb")]
struct AmbV4 {
    x: u32,
}

macro_rules! step {
    ($name:ident, $from:ty, $to:ty) => {
        struct $name;
        impl Migration for $name {
            type From = $from;
            type To = $to;
            fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
                identity(old, ctx)
            }
        }
    };
}

step!(Amb12, AmbV1, AmbV2);
step!(Amb13, AmbV1, AmbV3);
step!(Amb24, AmbV2, AmbV4);
step!(Amb34, AmbV3, AmbV4);

#[test]
fn two_equally_short_paths_are_ambiguous() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations.register::<Amb12>(&mut registry).unwrap();
    migrations.register::<Amb13>(&mut registry).unwrap();
    migrations.register::<Amb24>(&mut registry).unwrap();
    migrations.register::<Amb34>(&mut registry).unwrap();
    registry.register::<AmbV4>().unwrap();
    let bytes = save(&Blake3, id(), &AmbV1 { x: 1 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<AmbV4>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::Ambiguous {
            name: "Amb",
            from: 1,
            to: 4,
            paths: 2
        })
    );
}

#[test]
fn duplicate_migration_is_rejected_at_registration() {
    let mut registry = Registry::new();
    let mut migrations = Migrations::new();
    migrations.register::<Amb12>(&mut registry).unwrap();
    let err = migrations.register::<Amb12>(&mut registry).unwrap_err();
    assert_eq!(
        err,
        MigrationError::DuplicateMigration {
            name: "Amb",
            from: 1,
            to: 2
        }
    );
}

// ---- minimum supported version, downgrades ----------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Floor")]
struct FloorV1 {
    x: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Floor")]
struct FloorV2 {
    x: u32,
    #[tpt(since = 2, default = "0")]
    y: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Floor")]
struct FloorV3 {
    x: u32,
    y: u32,
}

#[test]
fn data_below_minimum_supported_is_refused() {
    let mut registry = Registry::new();
    registry.register::<FloorV1>().unwrap();
    registry.register::<FloorV2>().unwrap();
    registry.register::<FloorV3>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &FloorV1 { x: 1 }).unwrap();
    let mut loader = loader(&registry, &migrations);
    loader.min_supported = Some(2);
    let err = loader.load::<FloorV3>(&bytes).unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::BelowMinSupported {
            name: "Floor",
            found: 1,
            min: 2
        })
    );
    // At the floor, loading works.
    let bytes = save(&Blake3, id(), &FloorV2 { x: 1, y: 2 }).unwrap();
    let got: FloorV3 = loader.load(&bytes).unwrap();
    assert_eq!((got.x, got.y), (1, 2));
}

#[test]
fn newer_data_is_refused_as_a_downgrade() {
    let mut registry = Registry::new();
    registry.register::<FloorV2>().unwrap();
    registry.register::<FloorV3>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &FloorV3 { x: 1, y: 2 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<FloorV2>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::Downgrade {
            name: "Floor",
            from: 3,
            to: 2
        })
    );
}

// ---- tampering and envelope checks ------------------------------------------

#[test]
fn modified_payload_is_detected_before_anything_else() {
    let mut registry = Registry::new();
    registry.register::<FloorV1>().unwrap();
    let migrations = Migrations::new();
    let mut bytes = save(&Blake3, id(), &FloorV1 { x: 1 }).unwrap();
    let payload_byte = bytes.len() - 40;
    bytes[payload_byte] ^= 0xff;
    let err = loader(&registry, &migrations)
        .load::<FloorV1>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Envelope(tpt_model::envelope::EnvelopeError::Tampered)
    );
}

#[test]
fn unknown_schema_id_is_reported() {
    let registry = Registry::new();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &FloorV1 { x: 1 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<FloorV1>(&bytes)
        .unwrap_err();
    assert_eq!(err, LoadError::UnknownSchema(FloorV1::SCHEMA_ID));
}

#[test]
fn wrong_requested_type_is_reported() {
    let mut registry = Registry::new();
    registry.register::<GapV1>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &GapV1 { x: 1 }).unwrap();
    let mut other = Registry::new();
    other.register::<FloorV1>().unwrap();
    other.register::<GapV1>().unwrap();
    let err = loader(&other, &migrations)
        .load::<FloorV1>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::WrongType {
            expected: "Floor",
            found: "Gap"
        }
    );
}

// ---- strict automatic evolution ---------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Auto")]
struct AutoV1 {
    keep: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Auto")]
struct AutoV2 {
    keep: u32,
    #[tpt(since = 2)]
    must_have: u32,
}

#[test]
fn new_required_field_without_default_fails() {
    let mut registry = Registry::new();
    registry.register::<AutoV1>().unwrap();
    registry.register::<AutoV2>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &AutoV1 { keep: 1 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<AutoV2>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::MissingField {
            name: "Auto",
            field: "must_have",
            from: 1,
            to: 2
        })
    );
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Removal")]
struct RemovalV1 {
    keep: u32,
    #[tpt(removed_in = 2, migrate = "unused_migration")]
    old: u32,
}

fn unused_migration(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    identity(old, ctx)
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Removal")]
struct RemovalV2 {
    keep: u32,
}

#[test]
fn removed_field_with_migrate_needs_a_registered_migration() {
    let mut registry = Registry::new();
    registry.register::<RemovalV1>().unwrap();
    registry.register::<RemovalV2>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &RemovalV1 { keep: 1, old: 9 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<RemovalV2>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::RequiresMigration {
            name: "Removal",
            field: "old",
            from: 1,
            to: 2
        })
    );
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Widen")]
struct WidenV1 {
    value: i64,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Widen")]
struct WidenV2 {
    value: f64,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Widen")]
struct WidenV3 {
    value: i64,
}

#[test]
fn integer_to_float_widening_is_automatic() {
    let mut registry = Registry::new();
    registry.register::<WidenV1>().unwrap();
    registry.register::<WidenV2>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &WidenV1 { value: -4 }).unwrap();
    let got: WidenV2 = loader(&registry, &migrations).load(&bytes).unwrap();
    assert_eq!(got.value, -4.0);
}

#[test]
fn float_to_integer_narrowing_needs_an_explicit_migration() {
    let mut registry = Registry::new();
    registry.register::<WidenV2>().unwrap();
    registry.register::<WidenV3>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &WidenV2 { value: 2.5 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<WidenV3>(&bytes)
        .unwrap_err();
    assert!(matches!(
        err,
        LoadError::Migration(MigrationError::TypeChange {
            name: "Widen",
            field: "value",
            ..
        })
    ));
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Extra")]
struct ExtraV1 {
    keep: u32,
    extra: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Extra")]
struct ExtraV2 {
    keep: u32,
}

#[test]
fn field_that_vanishes_without_removed_in_is_an_error() {
    let mut registry = Registry::new();
    registry.register::<ExtraV1>().unwrap();
    registry.register::<ExtraV2>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &ExtraV1 { keep: 1, extra: 2 }).unwrap();
    let err = loader(&registry, &migrations)
        .load::<ExtraV2>(&bytes)
        .unwrap_err();
    assert_eq!(
        err,
        LoadError::Migration(MigrationError::UnknownField {
            name: "Extra",
            field: "extra".into(),
            from: 1,
            to: 2
        })
    );
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "BadDefault")]
struct BadDefaultV2 {
    #[tpt(since = 2, default = "not an expression!")]
    value: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "BadDefault")]
struct BadDefaultV1 {}

#[test]
fn malformed_default_is_reported() {
    let mut registry = Registry::new();
    registry.register::<BadDefaultV1>().unwrap();
    registry.register::<BadDefaultV2>().unwrap();
    let migrations = Migrations::new();
    let bytes = save(&Blake3, id(), &BadDefaultV1 {}).unwrap();
    let err = loader(&registry, &migrations)
        .load::<BadDefaultV2>(&bytes)
        .unwrap_err();
    assert!(matches!(
        err,
        LoadError::Migration(MigrationError::Default {
            name: "BadDefault",
            field: "value",
            ..
        })
    ));
}

// ---- shape validation and nested types --------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
enum Shape {
    Empty,
    Circle(u32),
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Holder")]
struct HolderV1 {
    shape: Shape,
    maybe: Option<Shape>,
}

#[test]
fn nested_enums_round_trip_and_unknown_variants_fail() {
    let mut registry = Registry::new();
    registry.register::<Shape>().unwrap();
    registry.register::<HolderV1>().unwrap();
    let migrations = Migrations::new();
    let value = HolderV1 {
        shape: Shape::Circle(3),
        maybe: Some(Shape::Empty),
    };
    let got: HolderV1 = loader(&registry, &migrations)
        .load(&save(&Blake3, id(), &value).unwrap())
        .unwrap();
    assert_eq!(got.shape, Shape::Circle(3));
    assert_eq!(got.maybe, Some(Shape::Empty));

    // A variant name the registry does not know is rejected by shape validation.
    let mut node = tpt_model::ir_serde::to_node(&value).unwrap();
    node.insert("shape", UirNode::Str("Hexagon".into()));
    let bytes = tpt_uir_stub::encode(&node);
    let env = tpt_model::Envelope::seal(&Blake3, HolderV1::SCHEMA_ID, 1, id(), bytes).unwrap();
    let err = loader(&registry, &migrations)
        .load::<HolderV1>(&env.encode())
        .unwrap_err();
    assert!(
        matches!(
            err,
            LoadError::SourceShape {
                name: "Holder",
                version: 1,
                error: tpt_model::shape::ShapeError::UnknownVariant { .. },
            }
        ),
        "got {err:?}"
    );
}

// ---- graph checks ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CycA {
    next: Option<Box<CycB>>,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct CycB {
    prev: Option<Box<CycA>>,
}

#[test]
fn named_cycles_are_rejected() {
    let mut registry = Registry::new();
    registry.register::<CycA>().unwrap();
    registry.register::<CycB>().unwrap();
    match registry.check_graph() {
        Err(RegistryError::Cycle { .. }) => {}
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct Dangling {
    missing: NeverRegistered,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1)]
struct NeverRegistered {
    x: u32,
}

#[test]
fn dangling_references_are_rejected() {
    let mut registry = Registry::new();
    registry.register::<Dangling>().unwrap();
    assert_eq!(
        registry.check_graph(),
        Err(RegistryError::DanglingRef {
            schema: "Dangling",
            reference: "NeverRegistered"
        })
    );
}

// ---- field access ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Secret")]
struct SecretV1 {
    public: u32,
    #[tpt(intent = "read", capability = "cad.read")]
    private: String,
    #[tpt(intent = "write", capability = "cad.edit")]
    guarded: u32,
}

#[test]
fn field_access_follows_intent_and_capability() {
    let schema = SecretV1::schema();
    let read = Intent::Read;
    let write = Intent::Write;
    let reader = [Capability("cad.read")];
    let editor = [Capability("cad.edit")];

    assert!(access::check(schema, "public", read, &[]).is_ok());
    assert!(access::check(schema, "private", read, &[]).is_err());
    assert!(access::check(schema, "private", read, &reader).is_ok());
    // Writes to `private` also need cad.read, because intent "read" covers writes too.
    assert!(access::check(schema, "private", write, &reader).is_ok());
    // `guarded` only restricts writes.
    assert!(access::check(schema, "guarded", read, &[]).is_ok());
    assert!(access::check(schema, "guarded", write, &[]).is_err());
    assert!(access::check(schema, "guarded", write, &editor).is_ok());
    assert!(access::check(schema, "nope", read, &[]).is_err());
}

#[test]
fn redaction_removes_denied_fields() {
    let mut registry = Registry::new();
    registry.register::<SecretV1>().unwrap();
    let value = SecretV1 {
        public: 1,
        private: "p".into(),
        guarded: 2,
    };
    let node = tpt_model::ir_serde::to_node(&value).unwrap();
    let redacted = access::redact(&registry, SecretV1::schema(), &node, Intent::Read, &[]);
    assert!(redacted.get("public").is_some());
    assert!(redacted.get("private").is_none());
    assert!(redacted.get("guarded").is_some());
}

#[test]
fn schema_validation_catches_bad_version_ranges() {
    // Built by hand because the derive would reject these at compile time.
    static FIELDS: [tpt_model::Field; 1] = [tpt_model::Field {
        name: "x",
        ty: tpt_model::TypeRef::Prim(tpt_model::Prim::U64),
        since: 3,
        removed_in: None,
        default: None,
        renames: &[],
        access: None,
        migrate: None,
        constraints: &[],
    }];
    static BAD: tpt_model::Schema = tpt_model::Schema {
        name: "Bad",
        version: 2,
        def: tpt_model::TypeDef::Struct(&FIELDS),
    };
    let err = BAD.validate().unwrap_err();
    assert!(err.message.contains("since"));
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register_schema(&BAD),
        Err(RegistryError::Invalid(_))
    ));
}

// ---- constraints ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Bounded")]
struct BoundedV1 {
    #[tpt(min = 1, max = 10)]
    count: u32,
    #[tpt(min = -5)]
    offset: i64,
    #[tpt(max = 3)]
    code: String,
}

#[test]
fn constraints_are_enforced_on_load() {
    let mut registry = Registry::new();
    registry.register::<BoundedV1>().unwrap();
    let migrations = Migrations::new();
    let ok = BoundedV1 {
        count: 4,
        offset: -5,
        code: "abc".into(),
    };
    let got: BoundedV1 = loader(&registry, &migrations)
        .load(&save(&Blake3, id(), &ok).unwrap())
        .unwrap();
    assert_eq!(got.count, 4);

    let too_big = BoundedV1 {
        count: 11,
        offset: 0,
        code: "a".into(),
    };
    let err = loader(&registry, &migrations)
        .load::<BoundedV1>(&save(&Blake3, id(), &too_big).unwrap())
        .unwrap_err();
    assert!(
        matches!(&err, LoadError::SourceShape { error: tpt_model::shape::ShapeError::Constraint { path, .. }, .. } if path == "$.count"),
        "{err:?}"
    );

    let too_low = BoundedV1 {
        count: 1,
        offset: -6,
        code: "a".into(),
    };
    assert!(loader(&registry, &migrations)
        .load::<BoundedV1>(&save(&Blake3, id(), &too_low).unwrap())
        .is_err());

    let long_code = BoundedV1 {
        count: 1,
        offset: 0,
        code: "abcd".into(),
    };
    assert!(loader(&registry, &migrations)
        .load::<BoundedV1>(&save(&Blake3, id(), &long_code).unwrap())
        .is_err());
}

#[test]
fn inverted_constraint_bounds_are_rejected_at_registration() {
    static FIELDS: [tpt_model::Field; 1] = [tpt_model::Field {
        name: "x",
        ty: tpt_model::TypeRef::Prim(tpt_model::Prim::I64),
        since: 1,
        removed_in: None,
        default: None,
        renames: &[],
        access: None,
        migrate: None,
        constraints: &[tpt_model::Constraint::Min(5), tpt_model::Constraint::Max(2)],
    }];
    static BAD: tpt_model::Schema = tpt_model::Schema {
        name: "Inverted",
        version: 1,
        def: tpt_model::TypeDef::Struct(&FIELDS),
    };
    let err = BAD.validate().unwrap_err();
    assert!(err.message.contains("min 5 above max 2"), "{err}");
}

// ---- automatic drop of a removed field (no migration) -------------------------

static DROP_OLD_FIELDS: [tpt_model::Field; 2] = [
    tpt_model::Field {
        name: "keep",
        ty: tpt_model::TypeRef::Prim(tpt_model::Prim::U64),
        since: 1,
        removed_in: None,
        default: None,
        renames: &[],
        access: None,
        migrate: None,
        constraints: &[],
    },
    tpt_model::Field {
        name: "gone",
        ty: tpt_model::TypeRef::Prim(tpt_model::Prim::U64),
        since: 1,
        removed_in: Some(2),
        default: None,
        renames: &[],
        access: None,
        migrate: None,
        constraints: &[],
    },
];

static DROP_NEW_FIELDS: [tpt_model::Field; 1] = [tpt_model::Field {
    name: "keep",
    ty: tpt_model::TypeRef::Prim(tpt_model::Prim::U64),
    since: 1,
    removed_in: None,
    default: None,
    renames: &[],
    access: None,
    migrate: None,
    constraints: &[],
}];

static DROP_V1: tpt_model::Schema = tpt_model::Schema {
    name: "Drop",
    version: 1,
    def: tpt_model::TypeDef::Struct(&DROP_OLD_FIELDS),
};
static DROP_V2: tpt_model::Schema = tpt_model::Schema {
    name: "Drop",
    version: 2,
    def: tpt_model::TypeDef::Struct(&DROP_NEW_FIELDS),
};

#[test]
fn a_removal_without_migrate_is_dropped_automatically() {
    // Hand-built schemas, because the derive requires `migrate` on removed fields.
    let mut registry = Registry::new();
    registry.register_schema(&DROP_V1).unwrap();
    registry.register_schema(&DROP_V2).unwrap();
    let migrations = Migrations::new();
    let node = UirNode::Map(vec![
        ("keep".into(), UirNode::UInt(7)),
        ("gone".into(), UirNode::UInt(9)),
    ]);
    let out = tpt_model::evolve::migrate(&registry, &migrations, "Drop", 1, 2, None, node).unwrap();
    assert_eq!(out, UirNode::Map(vec![("keep".into(), UirNode::UInt(7))]));
}

// ---- generic tree reader -------------------------------------------------------

#[test]
fn read_tree_returns_the_stored_tree_without_knowing_the_type() {
    let bytes = save(&Blake3, id(), &FloorV1 { x: 3 }).unwrap();
    let (env, node) = tpt_model::load::read_tree(&Blake3, &bytes).unwrap();
    assert_eq!(env.version, 1);
    assert_eq!(node.get("x"), Some(&UirNode::UInt(3)));

    let mut tampered = bytes;
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(tpt_model::load::read_tree(&Blake3, &tampered).is_err());
}
