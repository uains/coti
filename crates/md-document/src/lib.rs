//! Document model: block arena, inline content, linear coordinates, indexing,
//! selections, and structural invariants.
//!
//! This crate depends on [`md_core`] only. It must remain independent of
//! Markdown syntax, layout, and UI frameworks.

// Keep this declared crate boundary linked while the module skeleton is empty.
use md_core as _;
