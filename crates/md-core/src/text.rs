//! UTF-8 text primitives: offsets, ranges, and a boundary-checked buffer.

use std::fmt;
use std::ops::Range;

use crate::{TextError, TextResult};

/// A byte offset into UTF-8 text.
///
/// Offsets count bytes, not characters or grapheme clusters. Use
/// [`TextBuf::is_char_boundary`] before slicing or mutating text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TextOffset(usize);

impl TextOffset {
    /// The zero offset.
    pub const ZERO: Self = Self(0);

    /// Creates an offset from a byte count.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the byte count.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Adds `amount` bytes, returning `None` on overflow.
    #[must_use]
    pub const fn checked_add(self, amount: usize) -> Option<Self> {
        match self.0.checked_add(amount) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Subtracts `amount` bytes, returning `None` on underflow.
    #[must_use]
    pub const fn checked_sub(self, amount: usize) -> Option<Self> {
        match self.0.checked_sub(amount) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl fmt::Display for TextOffset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<usize> for TextOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<TextOffset> for usize {
    fn from(value: TextOffset) -> Self {
        value.0
    }
}

/// A half-open byte range into UTF-8 text.
///
/// A range can be constructed with `start > end`; call [`TextBuf::validate_range`]
/// before using it for slicing or mutation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: TextOffset,
    end: TextOffset,
}

impl TextRange {
    /// Creates a range from `start` (inclusive) to `end` (exclusive).
    #[must_use]
    pub const fn new(start: TextOffset, end: TextOffset) -> Self {
        Self { start, end }
    }

    /// Creates an empty range at `offset`.
    #[must_use]
    pub const fn empty(offset: TextOffset) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Returns the inclusive start offset.
    #[must_use]
    pub const fn start(self) -> TextOffset {
        self.start
    }

    /// Returns the exclusive end offset.
    #[must_use]
    pub const fn end(self) -> TextOffset {
        self.end
    }

    /// Returns the byte length, or zero for a reversed range.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.get().saturating_sub(self.start.get())
    }

    /// Returns `true` when `start == end`.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.get() == self.end.get()
    }

    /// Returns `true` when `start <= end`.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.start.get() <= self.end.get()
    }

    /// Returns `true` when `offset` lies in `[start, end)`.
    #[must_use]
    pub const fn contains(self, offset: TextOffset) -> bool {
        self.start.get() <= offset.get() && offset.get() < self.end.get()
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}..{}", self.start, self.end)
    }
}

impl From<Range<usize>> for TextRange {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start.into(), range.end.into())
    }
}

impl From<TextRange> for Range<usize> {
    fn from(range: TextRange) -> Self {
        range.start().get()..range.end().get()
    }
}

/// A UTF-8 text buffer with validated byte boundaries.
///
/// The buffer owns its text and never exposes a mutable string reference.
/// All slicing and mutation goes through boundary-checked methods.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextBuf {
    text: String,
}

impl TextBuf {
    /// Creates an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the text as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Consumes the buffer and returns its owned string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.text
    }

    /// Returns the text length in UTF-8 bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` when the buffer contains no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns `true` when `offset` is a UTF-8 character boundary in this buffer.
    #[must_use]
    pub fn is_char_boundary(&self, offset: TextOffset) -> bool {
        offset.get() <= self.text.len() && self.text.is_char_boundary(offset.get())
    }

    /// Validates that `offset` is inside the buffer and on a character boundary.
    ///
    /// # Errors
    ///
    /// Returns [`TextError::OutOfBounds`] past the end of the buffer and
    /// [`TextError::NotCharBoundary`] inside a multi-byte code point.
    pub fn validate_offset(&self, offset: TextOffset) -> TextResult<()> {
        if offset.get() > self.text.len() {
            return Err(TextError::OutOfBounds {
                offset: offset.get(),
                len: self.text.len(),
            });
        }

        if !self.text.is_char_boundary(offset.get()) {
            return Err(TextError::NotCharBoundary {
                offset: offset.get(),
            });
        }

        Ok(())
    }

    /// Validates that `range` is ordered, inside the buffer, and on character boundaries.
    ///
    /// # Errors
    ///
    /// Returns [`TextError::InvalidRange`] for a reversed or out-of-bounds
    /// range, and a boundary error when an endpoint is not a character boundary.
    pub fn validate_range(&self, range: TextRange) -> TextResult<()> {
        if !range.is_valid() || range.end().get() > self.text.len() {
            return Err(TextError::InvalidRange {
                start: range.start().get(),
                end: range.end().get(),
                len: self.text.len(),
            });
        }

        self.validate_offset(range.start())?;
        self.validate_offset(range.end())?;
        Ok(())
    }

    /// Returns the text slice covered by `range`.
    ///
    /// # Errors
    ///
    /// Returns an error when the range is reversed, out of bounds, or not on
    /// UTF-8 character boundaries.
    pub fn slice(&self, range: TextRange) -> TextResult<&str> {
        self.validate_range(range)?;
        Ok(&self.text[range.start().get()..range.end().get()])
    }
}

