//! Headless editing kernel: intents, ordered rules, operations, transactions,
//! position mappings, history, and IME state.
//!
//! This crate must not depend on GPUI, keycodes, pixels, or platform APIs.

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
