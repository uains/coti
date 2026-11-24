//! Undo/redo history built from Transaction change sets.

use std::collections::VecDeque;
use std::mem::size_of_val;
use std::time::{Duration, Instant};

use md_document::{BlockId, Document, SelectionSet};

use crate::{ChangeSet, Editor, EditorResult, Operation};

/// History capacity configuration.
#[derive(Clone, Copy, Debug)]
pub struct HistoryConfig {
    /// Maximum retained undo entries.
    pub max_entries: usize,
    /// Maximum estimated retained bytes.
    pub max_bytes: usize,
    /// Coalescing window for consecutive text inserts.
    pub coalesce_window: Duration,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_bytes: 32 * 1024 * 1024,
            coalesce_window: Duration::from_millis(500),
        }
    }
}

/// Coalescing group metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryGroup {
    /// Group kind.
    pub kind: GroupKind,
    /// Anchor block.
    pub block_id: BlockId,
}

/// Supported coalescing kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Consecutive literal text insertion.
    TextInsert,
}

/// One undoable history entry.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    /// Forward operations in apply order.
    pub forward: Vec<Operation>,
    /// Inverse operations in forward order; undo reverses them.
    pub inverse: Vec<Operation>,
    /// Document snapshot before the change.
    pub before_document: Document,
    /// Document snapshot after the change.
    pub after_document: Document,
    /// Selection before the change.
    pub before_selection: SelectionSet,
    /// Selection after the change.
    pub after_selection: SelectionSet,
    /// Group metadata.
    pub group: Option<HistoryGroup>,
    /// Document revision after the change.
    pub revision: u64,
    /// Creation timestamp.
    pub timestamp: Instant,
    /// Estimated heap cost.
    pub estimated_bytes: usize,
    /// Logical state id before the change.
    pub before_state: u64,
    /// Logical state id after the change.
    pub after_state: u64,
}

impl HistoryEntry {
    /// Builds an entry from a committed change set.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_change(
        change: ChangeSet,
        before_selection: SelectionSet,
        after_selection: SelectionSet,
        before_document: Document,
        after_document: Document,
        group: Option<HistoryGroup>,
        before_state: u64,
        after_state: u64,
    ) -> Self {
        let estimated_bytes = estimate_change(&change)
            .saturating_add(before_document.block_count() * 256)
            .saturating_add(after_document.block_count() * 256);
        Self {
            forward: change.forward,
            inverse: change.inverses,
            before_document,
            after_document,
            before_selection,
            after_selection,
            group,
            revision: change.new_revision,
            timestamp: Instant::now(),
            estimated_bytes,
            before_state,
            after_state,
        }
    }
}

/// Bounded undo/redo stack.
#[derive(Clone, Debug)]
pub struct History {
    config: HistoryConfig,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    total_bytes: usize,
    current_state: u64,
    save_point: Option<u64>,
}

impl History {
    /// Creates an empty history.
    #[must_use]
    pub fn new(config: HistoryConfig) -> Self {
        Self {
            config,
            undo: VecDeque::new(),
            redo: Vec::new(),
            total_bytes: 0,
            current_state: 0,
            save_point: None,
        }
    }

    /// Returns the current logical state id.
    #[must_use]
    pub const fn current_state(&self) -> u64 {
        self.current_state
    }

    /// Returns the number of undo entries.
    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Returns the number of redo entries.
    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Returns whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Marks the current logical state as saved.
    pub fn mark_saved(&mut self) {
        self.save_point = Some(self.current_state);
    }

    /// Returns whether the current state differs from the save point.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.save_point != Some(self.current_state)
    }

    /// Pushes a committed entry, coalescing or evicting as needed.
    pub fn push(&mut self, mut entry: HistoryEntry) {
        self.redo.clear();
        entry.before_state = self.current_state;
        self.current_state += 1;
        entry.after_state = self.current_state;

        if let Some(previous) = self.undo.back_mut()
            && previous.group.is_some()
            && previous.group == entry.group
            && previous.after_selection == entry.before_selection
            && self.save_point != Some(previous.after_state)
            && entry.timestamp.duration_since(previous.timestamp) < self.config.coalesce_window
        {
            let mut inverses = previous.inverse.clone();
            inverses.extend(entry.inverse);
            previous.forward.extend(entry.forward);
            previous.inverse = inverses;
            previous.after_selection = entry.after_selection;
            previous.after_document = entry.after_document;
            previous.revision = entry.revision;
            previous.timestamp = entry.timestamp;
            previous.after_state = entry.after_state;
            previous.estimated_bytes = previous
                .estimated_bytes
                .saturating_add(entry.estimated_bytes);
            self.total_bytes = self.total_bytes.saturating_add(entry.estimated_bytes);
            self.evict();
            return;
        }

        self.total_bytes = self.total_bytes.saturating_add(entry.estimated_bytes);
        self.undo.push_back(entry);
        self.evict();
    }

    /// Undoes the latest entry.
    ///
    /// # Errors
    ///
    /// Returns an error if applying the inverse fails.
    pub fn undo(&mut self, editor: &mut Editor) -> EditorResult<bool> {
        let Some(entry) = self.undo.pop_back() else {
            return Ok(false);
        };
        editor.restore_full_document(entry.before_document.clone());
        editor.set_selection(entry.before_selection.clone())?;
        self.total_bytes = self.total_bytes.saturating_sub(entry.estimated_bytes);
        self.current_state = entry.before_state;
        self.redo.push(entry);
        Ok(true)
    }

    /// Redoes the latest undone entry.
    ///
    /// # Errors
    ///
    /// Returns an error if applying the forward operations fails.
    pub fn redo(&mut self, editor: &mut Editor) -> EditorResult<bool> {
        let Some(entry) = self.redo.pop() else {
            return Ok(false);
        };
        editor.restore_full_document(entry.after_document.clone());
        editor.set_selection(entry.after_selection.clone())?;
        self.total_bytes = self.total_bytes.saturating_add(entry.estimated_bytes);
        self.current_state = entry.after_state;
        self.undo.push_back(entry);
        Ok(true)
    }

    /// Clears all history and resets the save point.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.total_bytes = 0;
        self.current_state = 0;
        self.save_point = None;
    }

    fn evict(&mut self) {
        while self.undo.len() > self.config.max_entries {
            if let Some(removed) = self.undo.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(removed.estimated_bytes);
            }
        }
        while self.total_bytes > self.config.max_bytes && self.undo.len() > 1 {
            if let Some(removed) = self.undo.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(removed.estimated_bytes);
            }
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(HistoryConfig::default())
    }
}

