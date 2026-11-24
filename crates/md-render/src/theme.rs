//! Render theme and paint command model shared by GPUI and headless tests.

use md_core::Rect;

/// RGBA color token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    /// Transparent color.
    pub const TRANSPARENT: Self = Self(0x0000_0000);
    /// Opaque black.
    pub const BLACK: Self = Self(0x0000_00FF);
    /// Opaque white.
    pub const WHITE: Self = Self(0xFFFF_FFFF);
}

/// Visual values used by the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Page background.
    pub background: Color,
    /// Surface background (title bar, status bar).
    pub surface: Color,
    /// Body text.
    pub text: Color,
    /// Heading text.
    pub heading: Color,
    /// Muted text.
    pub muted: Color,
    /// Selection fill.
    pub selection: Color,
    /// Caret fill.
    pub caret: Color,
    /// Code block background.
    pub code_background: Color,
    /// Code block text.
    pub code_text: Color,
    /// Quote background.
    pub quote_background: Color,
    /// Quote text.
    pub quote_text: Color,
    /// Table border.
    pub border: Color,
    /// Table header background.
    pub table_header: Color,
    /// Accent color for links and the caret.
    pub accent: Color,
    /// Quote bar color.
    pub quote_bar: Color,
    /// Code block border color.
    pub code_border: Color,
    /// List bullet/number color.
    pub list_marker: Color,
    /// Body font size.
    pub font_size: f32,
    /// Body line height.
    pub line_height: f32,
    /// Caret width.
    pub caret_width: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color(0xFFFF_FFFF),
            surface: Color(0xF4F5_F8FF),
            text: Color(0x2021_24FF),
            heading: Color(0x1416_1AFF),
            muted: Color(0x6B72_80FF),
            selection: Color(0xBFD7_FFFF),
            caret: Color(0x1A73_E8FF),
            code_background: Color(0xF5F6_FAFF),
            code_text: Color(0x2A2D_33FF),
            quote_background: Color(0xF8F9_FCFF),
            quote_text: Color(0x5057_63FF),
            border: Color(0xD9DD_E5FF),
            table_header: Color(0xF1F3_F7FF),
            accent: Color(0x1A73_E8FF),
            quote_bar: Color(0x1A73_E8FF),
            code_border: Color(0xD9DD_E5FF),
            list_marker: Color(0x1A73_E8FF),
            font_size: 14.0,
            line_height: 20.0,
            caret_width: 2.0,
        }
    }
}

/// Paint layer order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaintLayer {
    /// Block background and border.
    BlockBackground,
    /// Underline and other low decorations.
    Decorations,
    /// Selection fill.
    Selection,
    /// Text glyphs.
    Glyphs,
    /// High decorations.
    TopDecorations,
    /// Images and atomic elements.
    Atoms,
    /// Caret.
    Caret,
    /// Overlays such as IME candidate hints.
    Overlay,
}

/// One render command.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommand {
    /// Fill a rectangle.
    FillRect {
        /// Target rectangle.
        rect: Rect,
        /// Fill color.
        color: Color,
    },
    /// Draw text inside a line rectangle.
    Text {
        /// Line rectangle.
        rect: Rect,
        /// Text content.
        text: String,
        /// Text color.
        color: Color,
        /// Font size.
        font_size: f32,
        /// Font weight.
        font_weight: f32,
        /// Whether this line uses a monospace family.
        mono: bool,
    },
    /// Draw an image.
    Image {
        /// Target rectangle.
        rect: Rect,
        /// Source URL/path.
        source: String,
    },
    /// Draw the caret.
    Caret {
        /// Caret rectangle.
        rect: Rect,
        /// Caret color.
        color: Color,
    },
}

/// A paint command with its layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintItem {
    /// Paint layer.
    pub layer: PaintLayer,
    /// Command payload.
    pub command: PaintCommand,
}

/// Ordered paint plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintPlan {
    /// Items in paint order.
    pub items: Vec<PaintItem>,
}

impl PaintPlan {
    /// Appends a command and keeps stable layer ordering.
    pub fn push(&mut self, layer: PaintLayer, command: PaintCommand) {
        let index = self
            .items
            .iter()
            .rposition(|item| item.layer <= layer)
            .map_or(0, |position| position + 1);
        self.items.insert(index, PaintItem { layer, command });
    }

    /// Returns commands for one layer.
    #[must_use]
    pub fn layer(&self, layer: PaintLayer) -> Vec<&PaintCommand> {
        self.items
            .iter()
            .filter(|item| item.layer == layer)
            .map(|item| &item.command)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Color, PaintCommand, PaintLayer, PaintPlan};
    use md_core::Rect;

    #[test]
    fn paint_plan_keeps_layer_order() {
        let mut plan = PaintPlan::default();
        plan.push(
            PaintLayer::Caret,
            PaintCommand::Caret {
                rect: Rect::new(0.0, 0.0, 1.0, 10.0),
                color: Color::BLACK,
            },
        );
        plan.push(
            PaintLayer::BlockBackground,
            PaintCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                color: Color::WHITE,
            },
        );
        assert_eq!(plan.items[0].layer, PaintLayer::BlockBackground);
        assert_eq!(plan.items[1].layer, PaintLayer::Caret);
    }
}
