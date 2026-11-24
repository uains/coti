//! Editor state and intent dispatch.

use md_document::{
    Affinity, Anchor, DocPos, Document, Marks, Selection, SelectionKind, SelectionSet,
};

use crate::intent::MoveDirection;
use crate::rule::{self, RuleId};
use crate::{EditorError, EditorResult, Intent};

/// Editor configuration that affects headless behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorConfig {
    /// Tab width for indentation and code blocks.
    pub tab_size: u8,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self { tab_size: 4 }
    }
}

/// Minimal IME composition state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Composition {
    /// Current pre-edit text.
    pub text: String,
}

/// Headless editor state.
#[derive(Clone, Debug)]
pub struct Editor {
    document: Document,
    selection: SelectionSet,
    config: EditorConfig,
    composition: Option<Composition>,
    pending_marks: Option<Marks>,
    document_revision: u64,
    state_revision: u64,
}

impl Editor {
    /// Creates an editor with a collapsed selection at the document start.
    #[must_use]
    pub fn new(document: Document) -> Self {
        let selection = SelectionSet::new(Selection::collapsed(DocPos::ZERO, Affinity::Upstream));
        Self {
            document,
            selection,
            config: EditorConfig::default(),
            composition: None,
            pending_marks: None,
            document_revision: 0,
            state_revision: 0,
        }
    }

    /// Creates an editor with a validated initial selection.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection is invalid for the document.
    pub fn with_selection(document: Document, selection: SelectionSet) -> EditorResult<Self> {
        selection.validate(&document, document.index())?;
        Ok(Self {
            document,
            selection,
            config: EditorConfig::default(),
            composition: None,
            pending_marks: None,
            document_revision: 0,
            state_revision: 0,
        })
    }

    /// Returns the document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns the current selection set.
    #[must_use]
    pub fn selection(&self) -> &SelectionSet {
        &self.selection
    }

    /// Returns the editor configuration.
    #[must_use]
    pub const fn config(&self) -> EditorConfig {
        self.config
    }

    /// Returns the pending inline marks.
    #[must_use]
    pub fn pending_marks(&self) -> Option<&Marks> {
        self.pending_marks.as_ref()
    }

    /// Returns the IME composition state.
    #[must_use]
    pub fn composition(&self) -> Option<&Composition> {
        self.composition.as_ref()
    }

    /// Returns the document revision.
    #[must_use]
    pub const fn document_revision(&self) -> u64 {
        self.document_revision
    }

    /// Returns the state revision.
    #[must_use]
    pub const fn state_revision(&self) -> u64 {
        self.state_revision
    }

