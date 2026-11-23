//! Error types for Markdown parsing and serialization.

use md_document::DocumentError;

/// Errors produced while parsing Markdown or serializing a document.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MarkdownError {
    /// The parser could not build a valid document from the source.
    #[error("markdown parse failed: {message}")]
    Parse {
        /// Human-readable parse failure.
        message: String,
    },

    /// The serializer could not represent the document in the target dialect.
    #[error("markdown serialization failed: {message}")]
    Serialize {
        /// Human-readable serialization failure.
        message: String,
    },

    /// The document builder rejected a structural change.
    #[error(transparent)]
    Document(#[from] DocumentError),
}

/// Result alias for Markdown operations.
pub type MarkdownResult<T> = Result<T, MarkdownError>;
