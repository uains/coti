//! Error types for the document model, its index, and position resolution.

use md_core::TextError;

/// Errors produced by document structure and index operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentError {
    /// A text operation rejected an offset or range.
    #[error("{0}")]
    Text(#[from] TextError),

    /// An operation would violate a document invariant.
    #[error("document invariant violated: {message}")]
    Invariant {
        /// Human-readable description of the violated invariant.
        message: String,
    },

    /// A block id refers to a freed or reused arena slot.
    #[error("block id {index}:{generation} is stale or unknown")]
    StaleBlockId {
        /// Arena slot index.
        index: u32,
        /// Generation expected by the caller.
        generation: u32,
    },

    /// A container-only operation received a leaf node.
    #[error("block id {index}:{generation} is not a container")]
    NotAContainer {
        /// Arena slot index.
        index: u32,
        /// Generation of the referenced node.
        generation: u32,
    },

    /// A leaf-only operation received a container node.
    #[error("block id {index}:{generation} is not a leaf")]
    NotALeaf {
        /// Arena slot index.
        index: u32,
        /// Generation of the referenced node.
        generation: u32,
    },
}

/// Errors produced while converting between `DocPos` and `Position`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PositionError {
    /// A document position lies beyond the end of the document.
    #[error("document position {pos} is out of bounds for length {len}")]
    DocPosOutOfBounds {
        /// The rejected document position.
        pos: usize,
        /// The current document length.
        len: usize,
    },

    /// A position points inside an atom or otherwise off a legal boundary.
    #[error("position is not on a legal document boundary")]
    IllegalBoundary,

    /// A leaf-local offset is beyond the leaf content.
    #[error("leaf offset {offset} is out of bounds for {len} units")]
    OffsetOutOfBounds {
        /// The rejected local offset.
        offset: usize,
        /// Leaf content length.
        len: usize,
    },

    /// A position references a block that is not a leaf.
    #[error("block id {index}:{generation} is not a leaf")]
    NotALeaf {
        /// Arena slot index.
        index: u32,
        /// Generation of the referenced node.
        generation: u32,
    },

    /// A block id is stale or unknown.
    #[error("block id {index}:{generation} is stale or unknown")]
    StaleBlockId {
        /// Arena slot index.
        index: u32,
        /// Generation expected by the caller.
        generation: u32,
    },
}

/// Result alias for document operations.
pub type DocumentResult<T> = Result<T, DocumentError>;

/// Result alias for position resolution.
pub type PositionResult<T> = Result<T, PositionError>;

#[cfg(test)]
mod tests {
    use super::{DocumentError, PositionError};
    use md_core::TextError;
    use std::error::Error;

    #[test]
    fn text_error_is_forwarded_with_source() {
        let error = DocumentError::from(TextError::NotCharBoundary { offset: 2 });
        assert_eq!(
            error.to_string(),
            "text offset 2 is not a UTF-8 character boundary"
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn stale_block_message_includes_generation() {
        let error = DocumentError::StaleBlockId {
            index: 1,
            generation: 3,
        };
        assert_eq!(error.to_string(), "block id 1:3 is stale or unknown");
    }

    #[test]
    fn position_bounds_message_includes_length() {
        let error = PositionError::DocPosOutOfBounds { pos: 9, len: 4 };
        assert_eq!(
            error.to_string(),
            "document position 9 is out of bounds for length 4"
        );
    }
}
