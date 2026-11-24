//! Resolved visual values supplied by the renderer.

use md_core::Size;

/// Layout-visible style values. Colors never affect geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedStyle {
    /// Base body font size.
    pub font_size: f32,
    /// Base body line height.
    pub line_height: f32,
    /// Vertical gap between sibling blocks.
    pub block_gap: f32,
    /// Heading scale factors for levels 1..=6.
    pub heading_scale: [f32; 6],
    /// Code block font size.
    pub code_font_size: f32,
    /// Code block inner padding.
    pub code_padding: f32,
    /// Quote indentation.
    pub quote_indent: f32,
    /// List indentation per level.
    pub list_indent: f32,
    /// Table cell padding.
    pub cell_padding: f32,
    /// Default image height when no intrinsic size is available.
    pub image_height: f32,
    /// Thematic break thickness.
    pub rule_thickness: f32,
}

impl Default for ResolvedStyle {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 20.0,
            block_gap: 8.0,
            heading_scale: [1.8, 1.6, 1.4, 1.25, 1.15, 1.1],
            code_font_size: 13.0,
            code_padding: 8.0,
            quote_indent: 16.0,
            list_indent: 20.0,
            cell_padding: 6.0,
            image_height: 120.0,
            rule_thickness: 1.0,
        }
    }
}

/// Measurement style for one text run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureStyle {
    /// Font size in layout units.
    pub font_size: f32,
    /// Line height in layout units.
    pub line_height: f32,
    /// Extra letter spacing.
    pub letter_spacing: f32,
}

impl MeasureStyle {
    /// Creates a measurement style.
    #[must_use]
    pub const fn new(font_size: f32, line_height: f32) -> Self {
        Self {
            font_size,
            line_height,
            letter_spacing: 0.0,
        }
    }
}

impl Default for MeasureStyle {
    fn default() -> Self {
        Self::new(14.0, 20.0)
    }
}

/// Paint-only style carried beside layout for renderer use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintStyle {
    /// Opaque foreground color token.
    pub foreground: u32,
}

/// Viewport and style inputs for a layout pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutEnv {
    /// Visible viewport size.
    pub viewport: Size,
    /// Resolved visual style.
    pub style: ResolvedStyle,
}

impl LayoutEnv {
    /// Creates a layout environment.
    #[must_use]
    pub const fn new(viewport: Size, style: ResolvedStyle) -> Self {
        Self { viewport, style }
    }

    /// Returns the content width.
    #[must_use]
    pub fn width(self) -> f32 {
        self.viewport.width.max(0.0)
    }
}

impl Default for LayoutEnv {
    fn default() -> Self {
        Self {
            viewport: Size::new(800.0, 600.0),
            style: ResolvedStyle::default(),
        }
    }
}