impl fmt::Display for TextBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text.fmt(formatter)
    }
}

impl From<&str> for TextBuf {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_owned(),
        }
    }
}

impl From<String> for TextBuf {
    fn from(text: String) -> Self {
        Self { text }
    }
}

impl From<TextBuf> for String {
    fn from(buffer: TextBuf) -> Self {
        buffer.into_string()
    }
}

impl AsRef<str> for TextBuf {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::{TextBuf, TextOffset, TextRange};
    use crate::TextError;

    fn offset(value: usize) -> TextOffset {
        TextOffset::new(value)
    }

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(offset(start), offset(end))
    }

    #[test]
    fn empty_buffer_has_zero_length() {
        let buffer = TextBuf::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len_bytes(), 0);
        assert!(buffer.is_char_boundary(TextOffset::ZERO));
        assert_eq!(buffer.slice(range(0, 0)).unwrap(), "");
    }

    #[test]
    fn ascii_boundaries_are_valid() {
        let buffer = TextBuf::from("hello");
        assert!(buffer.is_char_boundary(offset(0)));
        assert!(buffer.is_char_boundary(offset(5)));
        assert!(!buffer.is_char_boundary(offset(6)));
        assert_eq!(buffer.slice(range(1, 4)).unwrap(), "ell");
    }

    #[test]
    fn cjk_slice_uses_byte_boundaries() {
        let buffer = TextBuf::from("你好世界");
        assert_eq!(buffer.len_bytes(), 12);
        assert_eq!(buffer.slice(range(0, 3)).unwrap(), "你");
        assert_eq!(buffer.slice(range(3, 6)).unwrap(), "好");
        assert!(matches!(
            buffer.slice(range(0, 1)),
            Err(TextError::NotCharBoundary { offset: 1 })
        ));
    }

    #[test]
    fn emoji_slice_uses_byte_boundaries() {
        let buffer = TextBuf::from("a😀b");
        assert_eq!(buffer.len_bytes(), 6);
        assert_eq!(buffer.slice(range(0, 1)).unwrap(), "a");
        assert_eq!(buffer.slice(range(1, 5)).unwrap(), "😀");
        assert_eq!(buffer.slice(range(5, 6)).unwrap(), "b");
    }

    #[test]
    fn reversed_range_is_rejected() {
        let buffer = TextBuf::from("abc");
        assert!(matches!(
            buffer.validate_range(range(2, 1)),
            Err(TextError::InvalidRange {
                start: 2,
                end: 1,
                len: 3
            })
        ));
    }

    #[test]
    fn out_of_bounds_range_is_rejected() {
        let buffer = TextBuf::from("abc");
        assert!(matches!(
            buffer.validate_range(range(1, 4)),
            Err(TextError::InvalidRange {
                start: 1,
                end: 4,
                len: 3
            })
        ));
        assert!(matches!(
            buffer.validate_offset(offset(4)),
            Err(TextError::OutOfBounds { offset: 4, len: 3 })
        ));
    }

    #[test]
    fn range_length_is_saturating() {
        assert_eq!(range(1, 4).len(), 3);
        assert_eq!(range(4, 1).len(), 0);
        assert!(!range(4, 1).is_valid());
        assert!(range(2, 2).is_empty());
        assert!(range(1, 3).contains(offset(1)));
        assert!(!range(1, 3).contains(offset(3)));
    }
}
