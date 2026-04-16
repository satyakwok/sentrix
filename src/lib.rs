// lib.rs - Sentrix
// Why: missing_docs is enabled in [lints.rust] to catch NEW undocumented public
// APIs going forward. The existing codebase pre-dates the doc policy; suppressed
// here until a dedicated documentation sprint adds top-level module docs.
#![allow(missing_docs)]

pub mod api;
pub mod core;
pub mod network;
pub mod storage;
pub mod types;
pub mod wallet;
