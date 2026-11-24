//! Editor state and intent dispatch.

use md_core::{TextOffset, TextRange};
use md_document::{
    Affinity, Anchor, AtomKind, BlockKind, CellPos, DocPos, Document, DocumentSnapshot, Marks,
    Selection, SelectionKind, SelectionSet,
};

use crate::history::{GroupKind, History, HistoryEntry, HistoryGroup};
use crate::intent::MoveDirection;
use crate::rule::{self, RuleId};
use crate::{ChangeSet, EditorError, EditorResult, Intent, Operation, Transaction};

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
    history: History,
    pending_change: Option<ChangeSet>,
    last_copied: String,
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
            history: History::default(),
            pending_change: None,
            last_copied: String::new(),
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
            history: History::default(),
            pending_change: None,
            last_copied: String::new(),
        })
    }

    /// Creates an editor with a specific history configuration.
    #[must_use]
    pub fn with_history(document: Document, history: History) -> Self {
        let mut editor = Self::new(document);
        editor.history = history;
        editor
    }

    /// Returns the history.
    #[must_use]
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Returns whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Returns whether the document is modified relative to the save point.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    /// Undoes the latest change.
    ///
    /// # Errors
    ///
    /// Returns an error if applying the inverse fails.
    pub fn undo(&mut self) -> EditorResult<bool> {
        let mut history = std::mem::take(&mut self.history);
        let result = history.undo(self);
        self.history = history;
        result
    }

    /// Redoes the latest undone change.
    ///
    /// # Errors
    ///
    /// Returns an error if applying the forward operations fails.
    pub fn redo(&mut self) -> EditorResult<bool> {
        let mut history = std::mem::take(&mut self.history);
        let result = history.redo(self);
        self.history = history;
        result
    }

    /// Sets the IME marked text.
    pub fn ime_set_marked(&mut self, text: &str) {
        self.composition = Some(Composition {
            text: text.to_owned(),
        });
        self.state_revision += 1;
    }

    /// Cancels the IME composition.
    pub fn ime_cancel(&mut self) {
        self.composition = None;
        self.state_revision += 1;
    }

    /// Commits the IME composition as one history entry.
    ///
    /// # Errors
    ///
    /// Returns an error when inserting the committed text fails.
    pub fn ime_commit(&mut self, text: &str) -> EditorResult<()> {
        let committed = if text.is_empty() {
            self.composition
                .take()
                .map_or_else(String::new, |composition| composition.text)
        } else {
            self.composition = None;
            text.to_owned()
        };
        if committed.is_empty() {
            return Ok(());
        }
        self.insert_text(&committed)
    }

    pub(crate) fn commit(&mut self, transaction: Transaction) -> EditorResult<ChangeSet> {
        let changes = transaction.apply(self)?;
        if let Some(existing) = self.pending_change.as_mut() {
            existing.forward.extend(changes.forward);
            existing.inverses.extend(changes.inverses);
            for id in changes.affected {
                if !existing.affected.contains(&id) {
                    existing.affected.push(id);
                }
            }
            existing.new_revision = changes.new_revision;
            return Ok(existing.clone());
        }
        self.pending_change = Some(changes.clone());
        Ok(changes)
    }

    /// Returns the last plain-text copy payload.
    #[must_use]
    pub fn copied_text(&self) -> &str {
        &self.last_copied
    }

    /// Returns the plain-text projection of the primary selection.
    #[must_use]
    pub fn copy_text(&self) -> String {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return String::new();
        };
        if !matches!(selection.kind, SelectionKind::Range) || selection.is_collapsed() {
            return String::new();
        }
        let index = self.document.index();
        let Ok(start) =
            index.resolve_position(&self.document, selection.start(), Affinity::Downstream)
        else {
            return String::new();
        };
        let Ok(end) = index.resolve_position(&self.document, selection.end(), Affinity::Upstream)
        else {
            return String::new();
        };
        if start.block_id == end.block_id {
            return self.block_text_range(start.block_id, start.offset, end.offset);
        }
        let Some(start_leaf) = index.leaf_index(start.block_id) else {
            return String::new();
        };
        let Some(end_leaf) = index.leaf_index(end.block_id) else {
            return String::new();
        };
        let leaves = index.leaf_order().to_vec();
        let mut parts = Vec::new();
        for (leaf_index, leaf) in leaves
            .iter()
            .enumerate()
            .take(end_leaf + 1)
            .skip(start_leaf)
        {
            let leaf = *leaf;
            let from = if leaf_index == start_leaf {
                start.offset
            } else {
                0
            };
            let to = if leaf_index == end_leaf {
                end.offset
            } else {
                self.document
                    .kind(leaf)
                    .and_then(BlockKind::leaf_len)
                    .unwrap_or(0)
            };
            parts.push(self.block_text_range(leaf, from, to));
        }
        parts.join("\n")
    }

    fn block_text_range(&self, block: md_document::BlockId, start: usize, end: usize) -> String {
        let Some(kind) = self.document.kind(block) else {
            return String::new();
        };
        if let Some(content) = kind.inline_content() {
            let Some((_, right)) = content.split_at(start) else {
                return String::new();
            };
            let Some((middle, _)) = right.split_at(end.saturating_sub(start)) else {
                return String::new();
            };
            return middle.plain_text();
        }
        if let BlockKind::CodeBlock { text, .. } = kind {
            return text.as_str().get(start..end).unwrap_or_default().to_owned();
        }
        String::new()
    }

    /// Pastes plain text, splitting blank lines into paragraphs.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection or insertion fails.
    pub fn paste_text(&mut self, text: &str) -> EditorResult<()> {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if normalized.is_empty() {
            return Ok(());
        }
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "paste requires a selection".to_owned(),
            });
        };
        if !selection.is_collapsed() {
            self.delete_range(selection.start(), selection.end())?;
        }
        for (index, chunk) in normalized.split("\n\n").enumerate() {
            if index > 0 {
                self.insert_newline()?;
            }
            let chunk = chunk.trim_end_matches('\n');
            if !chunk.is_empty() {
                self.insert_text(chunk)?;
            }
        }
        Ok(())
    }

    /// Returns the document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub(crate) fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub(crate) fn restore_document(&mut self, snapshot: DocumentSnapshot) {
        self.document.restore(snapshot);
    }

    pub(crate) fn restore_full_document(&mut self, document: Document) {
        self.document = document;
        self.document_revision += 1;
    }

    pub(crate) fn bump_document_revision(&mut self) {
        self.document_revision += 1;
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
        let before_selection = self.selection.clone();
        let before_revision = self.document_revision();
        let before_document = self.document.clone();
        self.pending_change = None;
        let rule_id = self.dispatch_inner(&intent)?;
        self.record_history(before_selection, before_revision, before_document, &intent);
        Ok(rule_id)
    }

    fn record_history(
        &mut self,
        before_selection: SelectionSet,
        before_revision: u64,
        before_document: Document,
        intent: &Intent,
    ) {
        let Some(change) = self.pending_change.take() else {
            return;
        };
        if change.new_revision == before_revision {
            return;
        }
        let group = if matches!(intent, Intent::InsertText(_))
            && change.affected.len() == 1
            && self
                .selection
                .primary_selection()
                .is_some_and(|selection| selection.is_collapsed())
        {
            Some(HistoryGroup {
                kind: GroupKind::TextInsert,
                block_id: change.affected[0],
            })
        } else {
            None
        };
        let before_state = self.history.current_state();
        let entry = HistoryEntry::from_change(
            change,
            before_selection,
            self.selection.clone(),
            before_document,
            self.document.clone(),
            group,
            before_state,
            before_state + 1,
        );
        self.history.push(entry);
    }

    fn dispatch_inner(&mut self, intent: &Intent) -> EditorResult<RuleId> {
        let rule_id = rule::match_rule(self, intent).ok_or_else(|| EditorError::InvalidIntent {
            message: format!("no rule matched {}", intent.name()),
        })?;

        if rule_id == RuleId::UNDO {
            self.undo()?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::REDO {
            self.redo()?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::IME_SET_MARKED {
            let Intent::ImeSetMarked(text) = intent else {
                return Err(EditorError::InvalidIntent {
                    message: "IME marked rule requires ImeSetMarked".to_owned(),
                });
            };
            self.ime_set_marked(text);
            return Ok(rule_id);
        }

        if rule_id == RuleId::IME_COMMIT {
            let Intent::ImeCommit(text) = intent else {
                return Err(EditorError::InvalidIntent {
                    message: "IME commit rule requires ImeCommit".to_owned(),
                });
            };
            self.ime_commit(text)?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::IME_CANCEL {
            self.ime_cancel();
            return Ok(rule_id);
        }

        if rule_id == RuleId::COPY {
            self.last_copied = self.copy_text();
            return Ok(rule_id);
        }

        if rule_id == RuleId::PASTE {
            let Intent::Paste(text) = intent else {
                return Err(EditorError::InvalidIntent {
                    message: "paste rule requires Paste".to_owned(),
                });
            };
            self.paste_text(text)?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::INSERT_TEXT {
            let Intent::InsertText(text) = intent else {
                return Err(EditorError::InvalidIntent {
                    message: "insert rule requires InsertText".to_owned(),
                });
            };
            self.insert_text(text)?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::DELETE_BACKWARD {
            self.delete_backward()?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::ENTER {
            self.insert_newline()?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::DELETE_FORWARD {
            self.delete_forward()?;
            return Ok(rule_id);
        }

        if rule_id == RuleId::CHANGE_INDENT {
            let Intent::ChangeIndent { outdent } = intent else {
                return Err(EditorError::InvalidIntent {
                    message: "indent rule requires ChangeIndent".to_owned(),
                });
            };
            self.change_indent(*outdent)?;
            return Ok(rule_id);
        }

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
            self.apply_move(*direction, *extend)?;
            return Ok(rule_id);
        }

        Err(EditorError::InvalidIntent {
            message: format!("rule {rule_id} has no executor"),
        })
    }

    fn change_indent(&mut self, outdent: bool) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "indent requires a selection".to_owned(),
            });
        };
        if let SelectionKind::TableCells { table, start, .. } = selection.kind {
            let Some(BlockKind::Table { aligns, rows }) = self.document.kind(table) else {
                return Ok(());
            };
            let columns = aligns.len();
            if outdent {
                if start.column > 0 {
                    self.select_table_cell(table, start.row, start.column - 1)?;
                } else if start.row > 0 {
                    self.select_table_cell(table, start.row - 1, columns - 1)?;
                }
            } else if start.column + 1 < columns {
                self.select_table_cell(table, start.row, start.column + 1)?;
            } else if start.row + 1 < rows.len() {
                self.select_table_cell(table, start.row + 1, 0)?;
            } else {
                self.append_table_row_and_select(table)?;
            }
            return Ok(());
        }
        if !selection.is_collapsed() {
            return Err(EditorError::InvalidIntent {
                message: "indent on a non-collapsed selection is not implemented".to_owned(),
            });
        }

        let index = self.document.index();
        let resolved =
            index.resolve_position(&self.document, selection.head.pos, Affinity::Downstream)?;

        if let Some((table, row, column, columns)) = self.table_cell_context(resolved.block_id) {
            if outdent {
                if column > 0 {
                    self.select_table_cell(table, row, column - 1)?;
                } else if row > 0 {
                    self.select_table_cell(table, row - 1, columns - 1)?;
                }
            } else if column + 1 < columns {
                self.select_table_cell(table, row, column + 1)?;
            } else {
                let has_next_row = matches!(
                    self.document.kind(table),
                    Some(BlockKind::Table { rows, .. }) if row + 1 < rows.len()
                );
                if has_next_row {
                    self.select_table_cell(table, row + 1, 0)?;
                } else {
                    self.append_table_row_and_select(table)?;
                }
            }
            return Ok(());
        }

        if let Some(item) = self.nearest_list_item(resolved.block_id) {
            let operation = if outdent {
                Operation::OutdentListItem { id: item }
            } else {
                Operation::IndentListItem { id: item }
            };
            let mut transaction = Transaction::new(self.document_revision());
            transaction.push(operation);
            self.commit(transaction)?;

            let index = self.document.index();
            let leaf = index
                .leaf_order()
                .iter()
                .find(|leaf| **leaf == item || self.document.is_ancestor(item, **leaf))
                .copied()
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "list item has no editable leaf".to_owned(),
                })?;
            let leaf_index = index
                .leaf_index(leaf)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "list item leaf has no index".to_owned(),
                })?;
            let pos = index
                .leaf_start(leaf_index)
                .map(DocPos::new)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "list item leaf has no position".to_owned(),
                })?;
            self.selection = SelectionSet::new(Selection::collapsed(pos, Affinity::Downstream));
            self.pending_marks = None;
            self.state_revision += 1;
            return Ok(());
        }

        let Some(BlockKind::CodeBlock { text, .. }) =
            self.document.kind(resolved.block_id).cloned()
        else {
            return Ok(());
        };

        let (range, new_pos) = if outdent {
            if resolved.offset == 0
                || text.as_str().as_bytes().get(resolved.offset - 1) != Some(&b'\t')
            {
                return Ok(());
            }
            (
                TextRange::new(
                    TextOffset::new(resolved.offset - 1),
                    TextOffset::new(resolved.offset),
                ),
                resolved.offset - 1,
            )
        } else {
            (
                TextRange::new(
                    TextOffset::new(resolved.offset),
                    TextOffset::new(resolved.offset),
                ),
                resolved.offset + 1,
            )
        };

        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(Operation::ReplaceText {
            block_id: resolved.block_id,
            range,
            text: if outdent {
                String::new()
            } else {
                "\t".to_owned()
            },
            marks: Marks::plain(),
        });
        self.commit(transaction)?;

        let new_pos = DocPos::new(new_pos);
        self.selection = SelectionSet::new(Selection::collapsed(new_pos, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn delete_range(&mut self, from: DocPos, to: DocPos) -> EditorResult<()> {
        let index = self.document.index();
        let start = index.resolve_position(&self.document, from, Affinity::Downstream)?;
        let end = index.resolve_position(&self.document, to, Affinity::Upstream)?;

        if start.block_id == end.block_id {
            let (block_id, range) = self.primary_local_range(from, to)?;
            let operation = Operation::ReplaceText {
                block_id,
                range,
                text: String::new(),
                marks: Marks::plain(),
            };
            let mut transaction = Transaction::new(self.document_revision());
            transaction.push(operation);
            self.commit(transaction)?;
            self.selection = SelectionSet::new(Selection::collapsed(from, Affinity::Downstream));
            self.pending_marks = None;
            self.state_revision += 1;
            return Ok(());
        }

        let start_leaf =
            index
                .leaf_index(start.block_id)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "range start is not a leaf".to_owned(),
                })?;
        let end_leaf =
            index
                .leaf_index(end.block_id)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "range end is not a leaf".to_owned(),
                })?;
        if start_leaf >= end_leaf {
            return Err(EditorError::InvalidIntent {
                message: "range endpoints are not ordered".to_owned(),
            });
        }
        let parent = self.document.parent(start.block_id);
        if parent != self.document.parent(end.block_id) {
            return Err(EditorError::InvalidIntent {
                message: "cross-container deletion is not implemented".to_owned(),
            });
        }
        let leaf_ids = index.leaf_order().to_vec();
        let first_len = self
            .document
            .kind(start.block_id)
            .and_then(BlockKind::leaf_len)
            .ok_or_else(|| EditorError::InvalidIntent {
                message: "range start is not a text leaf".to_owned(),
            })?;
        for leaf in &leaf_ids[start_leaf..=end_leaf] {
            if self.document.parent(*leaf) != parent
                || !matches!(
                    self.document.kind(*leaf),
                    Some(BlockKind::Paragraph(_) | BlockKind::Heading { .. })
                )
            {
                return Err(EditorError::InvalidIntent {
                    message: "cross-block deletion requires sibling inline leaves".to_owned(),
                });
            }
        }

        let mut transaction = Transaction::new(self.document_revision());
        for leaf in &leaf_ids[start_leaf + 1..end_leaf] {
            transaction.push(Operation::RemoveBlock { id: *leaf });
        }
        transaction.push(Operation::ReplaceText {
            block_id: start.block_id,
            range: TextRange::new(TextOffset::new(start.offset), TextOffset::new(first_len)),
            text: String::new(),
            marks: Marks::plain(),
        });
        transaction.push(Operation::ReplaceText {
            block_id: end.block_id,
            range: TextRange::new(TextOffset::new(0), TextOffset::new(end.offset)),
            text: String::new(),
            marks: Marks::plain(),
        });
        transaction.push(Operation::JoinBlocks {
            left: start.block_id,
            right: end.block_id,
        });
        self.commit(transaction)?;

        self.selection = SelectionSet::new(Selection::collapsed(from, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn delete_forward(&mut self) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "delete forward requires a selection".to_owned(),
            });
        };
        if !matches!(selection.kind, SelectionKind::Range) {
            return Err(EditorError::InvalidIntent {
                message: "delete forward only supports range selections".to_owned(),
            });
        }

        let (from, to) = if selection.is_collapsed() {
            let Some(next) = self
                .document
                .index()
                .next_cursor(&self.document, selection.head.pos)
            else {
                return Ok(());
            };
            (selection.head.pos, next)
        } else {
            self.delete_range(selection.start(), selection.end())?;
            return Ok(());
        };

        let (block_id, range) = self.primary_local_range(from, to)?;
        let operation = Operation::ReplaceText {
            block_id,
            range,
            text: String::new(),
            marks: Marks::plain(),
        };
        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(operation);
        self.commit(transaction)?;

        self.selection = SelectionSet::new(Selection::collapsed(from, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn insert_newline(&mut self) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "enter requires a selection".to_owned(),
            });
        };
        if !selection.is_collapsed() {
            return Err(EditorError::InvalidIntent {
                message: "enter on a non-collapsed selection is not implemented".to_owned(),
            });
        }

        let index = self.document.index();
        let resolved =
            index.resolve_position(&self.document, selection.head.pos, Affinity::Downstream)?;
        let block_id = resolved.block_id;
        let kind =
            self.document
                .kind(block_id)
                .cloned()
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "enter target is stale".to_owned(),
                })?;

        if let BlockKind::TableCell { content } = kind {
            let mut content = content;
            content.insert_atom_at(resolved.offset, AtomKind::HardBreak, Marks::plain())?;
            let mut transaction = Transaction::new(self.document_revision());
            transaction.push(Operation::SetInlineContent { block_id, content });
            self.commit(transaction)?;
            let new_pos = DocPos::new(selection.head.pos.get() + 1);
            self.selection = SelectionSet::new(Selection::collapsed(new_pos, Affinity::Downstream));
            self.pending_marks = None;
            self.state_revision += 1;
            return Ok(());
        }

        let Some(content) = kind.inline_content() else {
            return Err(EditorError::InvalidIntent {
                message: "enter only supports inline leaves".to_owned(),
            });
        };
        let (left, right) =
            content
                .split_at(resolved.offset)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "could not split inline content at caret".to_owned(),
                })?;

        let parent = self.document.parent(block_id);
        let child_index = match parent {
            Some(parent_id) => self
                .document
                .children(parent_id)
                .and_then(|children| children.iter().position(|id| *id == block_id))
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "enter target is not linked from its parent".to_owned(),
                })?,
            None => self
                .document
                .root_children()
                .iter()
                .position(|id| *id == block_id)
                .ok_or_else(|| EditorError::InvalidIntent {
                    message: "enter target is not linked from the root".to_owned(),
                })?,
        };

        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(Operation::SetInlineContent {
            block_id,
            content: left,
        });
        transaction.push(Operation::InsertBlock {
            parent,
            index: child_index + 1,
            kind: BlockKind::Paragraph(right),
        });
        self.commit(transaction)?;

        let new_id = match parent {
            Some(parent_id) => self.document.children(parent_id).unwrap()[child_index + 1],
            None => self.document.root_children()[child_index + 1],
        };
        let index = self.document.index();
        let new_leaf = index
            .leaf_index(new_id)
            .ok_or_else(|| EditorError::InvalidIntent {
                message: "new paragraph is not a leaf".to_owned(),
            })?;
        let new_pos = index.leaf_start(new_leaf).map(DocPos::new).ok_or_else(|| {
            EditorError::InvalidIntent {
                message: "new paragraph has no document position".to_owned(),
            }
        })?;

        self.selection = SelectionSet::new(Selection::collapsed(new_pos, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn insert_text(&mut self, text: &str) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "insert requires a selection".to_owned(),
            });
        };
        if !matches!(selection.kind, SelectionKind::Range) {
            return Err(EditorError::InvalidIntent {
                message: "insert only supports range selections".to_owned(),
            });
        }

        let (block_id, range) = self.primary_local_range(selection.start(), selection.end())?;
        let marks = self.pending_marks.clone().unwrap_or_else(Marks::plain);
        let operation = Operation::ReplaceText {
            block_id,
            range,
            text: text.to_owned(),
            marks,
        };
        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(operation);
        self.commit(transaction)?;

        let new_pos = DocPos::new(selection.start().get() + text.len());
        self.selection = SelectionSet::new(Selection::collapsed(new_pos, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        self.try_heading_input_rule()?;
        Ok(())
    }

    fn try_heading_input_rule(&mut self) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Ok(());
        };
        if !selection.is_collapsed() {
            return Ok(());
        }
        let index = self.document.index();
        let resolved =
            index.resolve_position(&self.document, selection.head.pos, Affinity::Downstream)?;
        let Some(BlockKind::Paragraph(content)) = self.document.kind(resolved.block_id).cloned()
        else {
            return Ok(());
        };
        let text = content.plain_text();
        let level = text
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if !(1..=6).contains(&level) || text.as_bytes().get(level) != Some(&b' ') {
            return Ok(());
        }
        let prefix = level + 1;
        if resolved.offset < prefix {
            return Ok(());
        }
        let Some((_, heading_content)) = content.split_at(prefix) else {
            return Ok(());
        };
        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(Operation::SetBlockKind {
            id: resolved.block_id,
            kind: BlockKind::Heading {
                level: level as u8,
                content: heading_content,
            },
        });
        self.commit(transaction)?;
        let new_pos = DocPos::new(selection.head.pos.get() - prefix);
        self.selection = SelectionSet::new(Selection::collapsed(new_pos, Affinity::Downstream));
        Ok(())
    }

    fn delete_backward(&mut self) -> EditorResult<()> {
        let Some(selection) = self.selection.primary_selection().copied() else {
            return Err(EditorError::InvalidIntent {
                message: "delete requires a selection".to_owned(),
            });
        };
        if !matches!(selection.kind, SelectionKind::Range) {
            return Err(EditorError::InvalidIntent {
                message: "delete only supports range selections".to_owned(),
            });
        }

        let (from, to) = if selection.is_collapsed() {
            let index = self.document.index();
            let resolved =
                index.resolve_position(&self.document, selection.head.pos, Affinity::Downstream)?;
            if resolved.offset == 0
                && let Some((table, row, column, columns)) =
                    self.table_cell_context(resolved.block_id)
            {
                let previous = if column > 0 {
                    Some((row, column - 1))
                } else if row > 0 {
                    Some((row - 1, columns - 1))
                } else {
                    None
                };
                if let Some((previous_row, previous_column)) = previous
                    && let Some(previous_cell) =
                        self.table_cell_id(table, previous_row, previous_column)
                {
                    let leaf_index = index.leaf_index(previous_cell).ok_or_else(|| {
                        EditorError::InvalidIntent {
                            message: "previous table cell is not a leaf".to_owned(),
                        }
                    })?;
                    let start =
                        index
                            .leaf_start(leaf_index)
                            .ok_or_else(|| EditorError::InvalidIntent {
                                message: "previous table cell has no position".to_owned(),
                            })?;
                    let len =
                        index
                            .leaf_len(leaf_index)
                            .ok_or_else(|| EditorError::InvalidIntent {
                                message: "previous table cell has no length".to_owned(),
                            })?;
                    self.set_selection(SelectionSet::new(Selection::collapsed(
                        DocPos::new(start + len),
                        Affinity::Upstream,
                    )))?;
                }
                return Ok(());
            }

            if resolved.offset == 0
                && let Some((_, 0)) = self.quote_context(resolved.block_id)
            {
                let mut transaction = Transaction::new(self.document_revision());
                transaction.push(Operation::OutdentQuoteBlock {
                    id: resolved.block_id,
                });
                self.commit(transaction)?;

                let index = self.document.index();
                let leaf = index
                    .leaf_order()
                    .iter()
                    .find(|leaf| {
                        **leaf == resolved.block_id
                            || self.document.is_ancestor(resolved.block_id, **leaf)
                    })
                    .copied()
                    .ok_or_else(|| EditorError::InvalidIntent {
                        message: "quote child has no editable leaf".to_owned(),
                    })?;
                let leaf_index =
                    index
                        .leaf_index(leaf)
                        .ok_or_else(|| EditorError::InvalidIntent {
                            message: "quote child leaf has no index".to_owned(),
                        })?;
                let pos = index
                    .leaf_start(leaf_index)
                    .map(DocPos::new)
                    .ok_or_else(|| EditorError::InvalidIntent {
                        message: "quote child leaf has no position".to_owned(),
                    })?;
                self.selection = SelectionSet::new(Selection::collapsed(pos, Affinity::Downstream));
                self.pending_marks = None;
                self.state_revision += 1;
                return Ok(());
            }

            let Some(previous) = index.prev_cursor(&self.document, selection.head.pos) else {
                return Ok(());
            };
            (previous, selection.head.pos)
        } else {
            self.delete_range(selection.start(), selection.end())?;
            return Ok(());
        };

        let (block_id, range) = self.primary_local_range(from, to)?;
        let operation = Operation::ReplaceText {
            block_id,
            range,
            text: String::new(),
            marks: Marks::plain(),
        };
        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(operation);
        self.commit(transaction)?;

        self.selection = SelectionSet::new(Selection::collapsed(from, Affinity::Downstream));
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn quote_context(&self, block: md_document::BlockId) -> Option<(md_document::BlockId, usize)> {
        let quote = self.document.parent(block)?;
        if !matches!(self.document.kind(quote), Some(BlockKind::Quote(_))) {
            return None;
        }
        let index = self
            .document
            .children(quote)?
            .iter()
            .position(|child| *child == block)?;
        Some((quote, index))
    }

    fn table_cell_context(
        &self,
        block: md_document::BlockId,
    ) -> Option<(md_document::BlockId, usize, usize, usize)> {
        if !matches!(self.document.kind(block), Some(BlockKind::TableCell { .. })) {
            return None;
        }
        let row = self.document.parent(block)?;
        if !matches!(self.document.kind(row), Some(BlockKind::TableRow { .. })) {
            return None;
        }
        let table = self.document.parent(row)?;
        let Some(BlockKind::Table { aligns, rows }) = self.document.kind(table) else {
            return None;
        };
        let row_index = rows.iter().position(|id| *id == row)?;
        let column_index = self
            .document
            .children(row)?
            .iter()
            .position(|id| *id == block)?;
        Some((table, row_index, column_index, aligns.len()))
    }

    fn table_cell_id(
        &self,
        table: md_document::BlockId,
        row: usize,
        column: usize,
    ) -> Option<md_document::BlockId> {
        let row = *self.document.children(table)?.get(row)?;
        self.document.children(row)?.get(column).copied()
    }

    fn select_table_cell(
        &mut self,
        table: md_document::BlockId,
        row: usize,
        column: usize,
    ) -> EditorResult<()> {
        let cell = CellPos::new(row, column);
        self.set_selection(SelectionSet::new(Selection::table_cells(table, cell, cell)))
    }

    fn append_table_row_and_select(&mut self, table: md_document::BlockId) -> EditorResult<()> {
        let mut transaction = Transaction::new(self.document_revision());
        transaction.push(Operation::AppendTableRow { table });
        self.commit(transaction)?;

        let rows = self
            .document
            .children(table)
            .ok_or_else(|| EditorError::InvalidIntent {
                message: "table has no rows".to_owned(),
            })?;
        let row_index = rows.len().saturating_sub(1);
        let cell = CellPos::new(row_index, 0);
        self.set_selection(SelectionSet::new(Selection::table_cells(table, cell, cell)))?;
        self.pending_marks = None;
        self.state_revision += 1;
        Ok(())
    }

    fn nearest_list_item(&self, block: md_document::BlockId) -> Option<md_document::BlockId> {
        let mut current = Some(block);
        while let Some(id) = current {
            if matches!(self.document.kind(id), Some(BlockKind::ListItem { .. })) {
                return Some(id);
            }
            current = self.document.parent(id);
        }
        None
    }

    fn primary_local_range(
        &self,
        from: DocPos,
        to: DocPos,
    ) -> EditorResult<(md_document::BlockId, TextRange)> {
        let index = self.document.index();
        let start = index.resolve_position(&self.document, from, Affinity::Downstream)?;
        let end = index.resolve_position(&self.document, to, Affinity::Downstream)?;
        if start.block_id != end.block_id {
            return Err(EditorError::InvalidIntent {
                message: "editing across blocks is not implemented".to_owned(),
            });
        }
        Ok((
            start.block_id,
            TextRange::new(TextOffset::new(start.offset), TextOffset::new(end.offset)),
        ))
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
        Affinity, Anchor, BlockKind, CellPos, DocPos, Document, InlineContent, Selection,
        SelectionKind, SelectionSet,
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

    fn editor_with_table() -> Editor {
        let mut document = Document::new();
        let table = document
            .insert_block(
                None,
                0,
                BlockKind::Table {
                    aligns: vec![md_document::Align::Left; 2],
                    rows: vec![],
                },
            )
            .unwrap();
        for values in [["a", "b"], ["c", "d"]] {
            let row = document
                .append_block(Some(table), BlockKind::TableRow { cells: vec![] })
                .unwrap();
            for value in values {
                document
                    .append_block(
                        Some(row),
                        BlockKind::TableCell {
                            content: InlineContent::from_text(value),
                        },
                    )
                    .unwrap();
            }
        }
        let first_row = document.children(table).unwrap()[0];
        let first_cell = document.children(first_row).unwrap()[0];
        let leaf_index = document.index().leaf_index(first_cell).unwrap();
        let pos = document.index().leaf_start(leaf_index).unwrap();
        let selection =
            SelectionSet::new(Selection::collapsed(DocPos::new(pos), Affinity::Downstream));
        Editor::with_selection(document, selection).unwrap()
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
            editor.dispatch(Intent::ToggleList { ordered: false }),
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

    #[test]
    fn insert_text_rule_edits_and_moves_cursor() {
        let mut editor = editor_at(0);
        let rule = editor.dispatch(Intent::InsertText("x".to_owned())).unwrap();
        assert_eq!(rule, RuleId::INSERT_TEXT);
        assert_eq!(editor.document_revision(), 1);
        let root = editor.document().root_children()[0];
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(1));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn delete_backward_rule_removes_previous_grapheme() {
        let mut editor = editor_at(2);
        let rule = editor.dispatch(Intent::DeleteBackward).unwrap();
        assert_eq!(rule, RuleId::DELETE_BACKWARD);
        assert_eq!(editor.document_revision(), 1);
        let block = editor.document().root_children()[1];
        assert_eq!(editor.document().kind(block).unwrap().leaf_len(), Some(3));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn delete_backward_at_start_is_noop() {
        let mut editor = editor_at(0);
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    #[test]
    fn delete_backward_removes_selected_range() {
        let mut editor = editor_at(2);
        editor
            .set_selection(SelectionSet::new(Selection::range(
                Anchor::new(DocPos::new(1), Affinity::Upstream),
                Anchor::new(DocPos::new(2), Affinity::Upstream),
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        let block = editor.document().root_children()[1];
        assert_eq!(editor.document().kind(block).unwrap().leaf_len(), Some(3));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn delete_backward_joins_across_sibling_blocks() {
        let mut editor = editor_at(2);
        editor
            .set_selection(SelectionSet::new(Selection::range(
                Anchor::new(DocPos::new(2), Affinity::Upstream),
                Anchor::new(DocPos::new(7), Affinity::Upstream),
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert_eq!(editor.document().root_children().len(), 2);
        let block = editor.document().root_children()[1];
        assert_eq!(editor.document().kind(block).unwrap().leaf_len(), Some(1));
        assert_eq!(head(&editor), DocPos::new(2));
    }

    #[test]
    fn enter_splits_paragraph_at_caret() {
        let mut editor = editor_at(2);
        let before = editor.document().root_children().len();
        let rule = editor.dispatch(Intent::InsertNewline).unwrap();
        assert_eq!(rule, RuleId::ENTER);
        assert_eq!(editor.document_revision(), 1);
        assert_eq!(editor.document().root_children().len(), before + 1);
        assert_eq!(head(&editor), DocPos::new(3));
    }

    #[test]
    fn enter_at_paragraph_end_creates_empty_paragraph() {
        let mut editor = editor_at(5);
        editor.dispatch(Intent::InsertNewline).unwrap();
        let children = editor.document().root_children();
        let inserted = children[2];
        assert_eq!(
            editor.document().kind(inserted).unwrap().leaf_len(),
            Some(0)
        );
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn enter_at_empty_paragraph_creates_another_empty_paragraph() {
        let mut editor = editor_at(0);
        editor.dispatch(Intent::InsertNewline).unwrap();
        assert_eq!(editor.document().root_children().len(), 4);
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn delete_forward_rule_removes_next_grapheme() {
        let mut editor = editor_at(1);
        let rule = editor.dispatch(Intent::DeleteForward).unwrap();
        assert_eq!(rule, RuleId::DELETE_FORWARD);
        assert_eq!(editor.document_revision(), 1);
        let block = editor.document().root_children()[1];
        assert_eq!(editor.document().kind(block).unwrap().leaf_len(), Some(3));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn delete_forward_at_document_end_is_noop() {
        let mut editor = editor_at(7);
        editor.dispatch(Intent::DeleteForward).unwrap();
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(head(&editor), DocPos::new(7));
    }

    #[test]
    fn delete_forward_removes_selected_range() {
        let mut editor = editor_at(2);
        editor
            .set_selection(SelectionSet::new(Selection::range(
                Anchor::new(DocPos::new(1), Affinity::Upstream),
                Anchor::new(DocPos::new(2), Affinity::Upstream),
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteForward).unwrap();
        let block = editor.document().root_children()[1];
        assert_eq!(editor.document().kind(block).unwrap().leaf_len(), Some(3));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn tab_in_paragraph_is_noop() {
        let mut editor = editor_at(1);
        let rule = editor
            .dispatch(Intent::ChangeIndent { outdent: false })
            .unwrap();
        assert_eq!(rule, RuleId::CHANGE_INDENT);
        assert_eq!(editor.document_revision(), 0);
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn tab_in_code_block_inserts_tab() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        document
            .set_block_kind(
                root,
                BlockKind::CodeBlock {
                    lang: None,
                    text: "ab".into(),
                    fence: md_document::FenceStyle::BACKTICK,
                },
            )
            .unwrap();
        let mut editor = Editor::new(document);
        editor
            .dispatch(Intent::ChangeIndent { outdent: false })
            .unwrap();
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(3));
        assert_eq!(head(&editor), DocPos::new(1));
    }

    #[test]
    fn shift_tab_in_code_block_removes_tab() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        document
            .set_block_kind(
                root,
                BlockKind::CodeBlock {
                    lang: None,
                    text: "\tab".into(),
                    fence: md_document::FenceStyle::BACKTICK,
                },
            )
            .unwrap();
        let mut editor = Editor::with_selection(
            document,
            SelectionSet::new(Selection::collapsed(DocPos::new(1), Affinity::Downstream)),
        )
        .unwrap();
        editor
            .dispatch(Intent::ChangeIndent { outdent: true })
            .unwrap();
        assert_eq!(editor.document().kind(root).unwrap().leaf_len(), Some(2));
        assert_eq!(head(&editor), DocPos::ZERO);
    }

    fn table_cell(editor: &Editor) -> CellPos {
        match editor.selection().primary_selection().unwrap().kind {
            SelectionKind::TableCells { start, .. } => start,
            _ => panic!("expected table-cell selection"),
        }
    }

    #[test]
    fn tab_moves_to_next_table_cell() {
        let mut editor = editor_with_table();
        editor
            .dispatch(Intent::ChangeIndent { outdent: false })
            .unwrap();
        assert_eq!(table_cell(&editor), CellPos::new(0, 1));
    }

    #[test]
    fn shift_tab_moves_to_previous_table_cell() {
        let mut editor = editor_with_table();
        let table = *editor.document().root_children().first().unwrap();
        editor
            .set_selection(SelectionSet::new(Selection::table_cells(
                table,
                CellPos::new(0, 1),
                CellPos::new(0, 1),
            )))
            .unwrap();
        editor
            .dispatch(Intent::ChangeIndent { outdent: true })
            .unwrap();
        assert_eq!(table_cell(&editor), CellPos::new(0, 0));
    }

    #[test]
    fn tab_at_last_table_cell_appends_row() {
        let mut editor = editor_with_table();
        let table = *editor.document().root_children().first().unwrap();
        editor
            .set_selection(SelectionSet::new(Selection::table_cells(
                table,
                CellPos::new(1, 1),
                CellPos::new(1, 1),
            )))
            .unwrap();
        editor
            .dispatch(Intent::ChangeIndent { outdent: false })
            .unwrap();
        assert_eq!(table_cell(&editor), CellPos::new(2, 0));
        assert_eq!(editor.document().children(table).unwrap().len(), 3);
        assert!(editor.document().validate().is_ok());
    }

    #[test]
    fn enter_in_table_cell_inserts_hard_break() {
        let mut editor = editor_with_table();
        editor.dispatch(Intent::InsertNewline).unwrap();
        let table = *editor.document().root_children().first().unwrap();
        let row = editor.document().children(table).unwrap()[0];
        let cell = editor.document().children(row).unwrap()[0];
        let content = editor
            .document()
            .kind(cell)
            .and_then(BlockKind::inline_content)
            .unwrap();
        assert!(
            content
                .runs()
                .iter()
                .any(|run| matches!(run.atom_kind(), Some(md_document::AtomKind::HardBreak)))
        );
        assert_eq!(editor.document().root_children().len(), 2);
    }

    #[test]
    fn enter_in_quote_splits_paragraph_inside_quote() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        document
            .append_block(
                Some(quote),
                BlockKind::Paragraph(InlineContent::from_text("ab")),
            )
            .unwrap();
        let selection =
            SelectionSet::new(Selection::collapsed(DocPos::new(1), Affinity::Downstream));
        let mut editor = Editor::with_selection(document, selection).unwrap();
        editor.dispatch(Intent::InsertNewline).unwrap();
        assert_eq!(editor.document().children(quote).unwrap().len(), 2);
        assert!(editor.document().validate().is_ok());
    }

    #[test]
    fn backspace_at_table_cell_start_moves_to_previous_cell() {
        let mut editor = editor_with_table();
        let table = *editor.document().root_children().first().unwrap();
        let row = editor.document().children(table).unwrap()[0];
        let cells = editor.document().children(row).unwrap().to_vec();
        let second = cells[1];
        let index = editor.document().index();
        let leaf_index = index.leaf_index(second).unwrap();
        let pos = index.leaf_start(leaf_index).unwrap();
        editor
            .set_selection(SelectionSet::new(Selection::collapsed(
                DocPos::new(pos),
                Affinity::Downstream,
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        let first = cells[0];
        let index = editor.document().index();
        let first_index = index.leaf_index(first).unwrap();
        let expected =
            index.leaf_start(first_index).unwrap() + index.leaf_len(first_index).unwrap();
        assert_eq!(head(&editor), DocPos::new(expected));
    }

    #[test]
    fn backspace_at_first_table_cell_is_noop() {
        let mut editor = editor_with_table();
        let revision = editor.document_revision();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert_eq!(editor.document_revision(), revision);
    }

    #[test]
    fn backspace_at_row_start_moves_to_previous_row_end() {
        let mut editor = editor_with_table();
        let table = *editor.document().root_children().first().unwrap();
        let rows = editor.document().children(table).unwrap().to_vec();
        let second_row = rows[1];
        let first_cell = editor.document().children(second_row).unwrap()[0];
        let index = editor.document().index();
        let leaf_index = index.leaf_index(first_cell).unwrap();
        let pos = index.leaf_start(leaf_index).unwrap();
        editor
            .set_selection(SelectionSet::new(Selection::collapsed(
                DocPos::new(pos),
                Affinity::Downstream,
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        let previous_row = rows[0];
        let previous_cell = *editor
            .document()
            .children(previous_row)
            .unwrap()
            .last()
            .unwrap();
        let index = editor.document().index();
        let leaf_index = index.leaf_index(previous_cell).unwrap();
        let expected = index.leaf_start(leaf_index).unwrap() + index.leaf_len(leaf_index).unwrap();
        assert_eq!(head(&editor), DocPos::new(expected));
    }

    fn quote_editor(children: &[&str]) -> (Editor, md_document::BlockId) {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        for text in children {
            document
                .append_block(
                    Some(quote),
                    BlockKind::Paragraph(InlineContent::from_text(*text)),
                )
                .unwrap();
        }
        let editor = Editor::new(document);
        (editor, quote)
    }

    #[test]
    fn backspace_at_first_quote_child_moves_it_out() {
        let (mut editor, quote) = quote_editor(&["ab", "cd"]);
        editor
            .set_selection(SelectionSet::new(Selection::collapsed(
                DocPos::ZERO,
                Affinity::Downstream,
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert!(editor.document().node(quote).is_some());
        assert_eq!(editor.document().children(quote).unwrap().len(), 1);
        assert!(editor.document().validate().is_ok());
    }

    #[test]
    fn backspace_at_only_quote_child_removes_quote() {
        let (mut editor, quote) = quote_editor(&["ab"]);
        editor
            .set_selection(SelectionSet::new(Selection::collapsed(
                DocPos::ZERO,
                Affinity::Downstream,
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert!(editor.document().node(quote).is_none());
        assert!(editor.document().validate().is_ok());
    }

    #[test]
    fn backspace_at_second_quote_child_does_not_exit_quote() {
        let (mut editor, quote) = quote_editor(&["ab", "cd"]);
        let second = editor.document().children(quote).unwrap()[1];
        let index = editor.document().index();
        let leaf_index = index.leaf_index(second).unwrap();
        let pos = index.leaf_start(leaf_index).unwrap();
        editor
            .set_selection(SelectionSet::new(Selection::collapsed(
                DocPos::new(pos),
                Affinity::Downstream,
            )))
            .unwrap();
        editor.dispatch(Intent::DeleteBackward).unwrap();
        assert!(editor.document().node(quote).is_some());
        assert_eq!(editor.document().children(quote).unwrap().len(), 2);
    }

    #[test]
    fn ime_commit_is_one_undoable_entry() {
        let mut editor = Editor::new(Document::new());
        editor
            .dispatch(Intent::ImeSetMarked("你".to_owned()))
            .unwrap();
        assert_eq!(editor.composition().unwrap().text, "你");
        editor.dispatch(Intent::ImeCommit("你".to_owned())).unwrap();
        assert!(editor.composition().is_none());
        assert_eq!(editor.document().index().doc_length(), "你".len());
        assert_eq!(editor.history().undo_len(), 1);
        editor.undo().unwrap();
        assert_eq!(editor.document().index().doc_length(), 0);
    }

    #[test]
    fn ime_commit_with_empty_text_uses_marked_text() {
        let mut editor = Editor::new(Document::new());
        editor
            .dispatch(Intent::ImeSetMarked("ab".to_owned()))
            .unwrap();
        editor.dispatch(Intent::ImeCommit(String::new())).unwrap();
        assert_eq!(editor.document().index().doc_length(), 2);
    }

    #[test]
    fn ime_cancel_clears_composition() {
        let mut editor = Editor::new(Document::new());
        editor
            .dispatch(Intent::ImeSetMarked("x".to_owned()))
            .unwrap();
        editor.dispatch(Intent::ImeCancel).unwrap();
        assert!(editor.composition().is_none());
        assert_eq!(editor.document_revision(), 0);
    }

    #[test]
    fn copy_text_within_one_block() {
        let mut document = Document::new();
        let block = document.root_children()[0];
        document
            .set_inline_content(block, InlineContent::from_text("abc"))
            .unwrap();
        let selection = SelectionSet::new(Selection::range(
            Anchor::new(DocPos::new(1), Affinity::Upstream),
            Anchor::new(DocPos::new(3), Affinity::Upstream),
        ));
        let editor = Editor::with_selection(document, selection).unwrap();
        assert_eq!(editor.copy_text(), "bc");
    }

    #[test]
    fn copy_text_across_blocks_joins_with_newline() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("ab"))
            .unwrap();
        document
            .append_block(None, BlockKind::Paragraph(InlineContent::from_text("cd")))
            .unwrap();
        let selection = SelectionSet::new(Selection::range(
            Anchor::new(DocPos::new(1), Affinity::Upstream),
            Anchor::new(DocPos::new(4), Affinity::Upstream),
        ));
        let editor = Editor::with_selection(document, selection).unwrap();
        assert_eq!(editor.copy_text(), "b\nc");
    }

    #[test]
    fn paste_splits_blank_lines_and_undoes_once() {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::Paste("a\n\nb".to_owned())).unwrap();
        assert_eq!(editor.document().root_children().len(), 2);
        assert_eq!(editor.history().undo_len(), 1);
        editor.undo().unwrap();
        assert_eq!(editor.document().index().doc_length(), 0);
    }

    #[test]
    fn hash_space_input_rule_creates_heading() {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText("#".to_owned())).unwrap();
        editor.dispatch(Intent::InsertText(" ".to_owned())).unwrap();
        let block = editor.document().root_children()[0];
        assert!(matches!(
            editor.document().kind(block),
            Some(BlockKind::Heading { level: 1, .. })
        ));
        editor.undo().unwrap();
        assert_eq!(editor.document().index().doc_length(), 0);
    }
}
