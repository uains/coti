//! Headless editing kernel: intents, ordered rules, operations, transactions,
//! position mappings, history, and IME state.
//!
//! This crate must not depend on GPUI, keycodes, pixels, or platform APIs.

mod editor;
mod error;
mod intent;
mod rule;

pub use editor::{Composition, Editor, EditorConfig};
pub use error::{EditorError, EditorResult};
pub use intent::{BlockStyle, Intent, MoveDirection};
pub use rule::{Ctx, Rule, RuleId};

// Keep this declared crate boundary linked while editor modules grow.
use md_core as _;
