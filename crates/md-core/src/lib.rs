//! Core text and geometry primitives shared by every Coti crate.
//!
//! This crate owns UTF-8 boundary handling, grapheme navigation, UTF-16
//! conversion, and the `f32` geometry types. It must not know about document
//! structure or any UI framework.

mod error;
mod text;

pub use error::{TextError, TextResult};
pub use text::{TextBuf, TextOffset, TextRange};
