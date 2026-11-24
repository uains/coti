//! Shaping port, deterministic fake shaper, and shaping cache.

use std::cell::RefCell;
use std::collections::HashMap;

use md_layout::is_wide;

/// One shaped glyph cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    /// UTF-8 byte start in the source text.
    pub cluster_start: usize,
    /// UTF-8 byte end in the source text.
    pub cluster_end: usize,
    /// Advance width.
    pub advance: f32,
}

/// Shaped text run.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRun {
    /// Glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Total advance.
    pub width: f32,
}

/// Shaping port. Real GPUI shaping is supplied by the renderer.
pub trait Shaper {
    /// Shapes text at a font size.
    fn shape(&self, text: &str, font_size: f32) -> ShapedRun;
}

/// Deterministic shaper for headless tests.
#[derive(Debug, Default)]
pub struct FakeShaper;

impl Shaper for FakeShaper {
    fn shape(&self, text: &str, font_size: f32) -> ShapedRun {
        let mut glyphs = Vec::new();
        let mut width = 0.0;
        for (byte, character) in text.char_indices() {
            let advance = font_size
                * if character.is_whitespace() {
                    0.33
                } else if is_wide(character) {
                    1.0
                } else {
                    0.6
                };
            glyphs.push(ShapedGlyph {
                cluster_start: byte,
                cluster_end: byte + character.len_utf8(),
                advance,
            });
            width += advance;
        }
        ShapedRun { glyphs, width }
    }
}

/// Cache keyed by font size and text.
#[derive(Debug, Default)]
pub struct ShapingCache {
    entries: RefCell<HashMap<(u32, String), ShapedRun>>,
}

impl ShapingCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Shapes through a shaper, reusing cached results.
    pub fn shape(&self, shaper: &dyn Shaper, text: &str, font_size: f32) -> ShapedRun {
        let key = (font_size.to_bits(), text.to_owned());
        if let Some(run) = self.entries.borrow().get(&key) {
            return run.clone();
        }
        let run = shaper.shape(text, font_size);
        self.entries.borrow_mut().insert(key, run.clone());
        run
    }

    /// Returns cached entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// Clears the cache.
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeShaper, Shaper, ShapingCache};

    #[test]
    fn fake_shaping_is_deterministic() {
        let shaper = FakeShaper;
        assert_eq!(shaper.shape("abc", 14.0), shaper.shape("abc", 14.0));
    }

    #[test]
    fn shaping_cache_reuses_entries() {
        let cache = ShapingCache::new();
        let shaper = FakeShaper;
        let first = cache.shape(&shaper, "abc", 14.0);
        let second = cache.shape(&shaper, "abc", 14.0);
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
