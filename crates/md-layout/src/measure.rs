//! Text measurement port and a deterministic fake implementation.

use std::cell::RefCell;
use std::collections::HashMap;

use md_document::AtomKind;

use crate::style::MeasureStyle;

/// Vertical metrics for one laid-out line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    /// Distance from the line top to the baseline.
    pub ascent: f32,
    /// Distance below the baseline.
    pub descent: f32,
    /// Total line height.
    pub line_height: f32,
}

impl LineMetrics {
    /// Creates line metrics.
    #[must_use]
    pub const fn new(ascent: f32, descent: f32, line_height: f32) -> Self {
        Self {
            ascent,
            descent,
            line_height,
        }
    }
}

/// Port used by layout to measure clusters and atoms.
pub trait TextMeasurer {
    /// Measures one grapheme cluster or text chunk.
    fn advance(&self, text: &str, style: &MeasureStyle) -> f32;

    /// Measures the grapheme clusters of one text run.
    fn cluster_advances(&self, text: &str, style: &MeasureStyle) -> Vec<f32> {
        use unicode_segmentation::UnicodeSegmentation;
        text.graphemes(true)
            .map(|grapheme| self.advance(grapheme, style))
            .collect()
    }

    /// Measures one inline atom.
    fn atom_advance(&self, kind: &AtomKind, style: &MeasureStyle) -> f32;

    /// Returns baseline metrics.
    fn line_metrics(&self, style: &MeasureStyle) -> LineMetrics;
}

/// Deterministic fake measurer used by tests and headless layout.
#[derive(Debug, Default)]
pub struct FakeMeasurer {
    cache: RefCell<HashMap<(u32, String), f32>>,
}

impl FakeMeasurer {
    /// Creates an empty fake measurer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the internal width cache.
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    /// Returns the cached entry count.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.borrow().len()
    }

    fn char_advance(character: char, font_size: f32, letter_spacing: f32) -> f32 {
        let ratio = if character.is_ascii_whitespace() {
            0.33
        } else if character.is_whitespace() {
            0.5
        } else if is_wide(character) {
            1.0
        } else if character.is_ascii() {
            0.6
        } else {
            0.8
        };
        font_size.mul_add(ratio, letter_spacing)
    }
}

impl TextMeasurer for FakeMeasurer {
    fn advance(&self, text: &str, style: &MeasureStyle) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let key = (style.font_size.to_bits(), text.to_owned());
        if let Some(width) = self.cache.borrow().get(&key) {
            return *width;
        }
        let width = text.chars().fold(0.0, |width, character| {
            width + Self::char_advance(character, style.font_size, style.letter_spacing)
        });
        self.cache.borrow_mut().insert(key, width);
        width
    }

    fn atom_advance(&self, kind: &AtomKind, style: &MeasureStyle) -> f32 {
        match kind {
            AtomKind::HardBreak | AtomKind::SoftBreak => 0.0,
            AtomKind::Image(_) => style.font_size,
            AtomKind::InlineMath(source) => self.advance(source.as_str(), style),
            AtomKind::FootnoteRef(label) => self.advance(label, style) * 0.6,
        }
    }

    fn line_metrics(&self, style: &MeasureStyle) -> LineMetrics {
        LineMetrics::new(
            style.line_height * 0.8,
            style.line_height * 0.2,
            style.line_height,
        )
    }
}

/// Returns whether a character is treated as full-width.
#[must_use]
pub fn is_wide(character: char) -> bool {
    matches!(
        character as u32,
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x3FFFD
    )
}

#[cfg(test)]
mod tests {
    use super::{FakeMeasurer, TextMeasurer};
    use crate::style::MeasureStyle;

    #[test]
    fn fake_measurer_is_deterministic_and_caches() {
        let measurer = FakeMeasurer::new();
        let style = MeasureStyle::default();
        let first = measurer.advance("abc", &style);
        let second = measurer.advance("abc", &style);
        assert_eq!(first, second);
        assert_eq!(measurer.cache_len(), 1);
    }

    #[test]
    fn wide_characters_are_wider_than_ascii() {
        let measurer = FakeMeasurer::new();
        let style = MeasureStyle::default();
        assert!(measurer.advance("中", &style) > measurer.advance("a", &style));
    }

    #[test]
    fn empty_text_has_zero_width() {
        let measurer = FakeMeasurer::new();
        assert_eq!(measurer.advance("", &MeasureStyle::default()), 0.0);
    }
}
