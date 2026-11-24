//! Pure geometry layout engine: text measurement port, line breaking, block
//! geometry, hit-testing, selection rectangles, and incremental layout.
//!
//! This crate must not depend on GPUI, glyph shaping, colors, or font
//! resources. Visual values arrive through `ResolvedStyle` values supplied by
//! the renderer.

mod error;
mod inline;
mod layout;
mod measure;
mod style;

pub use error::{LayoutError, LayoutResult};
pub use inline::{Cluster, InlineLayout, Line, layout_inline};
pub use layout::{BlockLayout, LayoutEngine, LayoutKind, LayoutTree, body_measure_style};
pub use measure::{FakeMeasurer, LineMetrics, TextMeasurer, is_wide};
pub use style::{LayoutEnv, MeasureStyle, PaintStyle, ResolvedStyle};

// Keep this declared crate boundary linked while editor modules grow.
use md_core as _;
