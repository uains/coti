//! Markdown parsing, source hints, and source-faithful serialization.
//!
//! The parser turns Markdown into the [`md_document`] model. The serializer
//! preserves pristine source byte-for-byte and re-serializes only dirty nodes.

mod error;
mod parser;
mod serializer;

use std::sync::Arc;

use md_document::Document;

pub use error::{MarkdownError, MarkdownResult};

/// A parsed Markdown source and its document model.
#[derive(Clone, Debug)]
pub struct MarkdownDocument {
    document: Document,
    source: Arc<str>,
    dirty: bool,
}

impl MarkdownDocument {
    /// Returns the document model.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns mutable document access and marks the document dirty.
    pub fn document_mut(&mut self) -> &mut Document {
        self.dirty = true;
        &mut self.document
    }

    /// Returns the original Markdown source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns `true` when the document has been modified since parsing.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marks the document dirty.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Serializes back to Markdown.
    ///
    /// An unedited document returns its original source byte-for-byte.
    #[must_use]
    pub fn serialize(&self) -> String {
        if self.dirty {
            serializer::serialize(&self.document, &self.source)
        } else {
            self.source.to_string()
        }
    }

    /// Validates the document and its source hints.
    ///
    /// # Errors
    ///
    /// Returns the underlying document error.
    pub fn validate(&self) -> MarkdownResult<()> {
        self.document.validate()?;
        Ok(())
    }

    /// Consumes the wrapper and returns the document.
    #[must_use]
    pub fn into_document(self) -> Document {
        self.document
    }
}

/// Parses CommonMark + GFM into a [`MarkdownDocument`].
///
/// # Errors
///
/// Returns a parse or document error when the event stream cannot be built into
/// a valid document tree.
pub fn parse(source: &str) -> MarkdownResult<MarkdownDocument> {
    parser::parse(source)
}
