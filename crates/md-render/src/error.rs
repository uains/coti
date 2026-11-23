//! Error types for the GPUI rendering boundary.

use md_layout::LayoutError;

/// Errors produced while preparing or painting the document view.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// Layout could not provide the geometry required for painting.
    #[error(transparent)]
    Layout(#[from] LayoutError),

    /// An image or font resource could not be loaded.
    #[error("render resource failed: {message}")]
    Resource {
        /// Human-readable resource failure.
        message: String,
    },
}

/// Result alias for rendering operations.
pub type RenderResult<T> = Result<T, RenderError>;
