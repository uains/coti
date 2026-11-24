//! Property tests for document positions and selections.

use md_document::{
    Affinity, Align, AtomKind, BlockKind, DocPos, Document, InlineContent, Marks, Position,
    Selection, SelectionSet,
};
use proptest::collection::vec;
use proptest::prelude::*;

fn arb_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..24)
        .prop_map(|characters| characters.into_iter().collect())
}

fn build_document(texts: &[String]) -> Document {
    let mut document = Document::new();

    let mut inline = InlineContent::new();
    inline.push_text("a", Marks::plain());
    inline.push_atom(AtomKind::SoftBreak, Marks::plain());
    inline.push_text(texts.first().map_or("", String::as_str), Marks::plain());
    document
        .append_block(None, BlockKind::Paragraph(inline))
        .unwrap();

    document
        .append_block(None, BlockKind::Paragraph(InlineContent::new()))
        .unwrap();

    for text in texts.iter().skip(1) {
        document
            .append_block(
                None,
                BlockKind::Paragraph(InlineContent::from_text(text.as_str())),
            )
            .unwrap();
    }

    let quote = document
        .append_block(None, BlockKind::Quote(vec![]))
        .unwrap();
    document
        .append_block(
            Some(quote),
            BlockKind::Paragraph(InlineContent::from_text("quote")),
        )
        .unwrap();
    let inner_quote = document
        .append_block(Some(quote), BlockKind::Quote(vec![]))
        .unwrap();
    document
        .append_block(
            Some(inner_quote),
            BlockKind::Paragraph(InlineContent::from_text("deep")),
        )
        .unwrap();

    let list = document
        .append_block(
            None,
            BlockKind::List {
                ordered: false,
                start: 1,
                tight: true,
                items: vec![],
            },
        )
        .unwrap();
    let item = document
        .append_block(
            Some(list),
            BlockKind::ListItem {
                task: Some(false),
                children: vec![],
            },
        )
        .unwrap();
    let inner_list = document
        .append_block(
            Some(item),
            BlockKind::List {
                ordered: true,
                start: 1,
                tight: false,
                items: vec![],
            },
        )
        .unwrap();
    let inner_item = document
        .append_block(
            Some(inner_list),
            BlockKind::ListItem {
                task: None,
                children: vec![],
            },
        )
        .unwrap();
    document
        .append_block(
            Some(inner_item),
            BlockKind::Paragraph(InlineContent::from_text("nested")),
        )
        .unwrap();

    let table = document
        .append_block(
            None,
            BlockKind::Table {
                aligns: vec![Align::Left, Align::Right],
                rows: vec![],
            },
        )
        .unwrap();
    let row = document
        .append_block(Some(table), BlockKind::TableRow { cells: vec![] })
        .unwrap();
    for text in ["left", "right"] {
        document
            .append_block(
                Some(row),
                BlockKind::TableCell {
                    content: InlineContent::from_text(text),
                },
            )
            .unwrap();
    }

    assert!(
        document.validate().is_ok(),
        "property fixture must build a valid document"
    );
    document
}

proptest! {
    #[test]
    fn legal_doc_positions_roundtrip_with_an_affinity(texts in vec(arb_text(), 0..6)) {
        let document = build_document(&texts);
        let index = document.index();

        for pos in 0..=index.doc_length() {
            let Some(hit) = index.leaf_at(pos) else {
                continue;
            };
            let Some(kind) = document.kind(hit.block_id) else {
                continue;
            };
            if !kind.is_valid_leaf_offset(hit.offset) {
                continue;
            }

            let upstream = index
                .resolve_position(&document, DocPos::new(pos), Affinity::Upstream)
                .unwrap();
            let downstream = index
                .resolve_position(&document, DocPos::new(pos), Affinity::Downstream)
                .unwrap();

            let upstream_back = index.to_doc_pos(&document, upstream).unwrap();
            let downstream_back = index.to_doc_pos(&document, downstream).unwrap();

            prop_assert!(
                upstream_back == DocPos::new(pos) || downstream_back == DocPos::new(pos),
                "pos {} did not roundtrip: {:?} / {:?}",
                pos,
                upstream,
                downstream
            );
        }
    }

    #[test]
    fn every_leaf_boundary_converts_back(texts in vec(arb_text(), 0..6)) {
        let document = build_document(&texts);
        let index = document.index();

        for leaf_index in 0..index.leaf_count() {
            let block_id = index.leaf_order()[leaf_index];
            let kind = document.kind(block_id).unwrap();
            let len = kind.leaf_len().unwrap();
            let start = index.leaf_start(leaf_index).unwrap();

            for offset in 0..=len {
                if !kind.is_valid_leaf_offset(offset) {
                    continue;
                }

                let position = Position {
                    block_id,
                    offset,
                    affinity: Affinity::Downstream,
                };
                let pos = index.to_doc_pos(&document, position).unwrap();
                prop_assert_eq!(pos, DocPos::new(start + offset));

                let upstream = index
                    .resolve_position(&document, pos, Affinity::Upstream)
                    .unwrap();
                let downstream = index
                    .resolve_position(&document, pos, Affinity::Downstream)
                    .unwrap();
                let upstream_back = index.to_doc_pos(&document, upstream).unwrap();
                let downstream_back = index.to_doc_pos(&document, downstream).unwrap();
                prop_assert!(
                    upstream_back == pos || downstream_back == pos,
                    "leaf boundary {:?}+{} did not roundtrip",
                    block_id,
                    offset
                );
            }
        }
    }

    #[test]
    fn sorted_selection_sets_validate(texts in vec(arb_text(), 1..5)) {
        let document = build_document(&texts);
        let index = document.index();

        let mut positions = Vec::new();
        for leaf_index in 0..index.leaf_count() {
            positions.push(DocPos::new(index.leaf_start(leaf_index).unwrap()));
        }
        positions.sort();
        positions.dedup();

        let selections: Vec<_> = positions
            .into_iter()
            .map(|pos| Selection::collapsed(pos, Affinity::Upstream))
            .collect();
        let set = SelectionSet::from_selections(selections, 0).unwrap();

        prop_assert!(set.validate(&document, index).is_ok());
        for pair in set.selections().windows(2) {
            prop_assert!(pair[0].start() >= pair[1].start());
        }
    }
}
