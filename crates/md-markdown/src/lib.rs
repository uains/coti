//! Markdown parsing, source hints, and source-faithful serialization.
//!
//! The parser turns Markdown into the [`md_document`] model. The serializer
//! preserves pristine source byte-for-byte and re-serializes only dirty nodes.

mod error;

pub use error::{MarkdownError, MarkdownResult};

// Keep this declared crate boundary linked while the module skeleton is empty.
use md_core as _;
