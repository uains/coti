//! Error types for the pure geometry layout engine.

use md_document::DocumentError;

/// Errors produced while measuring or laying out a document.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LayoutError {
    /// A document query required for layout failed.
    #[error("{0}")]
    Document(#[from] DocumentError),

    /// A layout environment or geometry value is invalid.
    #[error("invalid layout geometry: {message}")]
    Geometry {
        /// Human-readable reason.
        message: String,
    },

    /// No layout record exists for the requested block.
    #[error("layout for block {index}:{generation} is not available")]
    MissingBlockLayout {
        /// Arena slot index.
        index: u32,
        /// Generation of the referenced node.
        generation: u32,
    },
}

/// Result alias for layout operations.
pub type LayoutResult<T> = Result<T, LayoutError>;

#[cfg(test)]
mod tests {
    use super::LayoutError;
    use md_document::DocumentError;
    use std::error::Error;

    #[test]
    fn geometry_message_includes_reason() {
        let error = LayoutError::Geometry {
            message: "negative width".to_owned(),
        };
        assert_eq!(error.to_string(), "invalid layout geometry: negative width");
    }

    #[test]
    fn missing_layout_message_includes_block_identity() {
        let error = LayoutError::MissingBlockLayout {
            index: 7,
            generation: 2,
        };
        assert_eq!(error.to_string(), "layout for block 7:2 is not available");
    }

    #[test]
    fn document_error_is_forwarded_with_source() {
        let error = LayoutError::from(DocumentError::Invariant {
            message: "bad index".to_owned(),
        });
        assert!(error.source().is_some());
    }
}
