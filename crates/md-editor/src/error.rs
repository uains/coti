//! Error types for the headless editing kernel.

use md_document::DocumentError;

/// Errors produced while dispatching intents or applying transactions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EditorError {
    /// A document operation rejected the requested change.
    #[error(transparent)]
    Document(#[from] DocumentError),

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
