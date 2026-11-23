//! Document model: block arena, inline content, linear coordinates, indexing,
//! selections, and structural invariants.
//!
//! This crate depends on [`md_core`] only. It must remain independent of
//! Markdown syntax, layout, and UI frameworks.

mod block;
mod error;
mod id;
mod inline;

pub use block::{Align, BlockKind, BlockNode, FenceStyle};
pub use error::{DocumentError, DocumentResult, PositionError, PositionResult};
pub use id::BlockId;
pub use inline::{AtomKind, ImageRef, InlineContent, LinkRef, MarkFlags, Marks, Run};
