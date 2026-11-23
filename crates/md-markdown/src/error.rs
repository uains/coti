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
    #[error("{0}")]
    Document(#[from] DocumentError),
}

/// Result alias for Markdown operations.
pub type MarkdownResult<T> = Result<T, MarkdownError>;

#[cfg(test)]
mod tests {
    use super::MarkdownError;
    use md_document::DocumentError;
    use std::error::Error;

    #[test]
    fn parse_error_message_includes_reason() {
        let error = MarkdownError::Parse {
            message: "unclosed fence".to_owned(),
        };
        assert_eq!(error.to_string(), "markdown parse failed: unclosed fence");
    }

    #[test]
    fn document_error_is_forwarded_with_source() {
        let error = MarkdownError::from(DocumentError::Invariant {
            message: "bad child".to_owned(),
        });
        assert_eq!(error.to_string(), "document invariant violated: bad child");
        assert!(error.source().is_some());
    }
}
