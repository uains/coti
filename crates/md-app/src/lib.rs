//! Coti desktop application assembly: window lifecycle, file I/O, menus,
//! themes, preferences, and coordination of the editor, layout, and render
//! layers.
//!
//! This is the only crate allowed to know about every lower layer. Business
//! editing rules stay in [`md_editor`]; rendering stays in [`md_render`].

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
use md_editor as _;
use md_layout as _;
use md_markdown as _;
use md_render as _;

/// Runs the Coti application.
pub fn run() {
    println!("Coti: 日常书写，日结为札");
}
