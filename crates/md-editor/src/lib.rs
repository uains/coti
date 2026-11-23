//! Headless editing kernel: intents, ordered rules, operations, transactions,
//! position mappings, history, and IME state.
//!
//! This crate must not depend on GPUI, keycodes, pixels, or platform APIs.

mod error;

pub use error::{EditorError, EditorResult};

// Keep this declared crate boundary linked while the module skeleton is empty.
use md_core as _;
