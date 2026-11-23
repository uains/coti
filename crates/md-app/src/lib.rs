//! Coti desktop application assembly: window lifecycle, file I/O, menus,
//! themes, preferences, and coordination of the editor, layout, and render
//! layers.
//!
//! This is the only crate allowed to know about every lower layer. Business
//! editing rules stay in [`md_editor`]; rendering stays in [`md_render`].

pub mod logging;

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
use md_editor as _;
use md_layout as _;
use md_markdown as _;
use md_render as _;

/// Runs the Coti application.
///
/// # Errors
///
/// Returns an error when application startup or the main event loop fails.
pub fn run() -> anyhow::Result<()> {
    logging::init();
    tracing::info!(target: "md_app", "starting Coti");
    println!("Coti: 日常书写，日结为札");
    Ok(())
}
