//! `tpt-model`: schema definition, registry, structural evolution and migrations.
//!
//! `no_std` with `alloc`. The `std` feature adds nothing by itself; `global-registry`
//! adds a process-wide `registry::global` registry.
//!
//! See `README.md` for a quickstart and `docs/` for the versioning rules and the
//! migration author guide.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
// Lets `::tpt_model::...` paths emitted by the derive resolve inside this crate too.
extern crate self as tpt_model;

pub mod access;
pub mod check;
pub mod default_expr;
pub mod entity;
pub mod envelope;
pub mod evolve;
pub mod export;
pub mod hash;
pub mod ir_serde;
pub mod load;
pub mod registry;
pub mod schema;
pub mod shape;
#[cfg(feature = "std")]
pub mod snapshot;

pub use entity::EntityId;
pub use envelope::Envelope;
pub use evolve::{Migration, MigrationContext, MigrationError, Migrations};
pub use hash::SchemaId;
pub use load::{LoadError, Loader};
pub use registry::{Registry, RegistryError};
pub use schema::{Access, Constraint, Field, Prim, Schema, TypeDef, TypeRef, Variant};
pub use tpt_model_derive::Model;
pub use tpt_uir_stub::UirNode;

/// A Rust type with a versioned schema.
///
/// Implemented by `#[derive(Model)]`. Each version of a type is its own Rust struct
/// (for example `CadPartV1` and `CadPartV2`), and all versions share a logical name.
pub trait Model: Sized {
    /// Identity of this exact schema version.
    const SCHEMA_ID: SchemaId;
    /// Version number from `#[tpt(version = N)]`.
    const SCHEMA_VERSION: u32;
    /// The schema graph for this version.
    fn schema() -> &'static Schema;
}

/// Internal re-exports used by generated code. Not part of the public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::evolve::{MigrationContext, MigrationError};
    pub use crate::hash::SchemaId;
    pub use crate::schema::{Access, Constraint, Field, Prim, Schema, TypeDef, TypeRef, Variant};
    pub use crate::UirNode;
    pub use tpt_crypto_stub::Blake3;
    pub use tpt_primitives_stub::Intent;
    pub use tpt_validex_stub::BoundsValidator;
}

/// Asserts that a value saved as `$old` loads as `$new` and equals `$expected`.
///
/// Uses unkeyed BLAKE3 and the default validator, the same as the examples. Use it in a
/// downstream crate's tests, one line per evolution:
///
/// ```ignore
/// tpt_model::assert_evolves!(registry, migrations, CadPartV1 { .. }, CadPartV2, expected);
/// ```
#[macro_export]
macro_rules! assert_evolves {
    ($registry:expr, $migrations:expr, $old:expr, $new:ty, $expected:expr $(,)?) => {{
        let id = $crate::EntityId::new_v7(1_700_000_000_000, [0; 10]);
        let digest = $crate::__private::Blake3;
        let bytes = $crate::load::save(&digest, id, &$old).expect("save the old value");
        let loader = $crate::load::Loader::new(
            &$registry,
            &$migrations,
            $crate::__private::Blake3,
            $crate::__private::BoundsValidator::default(),
        );
        let got: $new = loader
            .load(&bytes)
            .unwrap_or_else(|e| panic!("loading as {} failed: {e}", stringify!($new)));
        assert_eq!(got, $expected);
    }};
}

/// Asserts that `$registry` matches the schema snapshot at `$path` (see [`snapshot`]).
#[cfg(feature = "std")]
#[macro_export]
macro_rules! assert_schema_snapshot {
    ($registry:expr, $path:expr $(,)?) => {
        if let ::core::result::Result::Err(message) =
            $crate::snapshot::check(&$registry, ::std::path::Path::new(&$path))
        {
            panic!("{message}");
        }
    };
}
