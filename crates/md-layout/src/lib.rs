//! Pure geometry layout engine: text measurement port, line breaking, block
//! geometry, hit-testing, selection rectangles, and incremental layout.
//!
//! This crate must not depend on GPUI, glyph shaping, colors, or font
//! resources. Visual values arrive through `ResolvedStyle` values supplied by
//! the renderer.

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
