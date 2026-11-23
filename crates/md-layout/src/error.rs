//! Error types for the pure geometry layout engine.

use md_document::DocumentError;

/// Errors produced while measuring or laying out a document.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LayoutError {
    /// A document query required for layout failed.
    #[error(transparent)]
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
