//! GPUI rendering and input adaptation for the Coti document view.
//!
//! Rendering consumes layout geometry and must never mutate the document.
//! GPUI integration is intentionally introduced in phase 8; during phases 0-7
//! this crate stays free of UI dependencies.

mod error;

pub use error::{RenderError, RenderResult};

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
use md_editor as _;
