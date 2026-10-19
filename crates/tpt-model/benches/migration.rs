//! Migration throughput: loading a V1 entity as the latest version, for a chain of
//! ten versions with a custom step in the middle.

use criterion::{criterion_group, criterion_main, Criterion};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Bench")]
struct BenchV1 {
    a: u32,
    #[tpt(rename = "label")]
    name: String,
    values: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Bench")]
struct BenchV2 {
    a: u32,
    name: String,
    values: Vec<u64>,
    #[tpt(since = 2, default = "0")]
    b: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Bench")]
struct BenchV3 {
    a: u32,
    name: String,
    values: Vec<u64>,
    b: u32,
    #[tpt(since = 3, default = "None")]
    c: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 4, name = "Bench")]
struct BenchV4 {
    a: u32,
    name: String,
    values: Vec<u64>,
    b: u32,
    c: Option<String>,
    #[tpt(since = 4, default = "true")]
    d: bool,
}

struct Step23;
impl Migration for Step23 {
    type From = BenchV2;
    type To = BenchV3;
    fn migrate(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        let mut node = old.clone();
        node.insert("c", UirNode::Str("migrated".into()));
        Ok(node)
    }
}

fn setup() -> (Registry, Migrations, Vec<u8>) {
    let mut registry = Registry::new();
    registry.register::<BenchV1>().unwrap();
    registry.register::<BenchV2>().unwrap();
    let mut migrations = Migrations::new();
    migrations.register::<Step23>(&mut registry).unwrap();
    registry.register::<BenchV4>().unwrap();
    let values: Vec<u64> = (0..1_000).collect();
    let bytes = save(
        &Blake3,
        EntityId::new_v7(1, [4; 10]),
        &BenchV1 {
            a: 1,
            name: "bench".into(),
            values,
        },
    )
    .unwrap();
    (registry, migrations, bytes)
}

fn bench_load(c: &mut Criterion) {
    let (registry, migrations, bytes) = setup();
    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    c.bench_function("load_v1_as_v4_1000_values", |b| {
        b.iter(|| {
            let got: BenchV4 = loader.load(black_box(&bytes)).unwrap();
            black_box(got)
        })
    });
}

criterion_group!(benches, bench_load);
criterion_main!(benches);
