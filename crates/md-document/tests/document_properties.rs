//! Property tests for document structure invariants.

use md_document::{BlockKind, Document, InlineContent};
use proptest::collection::vec;
use proptest::prelude::*;

fn arb_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..32)
        .prop_map(|characters| characters.into_iter().collect())
}

fn paragraph(text: &str) -> BlockKind {
    BlockKind::Paragraph(InlineContent::from_text(text))
}

proptest! {
    #[test]
    fn inserting_root_paragraphs_keeps_invariants(texts in vec(arb_text(), 1..12)) {
        let mut document = Document::new();
        for text in texts {
            let index = document.root_children().len();
            document
                .insert_block(None, index, paragraph(text.as_str()))
                .unwrap();
            prop_assert!(document.validate().is_ok());
        }
    }

    #[test]
    fn removing_root_blocks_keeps_root_valid(texts in vec(arb_text(), 1..12)) {
        let mut document = Document::new();
        for text in texts {
            document
                .append_block(None, paragraph(text.as_str()))
                .unwrap();
        }

        let ids = document.root_children().to_vec();
        for id in ids {
            prop_assert!(document.remove_block(id).is_ok());
            prop_assert!(document.validate().is_ok());
        }
        prop_assert_eq!(document.root_children().len(), 1);
    }

    #[test]
    fn moving_root_block_preserves_id(
        texts in vec(arb_text(), 2..8),
        raw_target in any::<usize>(),
    ) {
        let mut document = Document::new();
        for text in texts {
            document
                .append_block(None, paragraph(text.as_str()))
                .unwrap();
        }

        let id = document.root_children()[0];
        let len = document.root_children().len();
        let target = raw_target % (len + 1);
        document.move_block(id, None, target).unwrap();

        prop_assert!(document.node(id).is_some());
        prop_assert!(document.root_children().contains(&id));
        prop_assert!(document.validate().is_ok());
    }
}
