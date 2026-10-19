//! Property tests: round-trips, canonical encoding, and "garbage in, error out".

use proptest::collection::vec;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{migrate, MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Model, Registry, UirNode};
use tpt_uir_stub::{decode, encode};
use tpt_validex_stub::BoundsValidator;

fn leaf() -> impl Strategy<Value = UirNode> {
    prop_oneof![
        Just(UirNode::Null),
        any::<bool>().prop_map(UirNode::Bool),
        any::<i64>().prop_map(UirNode::Int),
        any::<u64>().prop_map(UirNode::UInt),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(UirNode::Float),
        "[a-zA-Z0-9 _-]{0,12}".prop_map(UirNode::Str),
        vec(any::<u8>(), 0..16).prop_map(UirNode::Bytes),
    ]
}

fn node() -> impl Strategy<Value = UirNode> {
    leaf().prop_recursive(4, 64, 6, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..6).prop_map(UirNode::Seq),
            vec(("[a-z]{1,6}", inner), 0..6).prop_map(|entries| {
                // Keys must be unique for a map to round-trip meaningfully.
                let mut seen = std::collections::BTreeSet::new();
                UirNode::Map(
                    entries
                        .into_iter()
                        .filter(|(k, _)| seen.insert(k.clone()))
                        .collect(),
                )
            }),
        ]
    })
}

proptest! {
    #[test]
    fn encode_then_decode_is_identity(n in node()) {
        let bytes = encode(&n);
        prop_assert_eq!(decode(&bytes).unwrap(), n);
    }

    #[test]
    fn encoding_is_deterministic(n in node()) {
        prop_assert_eq!(encode(&n), encode(&n.clone()));
    }

    #[test]
    fn canonical_form_ignores_map_order(n in node()) {
        let reversed = reverse_maps(&n);
        prop_assert_eq!(encode(&n.canonicalize()), encode(&reversed.canonicalize()));
    }

    #[test]
    fn decoding_arbitrary_bytes_never_panics(bytes in vec(any::<u8>(), 0..512)) {
        let _ = decode(&bytes);
    }

    #[test]
    fn truncating_a_valid_encoding_always_errors(n in node(), cut in 0usize..256) {
        let bytes = encode(&n);
        if cut < bytes.len() {
            prop_assert!(decode(&bytes[..cut]).is_err());
        }
    }
}

fn reverse_maps(n: &UirNode) -> UirNode {
    match n {
        UirNode::Map(entries) => UirNode::Map(
            entries
                .iter()
                .rev()
                .map(|(k, v)| (k.clone(), reverse_maps(v)))
                .collect(),
        ),
        UirNode::Seq(items) => UirNode::Seq(items.iter().map(reverse_maps).collect()),
        other => other.clone(),
    }
}

// ---- garbage through the migration engine --------------------------------

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Fuzz")]
struct FuzzV1 {
    a: u32,
    #[tpt(since = 1)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Fuzz")]
struct FuzzV2 {
    a: u32,
    tags: Vec<String>,
    #[tpt(since = 2, default = "5")]
    b: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Fuzz")]
struct FuzzV3 {
    a: u32,
    tags: Vec<String>,
    b: u32,
    #[tpt(since = 3, default = "[]")]
    extra: Vec<String>,
}

struct FuzzStep;
impl tpt_model::Migration for FuzzStep {
    type From = FuzzV2;
    type To = FuzzV3;
    fn migrate(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        let mut node = old.clone();
        node.insert("extra", UirNode::Seq(Vec::new()));
        Ok(node)
    }
}

fn fuzz_registry() -> (Registry, Migrations) {
    let mut registry = Registry::new();
    registry.register::<FuzzV1>().unwrap();
    let mut migrations = Migrations::new();
    migrations.register::<FuzzStep>(&mut registry).unwrap();
    registry.register::<FuzzV3>().unwrap();
    (registry, migrations)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Any tree, fed into any step, returns Ok or Err. It never panics.
    #[test]
    fn garbage_trees_produce_errors_not_panics(n in node()) {
        let (registry, migrations) = fuzz_registry();
        let _ = migrate(&registry, &migrations, "Fuzz", 1, 3, None, n.clone());
        let _ = migrate(&registry, &migrations, "Fuzz", 2, 3, None, n);
    }

    /// Garbage payloads inside a valid envelope fail at load, never panic.
    #[test]
    fn garbage_payloads_in_a_valid_envelope_fail_cleanly(bytes in vec(any::<u8>(), 0..256)) {
        let (registry, migrations) = fuzz_registry();
        let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
        let id = EntityId::new_v7(1, [1; 10]);
        let env = tpt_model::Envelope::seal(&Blake3, FuzzV1::SCHEMA_ID, 1, id, bytes).unwrap();
        let _ = loader.load::<FuzzV3>(&env.encode());
    }

    /// Round-trip through the real pipeline keeps the data that should survive.
    #[test]
    fn values_survive_the_chain(a in any::<u32>(), tags in vec("[a-z]{0,5}", 0..4)) {
        let (registry, migrations) = fuzz_registry();
        let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
        let id = EntityId::new_v7(7, [2; 10]);
        let bytes = save(&Blake3, id, &FuzzV1 { a, tags: tags.clone() }).unwrap();
        let got: FuzzV3 = loader.load(&bytes).unwrap();
        prop_assert_eq!(got.a, a);
        prop_assert_eq!(got.tags, tags);
        prop_assert_eq!(got.b, 5);
        prop_assert!(got.extra.is_empty());
    }
}

// ---- order-normalized digest, property form -----------------------------

proptest! {
    #[test]
    fn any_permutation_of_fields_has_the_same_id(seed in any::<u64>()) {
        use tpt_model::{Field, Prim, Schema, SchemaId, TypeDef, TypeRef};
        let names = ["alpha", "beta", "gamma", "delta", "epsilon"];
        let build = |order: &[usize]| -> Vec<Field> {
            order
                .iter()
                .map(|&i| Field {
                    name: names[i],
                    ty: TypeRef::Prim(Prim::U64),
                    since: 1,
                    removed_in: None,
                    default: None,
                    renames: &[],
                    access: None,
                    migrate: None,
                    constraints: &[],
                })
                .collect()
        };
        let mut order: Vec<usize> = (0..names.len()).collect();
        // A cheap deterministic shuffle driven by the seed.
        let mut s = seed | 1;
        for i in (1..order.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            order.swap(i, (s % (i as u64 + 1)) as usize);
        }
        let base: Vec<Field> = build(&[0, 1, 2, 3, 4]);
        let shuffled: Vec<Field> = build(&order);
        let base_s: &'static [Field] = Box::leak(base.into_boxed_slice());
        let shuffled_s: &'static [Field] = Box::leak(shuffled.into_boxed_slice());
        let a: &'static Schema = Box::leak(Box::new(Schema { name: "Perm", version: 1, def: TypeDef::Struct(base_s) }));
        let b: &'static Schema = Box::leak(Box::new(Schema { name: "Perm", version: 1, def: TypeDef::Struct(shuffled_s) }));
        prop_assert_eq!(SchemaId::of(a), SchemaId::of(b));
    }
}
