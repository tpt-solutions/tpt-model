//! The load pipeline: `load::<T>(bytes)`.
//!
//! 1. Envelope read (strict lengths).
//! 2. Tamper-hash check.
//! 3. Validation of the raw payload against the source schema.
//! 4. Bytes to IR tree.
//! 5. Migration resolution and execution, validating after each step.
//! 6. Hydration of the IR tree into `T`.

use crate::entity::EntityId;
use crate::envelope::{Envelope, EnvelopeError};
use crate::evolve::{self, MigrationError, Migrations};
use crate::hash::SchemaId;
use crate::ir_serde::{self, SerdeError};
use crate::registry::Registry;
use crate::Model;
use alloc::vec::Vec;
use core::fmt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tpt_crypto_stub::Digest;
use tpt_uir_stub::{decode, encode, UirError, UirNode};
use tpt_validex_stub::{ExpectedSchema, ValidationError, Validator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Envelope(EnvelopeError),
    UnknownSchema(SchemaId),
    /// The envelope holds a different logical type than the one requested.
    WrongType {
        expected: &'static str,
        found: &'static str,
    },
    /// The envelope's version field disagrees with the registered schema it names.
    VersionMismatch {
        envelope: u32,
        schema: u32,
    },
    Validation(ValidationError),
    Decode(UirError),
    Migration(MigrationError),
    Hydrate(SerdeError),
    Encode(SerdeError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Envelope(e) => write!(f, "{e}"),
            LoadError::UnknownSchema(id) => write!(f, "no schema registered for id {id}"),
            LoadError::WrongType { expected, found } => {
                write!(f, "expected a {expected}, found a {found}")
            }
            LoadError::VersionMismatch { envelope, schema } => {
                write!(f, "envelope says v{envelope} but its schema is v{schema}")
            }
            LoadError::Validation(e) => write!(f, "{e}"),
            LoadError::Decode(e) => write!(f, "{e}"),
            LoadError::Migration(e) => write!(f, "{e}"),
            LoadError::Hydrate(e) => write!(f, "hydration failed: {e}"),
            LoadError::Encode(e) => write!(f, "encoding failed: {e}"),
        }
    }
}

impl From<EnvelopeError> for LoadError {
    fn from(e: EnvelopeError) -> Self {
        LoadError::Envelope(e)
    }
}

impl From<MigrationError> for LoadError {
    fn from(e: MigrationError) -> Self {
        LoadError::Migration(e)
    }
}

/// Holds the registry, migrations, digest and validator used to load entities.
pub struct Loader<'a, D: Digest, V: Validator> {
    pub registry: &'a Registry,
    pub migrations: &'a Migrations,
    pub digest: D,
    pub validator: V,
    /// Data older than this version is refused with `BelowMinSupported`.
    pub min_supported: Option<u32>,
}

impl<'a, D: Digest, V: Validator> Loader<'a, D, V> {
    pub fn new(
        registry: &'a Registry,
        migrations: &'a Migrations,
        digest: D,
        validator: V,
    ) -> Self {
        Loader {
            registry,
            migrations,
            digest,
            validator,
            min_supported: None,
        }
    }

    /// Loads, migrates and hydrates an entity of type `T`.
    pub fn load<T: Model + DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, LoadError> {
        let env = Envelope::decode(bytes)?;
        env.verify(&self.digest)?;

        let source = self
            .registry
            .get(&env.schema_id)
            .ok_or(LoadError::UnknownSchema(env.schema_id))?;
        let target = T::schema();
        if source.name != target.name {
            return Err(LoadError::WrongType {
                expected: target.name,
                found: source.name,
            });
        }
        if source.version != env.version {
            return Err(LoadError::VersionMismatch {
                envelope: env.version,
                schema: source.version,
            });
        }

        self.validator
            .validate(
                ExpectedSchema {
                    name: source.name,
                    version: source.version,
                },
                &env.payload,
            )
            .map_err(LoadError::Validation)?;
        let node = decode(&env.payload).map_err(LoadError::Decode)?;

        let node = evolve::migrate(
            self.registry,
            self.migrations,
            target.name,
            env.version,
            T::SCHEMA_VERSION,
            self.min_supported,
            node,
        )?;

        ir_serde::from_node::<T>(&node).map_err(LoadError::Hydrate)
    }
}

/// Serializes `value` into a sealed envelope.
pub fn save<T: Model + Serialize, D: Digest>(
    digest: &D,
    entity_id: EntityId,
    value: &T,
) -> Result<Vec<u8>, LoadError> {
    let node = ir_serde::to_node(value).map_err(LoadError::Encode)?;
    let payload = encode(&node);
    let env = Envelope::seal(digest, T::SCHEMA_ID, T::SCHEMA_VERSION, entity_id, payload)?;
    Ok(env.encode())
}

/// Reads an entity without knowing its type.
///
/// Checks the envelope and the tamper hash, then returns the envelope and the IR tree as
/// stored. No validation against a schema, no migration, no hydration. Intended for tooling
/// such as inspectors and repair scripts.
pub fn read_tree<D: Digest>(digest: &D, bytes: &[u8]) -> Result<(Envelope, UirNode), LoadError> {
    let env = Envelope::decode(bytes)?;
    env.verify(digest)?;
    let node = decode(&env.payload).map_err(LoadError::Decode)?;
    Ok((env, node))
}