    /// Replaces the selection after validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection is invalid.
    pub fn set_selection(&mut self, selection: SelectionSet) -> EditorResult<()> {
        selection.validate(&self.document, self.document.index())?;
        self.selection = selection;
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    /// Dispatches one semantic intent.
    ///
    /// # Errors
    ///
    /// Returns an error when no rule matches or the operation fails.
    pub fn dispatch(&mut self, intent: Intent) -> EditorResult<RuleId> {
        let rule_id =
            rule::match_rule(self, &intent).ok_or_else(|| EditorError::InvalidIntent {
                message: format!("no rule matched {}", intent.name()),
            })?;

        if rule_id == RuleId::MOVE_LEFT
            || rule_id == RuleId::MOVE_RIGHT
            || rule_id == RuleId::MOVE_DOC_START
            || rule_id == RuleId::MOVE_DOC_END
        {
            let Intent::Move { direction, extend } = intent else {
                return Err(EditorError::InvalidIntent {
                    message: format!("rule {rule_id} requires a Move intent"),
                });
            };
            self.apply_move(direction, extend)?;
            return Ok(rule_id);
        }

        Err(EditorError::InvalidIntent {
            message: format!("rule {rule_id} has no executor"),
        })
    }

    fn apply_move(&mut self, direction: MoveDirection, extend: bool) -> EditorResult<()> {
        let index = self.document.index();
        let primary = self.selection.primary();
        let mut selections = Vec::with_capacity(self.selection.len());
        let mut changed = false;

        for selection in self.selection.iter() {
            if !matches!(selection.kind, SelectionKind::Range) {
                return Err(EditorError::InvalidIntent {
                    message: "move only supports range selections".to_owned(),
                });
            }

            let next = match direction {
                MoveDirection::Left => index.prev_cursor(&self.document, selection.head.pos),
                MoveDirection::Right => index.next_cursor(&self.document, selection.head.pos),
                MoveDirection::DocStart => Some(DocPos::ZERO),
                MoveDirection::DocEnd => Some(DocPos::new(index.doc_length())),
            };

            let Some(next) = next else {
                selections.push(*selection);
                continue;
            };

            changed |= next != selection.head.pos;
            if extend {
                selections.push(Selection {
                    anchor: selection.anchor,
                    head: Anchor::new(next, Affinity::Downstream),
                    kind: SelectionKind::Range,
                    goal_x: None,
                });
            } else {
                selections.push(Selection::collapsed(next, Affinity::Downstream));
            }
        }

        if changed {
            self.selection = SelectionSet::from_selections(selections, primary)?;
            self.pending_marks = None;
            self.state_revision += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Composition, Editor};
    use crate::intent::MoveDirection::{DocEnd, DocStart, Left, Right};
    use crate::{Intent, RuleId};
    use md_document::{
        Affinity, Anchor, BlockKind, DocPos, Document, InlineContent, Selection, SelectionSet,
    };

    fn editor_at(pos: usize) -> Editor {
        let mut document = Document::new();
        document
            .append_block(None, BlockKind::Paragraph(InlineContent::from_text("a你")))
            .unwrap();
        document
            .append_block(None, BlockKind::Paragraph(InlineContent::from_text("b")))
            .unwrap();
        let selection =
            SelectionSet::new(Selection::collapsed(DocPos::new(pos), Affinity::Upstream));
        Editor::with_selection(document, selection).unwrap()
    }

    fn head(editor: &Editor) -> DocPos {
        editor.selection().primary_selection().unwrap().head.pos
    }

    #[test]
    fn move_left_within_leaf_moves_by_grapheme() {
        let mut editor = editor_at(5);
        assert_eq!(
            editor
                .dispatch(Intent::Move {
                    direction: Left,
                    extend: false
                })
                .unwrap(),
            RuleId::MOVE_LEFT
        );
        assert_eq!(head(&editor), DocPos::new(2));
    }

    #[test]
    fn move_left_at_document_start_is_noop() {
        let mut editor = editor_at(0);
        editor
            .dispatch(Intent::Move {
                direction: Left,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    #[test]
    fn move_left_crosses_leaf_boundary() {
        let mut editor = editor_at(6);
        editor
            .dispatch(Intent::Move {
                direction: Left,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(5));
    }

    #[test]
    fn move_right_within_leaf_moves_by_grapheme() {
        let mut editor = editor_at(1);
        editor
            .dispatch(Intent::Move {
                direction: Right,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(2));
    }

    #[test]
    fn move_right_at_document_end_is_noop() {
        let mut editor = editor_at(7);
        editor
            .dispatch(Intent::Move {
                direction: Right,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn move_right_crosses_leaf_boundary() {
        let mut editor = editor_at(5);
        editor
            .dispatch(Intent::Move {
                direction: Right,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(6));
    }

    #[test]
    fn move_doc_start_from_middle() {
        let mut editor = editor_at(5);
        editor
            .dispatch(Intent::Move {
                direction: DocStart,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    #[test]
    fn move_doc_start_at_start_is_noop() {
        let mut editor = editor_at(0);
        editor
            .dispatch(Intent::Move {
                direction: DocStart,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    #[test]
    fn move_doc_start_from_end() {
        let mut editor = editor_at(7);
        editor
            .dispatch(Intent::Move {
                direction: DocStart,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    #[test]
    fn move_doc_end_from_start() {
        let mut editor = editor_at(0);
        editor
            .dispatch(Intent::Move {
                direction: DocEnd,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn move_doc_end_from_middle() {
        let mut editor = editor_at(2);
        editor
            .dispatch(Intent::Move {
                direction: DocEnd,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn move_doc_end_at_end_is_noop() {
        let mut editor = editor_at(7);
        editor
            .dispatch(Intent::Move {
                direction: DocEnd,
                extend: false,
            })
            .unwrap();
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn extend_preserves_anchor_and_clears_goal_x() {
        let mut editor = editor_at(2);
        editor
            .set_selection(SelectionSet::new(Selection::range(
                Anchor::new(DocPos::new(1), Affinity::Upstream),
                Anchor::new(DocPos::new(2), Affinity::Upstream),
            )))
            .unwrap();
        editor
            .dispatch(Intent::Move {
                direction: Right,
                extend: true,
            })
            .unwrap();
        let selection = editor.selection().primary_selection().unwrap();
        assert_eq!(selection.anchor.pos, DocPos::new(1));
        assert_eq!(selection.head.pos, DocPos::new(5));
        assert_eq!(selection.goal_x, None);
    }

    #[test]
    fn unsupported_intent_is_rejected() {
        let mut editor = editor_at(0);
        assert!(matches!(
            editor.dispatch(Intent::InsertNewline),
            Err(crate::EditorError::InvalidIntent { .. })
        ));
    }

    #[test]
    fn new_editor_tracks_revisions_and_config() {
        let editor = Editor::new(Document::new());
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(editor.state_revision(), 0);
        assert_eq!(editor.config().tab_size, 4);
        assert_eq!(editor.composition(), None);
        assert_eq!(editor.pending_marks(), None);
        assert_eq!(Composition::default().text, "");
    }
}
