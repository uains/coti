//! Atomic transaction execution with snapshot rollback.

use md_document::BlockId;

use crate::{Editor, EditorError, EditorResult, Operation};

/// A pending set of operations based on a document revision.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    base_revision: u64,
    operations: Vec<Operation>,
}

impl Transaction {
    /// Creates a transaction against a document revision.
    #[must_use]
    pub const fn new(base_revision: u64) -> Self {
        Self {
            base_revision,
            operations: Vec::new(),
        }
    }

    /// Appends an operation.
    pub fn push(&mut self, operation: Operation) {
        self.operations.push(operation);
    }

    /// Returns the number of operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns `true` when no operations are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Applies all operations atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale base revision, an operation failure, or a
    /// document validation failure. In every failure case the document is
    /// restored to its pre-transaction snapshot.
    pub fn apply(self, editor: &mut Editor) -> EditorResult<ChangeSet> {
        if self.base_revision != editor.document_revision() {
            return Err(EditorError::StaleRevision {
                base: self.base_revision,
                current: editor.document_revision(),
            });
        }

        if self.operations.is_empty() {
            return Ok(ChangeSet {
                base_revision: self.base_revision,
                new_revision: self.base_revision,
                affected: Vec::new(),
                inverses: Vec::new(),
            });
        }

        let snapshot = editor.document().snapshot();
        let mut inverses = Vec::with_capacity(self.operations.len());
        let mut affected = Vec::new();

        for operation in &self.operations {
            match operation.apply(editor.document_mut()) {
                Ok(applied) => {
                    for id in applied.affected {
                        if !affected.contains(&id) {
                            affected.push(id);
                        }
                    }
                    inverses.push(applied.inverse);
                }
                Err(error) => {
                    editor.restore_document(snapshot);
                    return Err(error);
                }
            }
        }

        if let Err(error) = editor.document().validate() {
            editor.restore_document(snapshot);
            return Err(error.into());
        }

        editor.bump_document_revision();
        Ok(ChangeSet {
            base_revision: self.base_revision,
            new_revision: editor.document_revision(),
            affected,
            inverses,
        })
    }
}

/// Published result of a committed transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeSet {
    /// Document revision before the transaction.
    pub base_revision: u64,
    /// Document revision after commit.
    pub new_revision: u64,
    /// Distinct affected block ids.
    pub affected: Vec<BlockId>,
    /// Inverses in forward apply order; undo should apply them in reverse.
    pub inverses: Vec<Operation>,
}

#[cfg(test)]
mod tests {
    use super::Transaction;
    use crate::{Editor, Operation};
    use md_core::TextOffset;
    use md_document::{Align, BlockId, BlockKind, Document, InlineContent, Marks};

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn transaction_commits_and_returns_inverses() {
        let mut editor = Editor::new(Document::new());
        let root = editor.document().root_children()[0];
        let mut transaction = Transaction::new(0);
        transaction.push(Operation::SetInlineContent {
            block_id: root,
            content: InlineContent::from_text("changed"),
        });

        let changes = transaction.apply(&mut editor).unwrap();
        assert_eq!(changes.base_revision, 0);
        assert_eq!(changes.new_revision, 1);
        assert_eq!(changes.inverses.len(), 1);
        assert_eq!(editor.document_revision(), 1);
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(7));
    }

    #[test]
    fn stale_transaction_is_rejected_without_change() {
        let mut editor = Editor::new(Document::new());
        let root = editor.document().root_children()[0];
        let mut transaction = Transaction::new(9);
        transaction.push(Operation::SetInlineContent {
            block_id: root,
            content: InlineContent::from_text("x"),
        });

        assert!(matches!(
            transaction.apply(&mut editor),
            Err(crate::EditorError::StaleRevision {
                base: 9,
                current: 0
            })
        ));
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(0));
    }

    #[test]
    fn failing_operation_rolls_back_previous_operations() {
        let mut editor = Editor::new(Document::new());
        let root = editor.document().root_children()[0];
        let mut transaction = Transaction::new(0);
        transaction.push(Operation::SetInlineContent {
            block_id: root,
            content: InlineContent::from_text("changed"),
        });
        transaction.push(Operation::ReplaceText {
            block_id: BlockId::new(99, 1),
            range: md_core::TextRange::new(TextOffset::new(0), TextOffset::new(0)),
            text: "x".to_owned(),
            marks: Marks::plain(),
        });

        assert!(transaction.apply(&mut editor).is_err());
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(0));
    }

    #[test]
    fn validation_failure_rolls_back() {
        let mut editor = Editor::new(Document::new());
        let mut transaction = Transaction::new(0);
        transaction.push(Operation::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Table {
                aligns: vec![Align::Left],
                rows: vec![BlockId::new(99, 1)],
            },
        });

        assert!(transaction.apply(&mut editor).is_err());
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(editor.document().root_children().len(), 1);
    }

    #[test]
    fn empty_transaction_is_a_noop() {
        let mut editor = Editor::new(Document::new());
        let changes = Transaction::new(0).apply(&mut editor).unwrap();
        assert_eq!(changes.base_revision, 0);
        assert_eq!(changes.new_revision, 0);
        assert!(changes.affected.is_empty());
        assert!(changes.inverses.is_empty());
        assert_eq!(editor.document_revision(), 0);
    }

    #[test]
    fn transaction_uses_latest_revision() {
        let mut editor = Editor::new(Document::new());
        let root = editor.document().root_children()[0];
        let mut first = Transaction::new(0);
        first.push(Operation::SetInlineContent {
            block_id: root,
            content: InlineContent::from_text("a"),
        });
        first.apply(&mut editor).unwrap();

        let mut second = Transaction::new(1);
        second.push(Operation::InsertBlock {
            parent: None,
            index: 1,
            kind: paragraph("b"),
        });
        let changes = second.apply(&mut editor).unwrap();
        assert_eq!(changes.base_revision, 1);
        assert_eq!(changes.new_revision, 2);
    }
}
