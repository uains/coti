//! Random Undo/Redo completeness checks (AC-03).

use md_document::{BlockKind, Document, InlineContent};
use md_editor::{Editor, Intent, MoveDirection};

fn initial_document() -> Document {
    let mut document = Document::new();
    document
        .set_inline_content(
            document.root_children()[0],
            InlineContent::from_text("alpha"),
        )
        .unwrap();
    for text in ["beta", "gamma"] {
        document
            .append_block(None, BlockKind::Paragraph(InlineContent::from_text(text)))
            .unwrap();
    }
    document
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn pick(&mut self, max: u64) -> u64 {
        if max == 0 { 0 } else { self.next() % max }
    }
}

fn random_intent(rng: &mut Lcg) -> Intent {
    match rng.pick(8) {
        0 => Intent::InsertText(["a", "你", " ", "xy"][rng.pick(4) as usize].to_owned()),
        1 => Intent::DeleteBackward,
        2 => Intent::DeleteForward,
        3 => Intent::InsertNewline,
        4 => Intent::Move {
            direction: MoveDirection::Left,
            extend: false,
        },
        5 => Intent::Move {
            direction: MoveDirection::Right,
            extend: false,
        },
        6 => Intent::Move {
            direction: MoveDirection::DocEnd,
            extend: false,
        },
        _ => Intent::Move {
            direction: MoveDirection::DocStart,
            extend: false,
        },
    }
}

#[test]
fn random_edits_undo_to_initial_and_redo_to_final() {
    let initial = initial_document();
    let mut rng = Lcg(0xC071_2026);
    for sequence in 0..1000 {
        let mut editor = Editor::new(initial.clone());
        let initial_selection = editor.selection().clone();
        let mut first_edit_selection = None;
        for _ in 0..24 {
            let before_selection = editor.selection().clone();
            let before_revision = editor.document_revision();
            let _ = editor.dispatch(random_intent(&mut rng));
            if first_edit_selection.is_none() && editor.document_revision() != before_revision {
                first_edit_selection = Some(before_selection);
            }
        }
        let expected_undo_selection =
            first_edit_selection.unwrap_or_else(|| initial_selection.clone());
        let final_document = editor.document().clone();

        let mut undone = 0;
        loop {
            match editor.undo() {
                Ok(true) => {
                    undone += 1;
                    assert!(undone < 1000, "undo did not terminate");
                }
                Ok(false) => break,
                Err(error) => panic!("sequence {sequence} undo {undone} failed: {error}"),
            }
        }
        assert!(
            editor.document().content_eq(&initial),
            "sequence {sequence} did not restore the initial document"
        );
        assert_eq!(
            editor.selection(),
            &expected_undo_selection,
            "sequence {sequence} did not restore the pre-edit selection"
        );

        loop {
            match editor.redo() {
                Ok(true) => assert!(undone > 0),
                Ok(false) => break,
                Err(error) => panic!("sequence {sequence} redo failed: {error}"),
            }
        }
        assert!(
            editor.document().content_eq(&final_document),
            "sequence {sequence} did not redo to the final document"
        );
    }
}

#[test]
fn repeated_undo_without_changes_is_false() {
    let mut editor = Editor::new(initial_document());
    assert!(!editor.undo().unwrap());
    assert!(!editor.redo().unwrap());
}
