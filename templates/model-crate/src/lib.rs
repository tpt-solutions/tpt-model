//! {{project-name}}: versioned data model.
//!
//! Each version is its own struct. Add the next version as a new struct, mark what the old one
//! loses, and add a test in `tests/evolution.rs` for the change.

use serde::{Deserialize, Serialize};
use tpt_model::{EntityId, Model};

/// Version 1 of the item, as first released.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 1, name = "Item")]
pub struct ItemV1 {
    pub id: EntityId,
    pub name: String,
}

/// Version 2 adds an optional quantity, which old data reads as `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Model)]
#[tpt(version = 2, name = "Item")]
pub struct ItemV2 {
    pub id: EntityId,
    pub name: String,
    pub quantity: Option<u32>,
}
