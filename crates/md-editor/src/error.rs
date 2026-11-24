//! Error types for the headless editing kernel.

use md_document::{DocumentError, PositionError, SelectionError};

/// Errors produced while dispatching intents or applying transactions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EditorError {
    /// A document operation rejected the requested change.
    #[error("{0}")]
    Document(#[from] DocumentError),

    /// A structured position could not be resolved.
    #[error("{0}")]
    Position(#[from] PositionError),

    /// A selection is invalid.
    #[error("{0}")]
    Selection(#[from] SelectionError),

    /// A transaction was built against an outdated document revision.
    #[error("transaction based on revision {base} but document is at {current}")]
    StaleRevision {
        /// Revision the transaction was built against.
        base: u64,
        /// Current document revision.
        current: u64,
    },

    /// No rule can handle the intent in the current editing context.
    #[error("intent cannot be applied in the current context: {message}")]
    InvalidIntent {
        /// Human-readable reason.
        message: String,
    },

    /// A transaction failed preflight or apply and was rolled back.
    #[error("transaction failed and was rolled back: {message}")]
    Transaction {
        /// Human-readable failure reason.
        message: String,
    },
}

/// Result alias for editor operations.
pub type EditorResult<T> = Result<T, EditorError>;

#[cfg(test)]
mod tests {
    use super::EditorError;
    use md_document::DocumentError;
    use std::error::Error;

    #[test]
    fn stale_revision_reports_both_revisions() {
        let error = EditorError::StaleRevision {
            base: 4,
            current: 5,
        };
        assert_eq!(
            error.to_string(),
            "transaction based on revision 4 but document is at 5"
        );
    }

    #[test]
    fn invalid_intent_message_includes_reason() {
        let error = EditorError::InvalidIntent {
            message: "no rule matched".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "intent cannot be applied in the current context: no rule matched"
        );
    }

    #[test]
    fn document_error_is_forwarded_with_source() {
        let error = EditorError::from(DocumentError::Invariant {
            message: "bad selection".to_owned(),
        });
        assert!(error.source().is_some());
    }
}
