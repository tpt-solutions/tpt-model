//! A config file written three releases ago, read by the current build.
//!
//! v1 -> v2 is automatic: `server` is renamed to `host`, and `tls` takes its default.
//! v2 -> v3 is a registered migration: `host` and `port` become one `endpoint` string.
//!
//! Run with `cargo run -p tpt-model --example config_file`.

use serde::{Deserialize, Serialize};
use tpt_crypto_stub::Blake3;
use tpt_model::evolve::{MigrationContext, MigrationError, Migrations};
use tpt_model::load::{save, Loader};
use tpt_model::{EntityId, Migration, Model, Registry, UirNode};
use tpt_validex_stub::BoundsValidator;

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Config")]
struct ConfigV1 {
    server: String,
    port: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Config")]
struct ConfigV2 {
    #[tpt(rename = "server")]
    host: String,
    #[tpt(removed_in = 3, migrate = "join_endpoint")]
    port: u32,
    #[tpt(since = 2, default = "true")]
    tls: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Config")]
struct ConfigV3 {
    endpoint: String,
    tls: bool,
}

/// Replaces `host` and `port` with `endpoint = "host:port"`.
fn join_endpoint(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
    let mut node = old.clone();
    let host = match node.remove("host") {
        Some(UirNode::Str(s)) => s,
        _ => return Err(MigrationError::custom("host must be a string")),
    };
    let port = match node.remove("port") {
        Some(UirNode::UInt(p)) => p,
        _ => return Err(MigrationError::custom("port must be an integer")),
    };
    node.insert("endpoint", UirNode::Str(format!("{host}:{port}")));
    Ok(node)
}

struct ConfigV2ToV3;

impl Migration for ConfigV2ToV3 {
    type From = ConfigV2;
    type To = ConfigV3;
    fn migrate(old: &UirNode, ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        join_endpoint(old, ctx)
    }
}

fn main() {
    let mut registry = Registry::new();
    registry.register::<ConfigV1>().expect("v1");
    let mut migrations = Migrations::new();
    migrations
        .register::<ConfigV2ToV3>(&mut registry)
        .expect("v2 -> v3");

    let id = EntityId::new_v7(1_700_000_000_000, [9; 10]);
    let stored = save(
        &Blake3,
        id,
        &ConfigV1 {
            server: "example.org".into(),
            port: 8443,
        },
    )
    .expect("save v1");

    let loader = Loader::new(&registry, &migrations, Blake3, BoundsValidator::default());
    let config: ConfigV3 = loader.load(&stored).expect("load as v3");
    println!("{config:?}");
    assert_eq!(
        config,
        ConfigV3 {
            endpoint: "example.org:8443".into(),
            tls: true,
        }
    );
}
