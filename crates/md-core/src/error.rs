//! Error types for text and geometry primitives.

/// Errors produced while validating or mutating UTF-8 text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TextError {
    /// An offset is beyond the end of the text.
    #[error("text offset {offset} is out of bounds for {len} bytes")]
    OutOfBounds {
        /// The rejected byte offset.
        offset: usize,
        /// The current text length in bytes.
        len: usize,
    },

    /// An offset points inside a UTF-8 code point.
    #[error("text offset {offset} is not a UTF-8 character boundary")]
    NotCharBoundary {
        /// The rejected byte offset.
        offset: usize,
    },

    /// A range is reversed or extends past the end of the text.
    #[error("text range {start}..{end} is invalid for {len} bytes")]
    InvalidRange {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
        /// The current text length in bytes.
        len: usize,
    },

    /// UTF-16 input contains an unpaired surrogate.
    #[error("invalid UTF-16 input at code unit {index}")]
    InvalidUtf16 {
        /// Index of the first invalid UTF-16 code unit.
        index: usize,
    },
}

/// Result alias for text operations.
pub type TextResult<T> = Result<T, TextError>;

#[cfg(test)]
mod tests {
    use super::TextError;

    #[test]
    fn out_of_bounds_message_includes_offset_and_length() {
        let error = TextError::OutOfBounds { offset: 4, len: 3 };
        assert_eq!(
            error.to_string(),
            "text offset 4 is out of bounds for 3 bytes"
        );
    }

    #[test]
    fn invalid_range_message_includes_all_fields() {
        let error = TextError::InvalidRange {
            start: 4,
            end: 2,
            len: 8,
        };
        assert_eq!(error.to_string(), "text range 4..2 is invalid for 8 bytes");
    }

    #[test]
    fn invalid_utf16_message_includes_index() {
        let error = TextError::InvalidUtf16 { index: 2 };
        assert_eq!(error.to_string(), "invalid UTF-16 input at code unit 2");
    }
}
