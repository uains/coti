//! Error types for the GPUI rendering boundary.

use md_layout::LayoutError;

/// Errors produced while preparing or painting the document view.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// Layout could not provide the geometry required for painting.
    #[error("{0}")]
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

#[cfg(test)]
mod tests {
    use super::RenderError;
    use md_document::DocumentError;
    use md_layout::LayoutError;
    use std::error::Error;

    #[test]
    fn resource_message_includes_reason() {
        let error = RenderError::Resource {
            message: "image decode failed".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "render resource failed: image decode failed"
        );
    }

    #[test]
    fn layout_error_is_forwarded_with_source() {
        let error = RenderError::from(LayoutError::from(DocumentError::Invariant {
            message: "bad geometry".to_owned(),
        }));
        assert!(error.source().is_some());
    }
}
