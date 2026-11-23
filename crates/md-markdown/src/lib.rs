//! Markdown parsing, source hints, and source-faithful serialization.
//!
//! The parser turns Markdown into the [`md_document`] model. The serializer
//! preserves pristine source byte-for-byte and re-serializes only dirty nodes.

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
