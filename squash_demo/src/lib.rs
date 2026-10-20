//! Sample source for `tpt-model squash`. CI runs the CLI over this directory.
//!
//! With `--min-supported 2`, the v1 struct and the v1 -> v2 migration are planned for removal.
//! Nothing that stays refers to them, so the plan is accepted.

use serde::{Deserialize, Serialize};
use tpt_model::evolve::{MigrationContext, MigrationError};
use tpt_model::{Migration, Model, UirNode};

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Config")]
pub struct ConfigV1 {
    pub threads: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Config")]
pub struct ConfigV2 {
    pub workers: u32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[tpt(version = 3, name = "Config")]
pub struct ConfigV3 {
    pub workers: u32,
    #[tpt(since = 3, default = "\"default\"")]
    pub profile: String,
}

pub struct ConfigV1ToV2;

impl Migration for ConfigV1ToV2 {
    type From = ConfigV1;
    type To = ConfigV2;

    fn migrate(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {
        let mut node = old.clone();
        if let Some(threads) = node.remove("threads") {
            node.insert("workers", threads);
        }
        Ok(node)
    }
}
