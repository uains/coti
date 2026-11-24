//! Property tests for editor transactions and text intents.

use md_document::{DocPos, Document, InlineContent};
use md_editor::{Editor, Intent, MoveDirection, Operation, Transaction};
use proptest::collection::vec;
use proptest::prelude::*;

fn arb_text() -> impl Strategy<Value = String> {
    vec(any::<char>(), 0..24).prop_map(|characters| characters.into_iter().collect())
}

proptest! {
    #[test]
    fn insert_and_delete_keep_document_valid(text in arb_text()) {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText(text)).unwrap();
        prop_assert!(editor.document().validate().is_ok());

        editor
            .dispatch(Intent::Move {
                direction: MoveDirection::DocEnd,
                extend: false,
            })
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        prop_assert!(editor.document().validate().is_ok());
    }

    #[test]
    fn enter_then_insert_keeps_document_valid(first in arb_text(), second in arb_text()) {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText(first)).unwrap();
        editor
            .dispatch(Intent::Move {
                direction: MoveDirection::DocEnd,
                extend: false,
            })
            .unwrap();
        editor.dispatch(Intent::InsertNewline).unwrap();
        editor.dispatch(Intent::InsertText(second)).unwrap();
        prop_assert!(editor.document().validate().is_ok());
        prop_assert!(editor.document_revision() >= 2);
    }

    #[test]
    fn stale_transaction_never_mutates_document(revision in any::<u64>()) {
        let mut editor = Editor::new(Document::new());
        let root = editor.document().root_children()[0];
        let before = editor.document().clone();
        let mut transaction = Transaction::new(revision);
        transaction.push(Operation::SetInlineContent {
            block_id: root,
            content: InlineContent::from_text("x"),
        });
        prop_assert!(transaction.apply(&mut editor).is_err());
        prop_assert_eq!(editor.document(), &before);
    }

    #[test]
    fn random_pointer_position_remains_valid(text in arb_text(), raw in any::<usize>()) {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText(text)).unwrap();
        let len = editor.document().index().doc_length();
        let pos = if len == 0 { 0 } else { raw % (len + 1) };
        editor
            .dispatch(Intent::Move {
                direction: MoveDirection::DocStart,
                extend: false,
            })
            .unwrap();
        for _ in 0..pos {
            editor
                .dispatch(Intent::Move {
                    direction: MoveDirection::Right,
                    extend: false,
                })
                .unwrap();
        }
        prop_assert!(editor.selection().validate(editor.document(), editor.document().index()).is_ok());
        prop_assert!(editor.selection().primary_selection().unwrap().head.pos <= DocPos::new(len));
    }
}
