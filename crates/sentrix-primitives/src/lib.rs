//! sentrix-primitives — Core types and error handling for Sentrix blockchain.
//!
//! This crate provides the foundational error types used by all other Sentrix
//! crates. It has no internal dependencies and is the leaf of the dependency
//! graph.

#![allow(missing_docs)]

pub mod account;
pub mod error;

// Re-export commonly used types at crate root for convenience.
pub use account::{Account, AccountDB, EMPTY_CODE_HASH, EMPTY_STORAGE_ROOT, SENTRI_PER_SRX};
pub use error::{SentrixError, SentrixResult};
