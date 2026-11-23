//! Document model: block arena, inline content, linear coordinates, indexing,
//! selections, and structural invariants.
//!
//! This crate depends on [`md_core`] only. It must remain independent of
//! Markdown syntax, layout, and UI frameworks.

mod error;

pub use error::{DocumentError, DocumentResult, PositionError, PositionResult};
