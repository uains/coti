//! Source-fidelity metadata attached to block nodes.

use std::ops::Range;
use std::sync::Arc;

use crate::FenceStyle;

/// Heading source style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadingStyle {
    /// ATX heading (`# Heading`).
    Atx,
    /// Setext heading (`Heading` + `===` / `---`).
    Setext,
}

/// Hard break source style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardBreakStyle {
    /// Two trailing spaces.
    TwoSpaces,
    /// A trailing backslash.
    Backslash,
}

/// List marker style hints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListStyleHint {
    /// Bullet character for unordered lists.
    pub marker: char,
    /// Whether the list is ordered.
    pub ordered: bool,
    /// First number for ordered lists.
    pub start: u32,
    /// Width of the marker prefix.
    pub marker_width: usize,
    /// Leading indentation.
    pub indent: usize,
    /// Whether the list is tight.
    pub tight: bool,
}

/// Original style hints used when re-serializing dirty nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleHints {
    /// Emphasis marker character.
    pub emphasis_marker: Option<char>,
    /// Strong marker character.
    pub strong_marker: Option<char>,
    /// Code fence style.
    pub code_fence: Option<FenceStyle>,
    /// Heading source style.
    pub heading_style: Option<HeadingStyle>,
    /// List marker style.
    pub list_style: Option<ListStyleHint>,
    /// Hard break source style.
    pub hard_break: Option<HardBreakStyle>,
}

/// Source metadata for a block node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceHint {
    pristine: bool,
    original: Option<Arc<str>>,
    range: Option<Range<usize>>,
    style: StyleHints,
}

impl SourceHint {
    /// Creates a pristine hint pointing at a source range.
    #[must_use]
    pub fn pristine(original: Arc<str>, range: Range<usize>) -> Self {
        Self {
            pristine: true,
            original: Some(original),
            range: Some(range),
            style: StyleHints::default(),
        }
    }

    /// Creates a dirty hint without a source range.
    #[must_use]
    pub fn dirty() -> Self {
        Self::default()
    }

    /// Returns `true` when the node still maps to unmodified source.
    #[must_use]
    pub const fn is_pristine(&self) -> bool {
        self.pristine
    }

    /// Returns the shared original source.
    #[must_use]
    pub fn original(&self) -> Option<&str> {
        self.original.as_deref()
    }

    /// Returns the source byte range.
    #[must_use]
    pub fn range(&self) -> Option<Range<usize>> {
        self.range.clone()
    }

    /// Returns the style hints.
    #[must_use]
    pub const fn style(&self) -> &StyleHints {
        &self.style
    }

    /// Returns mutable style hints.
    pub fn style_mut(&mut self) -> &mut StyleHints {
        &mut self.style
    }

    /// Returns the pristine source slice, if valid.
    #[must_use]
    pub fn source_text(&self) -> Option<&str> {
        if !self.pristine {
            return None;
        }
        let original = self.original.as_deref()?;
        let range = self.range.as_ref()?;
        original.get(range.clone())
    }

    /// Marks the node dirty and drops its direct source range.
    pub fn mark_dirty(&mut self) {
        self.pristine = false;
        self.range = None;
    }

    /// Replaces the style hints.
    pub fn set_style(&mut self, style: StyleHints) {
        self.style = style;
    }

    /// Validates that a pristine hint has a usable range.
    ///
    /// # Errors
    ///
    /// Returns a message when the range is reversed, out of bounds, or missing.
    pub fn validate(&self) -> Result<(), String> {
        if !self.pristine {
            return Ok(());
        }

        let Some(original) = self.original.as_deref() else {
            return Err("pristine source hint is missing original text".to_owned());
        };
        let Some(range) = self.range.as_ref() else {
            return Err("pristine source hint is missing a range".to_owned());
        };
        if range.start > range.end || range.end > original.len() {
            return Err(format!(
                "pristine source range {}..{} is out of bounds for {} bytes",
                range.start,
                range.end,
                original.len()
            ));
        }
        if !original.is_char_boundary(range.start) || !original.is_char_boundary(range.end) {
            return Err("pristine source range is not on UTF-8 boundaries".to_owned());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{HardBreakStyle, HeadingStyle, SourceHint, StyleHints};
    use std::sync::Arc;

    #[test]
    fn pristine_hint_exposes_source_slice() {
        let source: Arc<str> = Arc::from("# Title\n");
        let hint = SourceHint::pristine(source, 0..8);
        assert!(hint.is_pristine());
        assert_eq!(hint.source_text(), Some("# Title\n"));
        assert_eq!(hint.original(), Some("# Title\n"));
        assert!(hint.validate().is_ok());
    }

    #[test]
    fn dirty_hint_has_no_source_slice() {
        let mut hint = SourceHint::pristine(Arc::from("text"), 0..4);
        hint.mark_dirty();
        assert!(!hint.is_pristine());
        assert!(hint.source_text().is_none());
        assert!(hint.range().is_none());
        assert!(hint.validate().is_ok());
    }

    #[test]
    fn invalid_pristine_ranges_are_rejected() {
        let source: Arc<str> = Arc::from("abc");
        let hint = SourceHint::pristine(source, 2..9);
        assert!(hint.validate().is_err());
    }

    #[test]
    fn style_hints_are_attached() {
        let source: Arc<str> = Arc::from("**x**");
        let mut hint = SourceHint::pristine(source, 0..5);
        hint.style_mut().strong_marker = Some('*');
        hint.style_mut().heading_style = Some(HeadingStyle::Atx);
        hint.style_mut().hard_break = Some(HardBreakStyle::Backslash);
        assert_eq!(hint.style().strong_marker, Some('*'));
        assert_eq!(hint.style().heading_style, Some(HeadingStyle::Atx));
        assert_eq!(hint.style().hard_break, Some(HardBreakStyle::Backslash));
        let _ = StyleHints::default();
    }
}