fn estimate_change(change: &ChangeSet) -> usize {
    change
        .forward
        .iter()
        .map(estimate_operation)
        .sum::<usize>()
        .saturating_add(128)
}

fn estimate_operation(operation: &Operation) -> usize {
    size_of_val(operation).saturating_add(64)
}

#[cfg(test)]
mod tests {
    use super::{History, HistoryConfig};
    use crate::{Editor, Intent};
    use md_document::Document;

    fn editor_with_history(history: History) -> Editor {
        Editor::with_history(Document::new(), history)
    }

    #[test]
    fn save_point_tracks_undo_and_redo() {
        let mut editor = editor_with_history(History::default());
        editor.mark_saved();
        assert!(!editor.is_modified());
        editor.dispatch(Intent::InsertText("a".to_owned())).unwrap();
        assert!(editor.is_modified());
        editor.mark_saved();
        assert!(!editor.is_modified());
        editor.dispatch(Intent::InsertText("b".to_owned())).unwrap();
        assert!(editor.is_modified());
        editor.undo().unwrap();
        assert!(!editor.is_modified());
        editor.redo().unwrap();
        assert!(editor.is_modified());
    }

    #[test]
    fn undo_redo_ten_steps() {
        let history = History::new(HistoryConfig {
            max_entries: 100,
            max_bytes: usize::MAX,
            coalesce_window: std::time::Duration::ZERO,
        });
        let mut editor = editor_with_history(history);
        for _ in 0..10 {
            editor.dispatch(Intent::InsertText("x".to_owned())).unwrap();
        }
        for _ in 0..10 {
            assert!(editor.undo().unwrap());
        }
        assert_eq!(editor.document().index().doc_length(), 0);
        for _ in 0..10 {
            assert!(editor.redo().unwrap());
        }
        assert_eq!(editor.document().index().doc_length(), 10);
    }

    #[test]
    fn consecutive_inserts_coalesce() {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText("a".to_owned())).unwrap();
        editor.dispatch(Intent::InsertText("b".to_owned())).unwrap();
        assert_eq!(editor.history().undo_len(), 1);
        editor.undo().unwrap();
        assert_eq!(editor.document().index().doc_length(), 0);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText("a".to_owned())).unwrap();
        editor.dispatch(Intent::InsertText("b".to_owned())).unwrap();
        editor.undo().unwrap();
        assert!(editor.can_redo());
        editor.dispatch(Intent::InsertText("c".to_owned())).unwrap();
        assert!(!editor.can_redo());
    }

    #[test]
    fn max_entries_evicts_oldest() {
        let history = History::new(HistoryConfig {
            max_entries: 1,
            max_bytes: usize::MAX,
            coalesce_window: std::time::Duration::ZERO,
        });
        let mut editor = editor_with_history(history);
        for text in ["a", "b", "c"] {
            editor
                .dispatch(Intent::InsertText(text.to_owned()))
                .unwrap();
        }
        assert_eq!(editor.history().undo_len(), 1);
        assert!(editor.undo().unwrap());
        assert!(!editor.undo().unwrap());
    }

    #[test]
    fn max_bytes_evicts_oldest() {
        let history = History::new(HistoryConfig {
            max_entries: 100,
            max_bytes: 1,
            coalesce_window: std::time::Duration::ZERO,
        });
        let mut editor = editor_with_history(history);
        for text in ["a", "b", "c"] {
            editor
                .dispatch(Intent::InsertText(text.to_owned()))
                .unwrap();
        }
        assert_eq!(editor.history().undo_len(), 1);
    }

    #[test]
    fn history_entry_contains_forward_and_inverse() {
        let mut editor = Editor::new(Document::new());
        editor.dispatch(Intent::InsertText("a".to_owned())).unwrap();
        let entry = &editor.history().undo[0];
        assert!(!entry.forward.is_empty());
        assert!(!entry.inverse.is_empty());
        assert_ne!(entry.before_selection, entry.after_selection);
    }

    #[test]
    fn undo_redo_dispatch_restores_selection() {
        let mut editor = Editor::new(Document::new());
        editor
            .dispatch(Intent::InsertText("ab".to_owned()))
            .unwrap();
        assert_eq!(editor.document().index().doc_length(), 2);
        let rule = editor.dispatch(Intent::Undo).unwrap();
        assert_eq!(rule, crate::RuleId::UNDO);
        assert_eq!(editor.document().index().doc_length(), 0);
        assert_eq!(
            editor.selection().primary_selection().unwrap().head.pos,
            md_document::DocPos::ZERO
        );
        let rule = editor.dispatch(Intent::Redo).unwrap();
        assert_eq!(rule, crate::RuleId::REDO);
        assert_eq!(editor.document().index().doc_length(), 2);
        assert_eq!(
            editor.selection().primary_selection().unwrap().head.pos,
            md_document::DocPos::new(2)
        );
    }
}
