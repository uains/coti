//! GPUI rendering and input adaptation for the Coti document view.
//!
//! Rendering consumes layout geometry and must never mutate the document.
//! The crate exposes a GPUI-ready paint plan, input adapter, shaping cache,
//! theme, and resource lifecycle. A thin GPUI element adapter is layered on
//! top of these primitives by `md-app`.

mod error;
mod input;
mod paint;
mod resources;
mod shaping;
mod theme;
mod view;

pub use error::{RenderError, RenderResult};
pub use input::{CaretBlink, InputAdapter, Key, ScrollState, key_to_intent};
pub use paint::{build_paint_plan, has_layer, transparent};
pub use resources::{ImageState, ResourceCache};
pub use shaping::{FakeShaper, ShapedGlyph, ShapedRun, Shaper, ShapingCache};
pub use theme::{Color, PaintCommand, PaintItem, PaintLayer, PaintPlan, Theme};
pub use view::RenderView;

// Keep these declared crate boundaries linked while the module skeleton is empty.
use md_core as _;
use md_document as _;
use md_editor as _;
