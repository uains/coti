//! Property tests for text boundary, grapheme, and UTF-16 invariants.

use md_core::{TextBuf, TextOffset, TextRange};
use proptest::prelude::*;

fn arb_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..64)
        .prop_map(|characters| characters.into_iter().collect())
}

fn valid_span(text: &str, raw_start: usize, raw_end: usize) -> (usize, usize) {
    let len = text.len();
    let mut start = raw_start % (len + 1);
    while !text.is_char_boundary(start) {
        start -= 1;
    }

    let mut end = raw_end % (len + 1);
    if end < start {
        end = start;
    }
    while !text.is_char_boundary(end) {
        end += 1;
    }

    (start, end)
}

proptest! {
    #[test]
    fn utf16_roundtrip(text in arb_text()) {
        let buffer = TextBuf::from(text.as_str());
        let units = buffer.to_utf16();
        prop_assert_eq!(units.len(), buffer.len_utf16());
        prop_assert_eq!(TextBuf::from_utf16(&units).unwrap(), buffer);
    }

    #[test]
    fn utf8_utf16_offsets_roundtrip(text in arb_text()) {
        let buffer = TextBuf::from(text.as_str());
        for (byte, _) in text.char_indices().chain(std::iter::once((text.len(), '\0'))) {
            let offset = TextOffset::new(byte);
            let units = buffer.utf8_to_utf16(offset).unwrap();
            prop_assert_eq!(buffer.utf16_to_utf8(units).unwrap(), offset);
        }
    }

    #[test]
    fn next_grapheme_walk_reaches_end(text in arb_text()) {
        let buffer = TextBuf::from(text.as_str());
        let mut offset = TextOffset::ZERO;
        let mut steps = 0usize;

        while let Some(next) = buffer.next_grapheme_boundary(offset) {
            prop_assert!(next > offset);
            offset = next;
            steps += 1;
            prop_assert!(steps <= text.len() + 1);
        }

        prop_assert_eq!(offset, TextOffset::new(text.len()));
    }

    #[test]
    fn prev_grapheme_walk_reaches_start(text in arb_text()) {
        let buffer = TextBuf::from(text.as_str());
        let mut offset = TextOffset::new(text.len());
        let mut steps = 0usize;

        while let Some(previous) = buffer.prev_grapheme_boundary(offset) {
            prop_assert!(previous < offset);
            offset = previous;
            steps += 1;
            prop_assert!(steps <= text.len() + 1);
        }

        prop_assert_eq!(offset, TextOffset::ZERO);
    }

    #[test]
    fn replacing_a_slice_with_itself_is_identity(
        text in arb_text(),
        raw_start in any::<usize>(),
        raw_end in any::<usize>(),
    ) {
        let (start, end) = valid_span(&text, raw_start, raw_end);
        let range = TextRange::new(TextOffset::new(start), TextOffset::new(end));
        let mut buffer = TextBuf::from(text.as_str());
        let slice = buffer.slice(range).unwrap().to_owned();

        buffer.replace(range, &slice).unwrap();
        prop_assert_eq!(buffer.as_str(), text.as_str());
    }

    #[test]
    fn insert_then_remove_roundtrip(
        text in arb_text(),
        inserted in arb_text(),
        raw_start in any::<usize>(),
    ) {
        let (start, _) = valid_span(&text, raw_start, raw_start);
        let mut buffer = TextBuf::from(text.as_str());
        buffer.insert(TextOffset::new(start), &inserted).unwrap();

        let range = TextRange::new(
            TextOffset::new(start),
            TextOffset::new(start + inserted.len()),
        );
        buffer.remove(range).unwrap();
        prop_assert_eq!(buffer.as_str(), text.as_str());
    }

    #[test]
    fn split_then_concat_roundtrip(text in arb_text(), raw_start in any::<usize>()) {
        let (start, _) = valid_span(&text, raw_start, raw_start);
        let mut prefix = TextBuf::from(text.as_str());
        let tail = prefix.split_off(TextOffset::new(start)).unwrap();

        prop_assert_eq!(prefix.concat(&tail), TextBuf::from(text.as_str()));
    }
}
