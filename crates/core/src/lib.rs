//! Domain models, shared types, and error definitions.
//!
//! Foundation crate -- no async or I/O dependencies.

pub mod error;
pub mod types;

pub use error::ArgusError;
pub use types::{
    AccessEntry, AccessList, AccessMode, Conflict, ConflictGraph, ConflictKind, StorageLocation,
    Transaction,
};
