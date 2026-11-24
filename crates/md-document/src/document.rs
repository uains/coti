//! Document root, controlled structural edits, snapshots, and invariants.

use std::collections::HashSet;

use md_core::TextBuf;

use crate::{
    BlockId, BlockKind, BlockNode, DocumentError, DocumentIndex, DocumentResult, InlineContent,
    NodeArena, SourceHint,
};

/// A document tree backed by a generation-checked arena.
///
/// Structural methods allow transiently invalid containers while assembling a
/// tree; [`Self::validate`] is the gate for every invariant. Transactional code
/// must validate before publishing a revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    root: Vec<BlockId>,
    arena: NodeArena,
    index: DocumentIndex,
}

impl Document {
    /// Creates a document with one empty paragraph.
    #[must_use]
    pub fn new() -> Self {
        let mut arena = NodeArena::new();
        let paragraph = arena.alloc(BlockKind::Paragraph(InlineContent::new()), None);
        let mut document = Self {
            root: vec![paragraph],
            arena,
            index: DocumentIndex::default(),
        };
        document.rebuild_index();
        document
    }

    /// Creates a structurally empty document for an importer.
    ///
    /// The document is invalid until at least one root block is inserted; this
    /// constructor exists so the Markdown parser does not have to remove a
    /// synthetic paragraph after parsing.
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            root: Vec::new(),
            arena: NodeArena::new(),
            index: DocumentIndex::default(),
        }
    }

    /// Returns the derived document index.
    #[must_use]
    pub fn index(&self) -> &DocumentIndex {
        &self.index
    }

    /// Rebuilds the derived index from the current tree.
    pub fn rebuild_index(&mut self) {
        self.index = DocumentIndex::rebuild(self);
    }

    /// Returns root-level block ids.
    #[must_use]
    pub fn root_children(&self) -> &[BlockId] {
        &self.root
    }

    /// Returns a live node by id.
    #[must_use]
    pub fn node(&self, id: BlockId) -> Option<&BlockNode> {
        self.arena.get(id)
    }

    /// Returns the parent of a live node.
    #[must_use]
    pub fn parent(&self, id: BlockId) -> Option<BlockId> {
        self.arena.get(id).and_then(BlockNode::parent)
    }

    /// Returns child ids for a container node.
    #[must_use]
    pub fn children(&self, id: BlockId) -> Option<&[BlockId]> {
        self.arena.get(id).and_then(|node| node.kind().children())
    }

    /// Returns a node's kind by id.
    #[must_use]
    pub fn kind(&self, id: BlockId) -> Option<&BlockKind> {
        self.arena.get(id).map(BlockNode::kind)
    }

    /// Returns source-fidelity metadata for a block.
    #[must_use]
    pub fn source(&self, id: BlockId) -> Option<&SourceHint> {
        self.arena.get(id).map(BlockNode::source)
    }

    /// Sets source-fidelity metadata for a block.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id.
    pub fn set_source(&mut self, id: BlockId, source: SourceHint) -> DocumentResult<()> {
        let node = self.arena.get_mut(id).ok_or_else(|| stale(id))?;
        *node.source_mut() = source;
        Ok(())
    }

    /// Replaces the inline content of a Paragraph, Heading, or TableCell.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id or a block without inline content.
    pub fn set_inline_content(
        &mut self,
        id: BlockId,
        content: InlineContent,
    ) -> DocumentResult<()> {
        let node = self.arena.get_mut(id).ok_or_else(|| stale(id))?;
        let target =
            node.kind_mut()
                .inline_content_mut()
                .ok_or_else(|| DocumentError::NotALeaf {
                    index: id.index(),
                    generation: id.generation(),
                })?;
        *target = content;
        self.rebuild_index();
        Ok(())
    }

    /// Returns mutable raw text for a CodeBlock or raw block.
    pub fn raw_text_mut(&mut self, id: BlockId) -> Option<&mut TextBuf> {
        self.arena.get_mut(id)?.kind_mut().raw_text_mut()
    }

    /// Replaces raw text for a CodeBlock or raw block.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id or a non-raw block.
    pub fn set_raw_text(&mut self, id: BlockId, text: TextBuf) -> DocumentResult<()> {
        let node = self.arena.get_mut(id).ok_or_else(|| stale(id))?;
        let target = node
            .kind_mut()
            .raw_text_mut()
            .ok_or(DocumentError::NotALeaf {
                index: id.index(),
                generation: id.generation(),
            })?;
        *target = text;
        self.rebuild_index();
        Ok(())
    }

    /// Replaces a leaf block's kind.
    ///
    /// Container kinds are intentionally rejected here; structural replacement
    /// must go through explicit Operation/Transaction rules.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id, parent/child mismatch, container kind,
    /// or invalid kind.
    pub fn set_block_kind(&mut self, id: BlockId, kind: BlockKind) -> DocumentResult<()> {
        let node = self.arena.get(id).ok_or_else(|| stale(id))?;
        if node.kind().is_container() || kind.is_container() {
            return Err(DocumentError::Invariant {
                message: "set_block_kind only supports leaf blocks".to_owned(),
            });
        }
        kind.validate()?;

        let parent = node.parent();
        if let Some(parent_id) = parent {
            let parent_node = self.arena.get(parent_id).ok_or_else(|| stale(parent_id))?;
            if !kind.can_be_child_of(parent_node.kind()) {
                return Err(DocumentError::Invariant {
                    message: format!(
                        "{} cannot be a child of {}",
                        kind.kind_name(),
                        parent_node.kind().kind_name()
                    ),
                });
            }
        } else if !kind.is_block_level() {
            return Err(DocumentError::Invariant {
                message: format!("{} cannot be a root block", kind.kind_name()),
            });
        }

        let node = self.arena.get_mut(id).ok_or_else(|| stale(id))?;
        *node.kind_mut() = kind;
        self.rebuild_index();
        Ok(())
    }

    /// Sets a list item's task checkbox state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id or a non-list-item block.
    pub fn set_list_item_task(&mut self, id: BlockId, checked: Option<bool>) -> DocumentResult<()> {
        let node = self.arena.get_mut(id).ok_or_else(|| stale(id))?;
        let BlockKind::ListItem { task, .. } = node.kind_mut() else {
            return Err(DocumentError::Invariant {
                message: format!("block {id} is not a list item"),
            });
        };
        *task = checked;
        Ok(())
    }

    /// Returns the number of live blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.arena.len()
    }

    /// Returns the number of live leaf blocks.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.arena
            .iter()
            .filter(|node| node.kind().is_leaf())
            .count()
    }

    /// Inserts a block under `parent` (`None` means root) at `index`.
    ///
    /// The kind is checked for parent/child compatibility but not fully
    /// validated, so empty containers can be assembled and filled afterwards.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale parent, non-container parent, invalid child
    /// type, or out-of-bounds index.
    pub fn insert_block(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        kind: BlockKind,
    ) -> DocumentResult<BlockId> {
        self.insert_block_with_source(parent, index, kind, SourceHint::default())
    }

    /// Inserts a block with explicit source metadata.
    ///
    /// # Errors
    ///
    /// See [`Self::insert_block`].
    pub fn insert_block_with_source(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        kind: BlockKind,
        source: SourceHint,
    ) -> DocumentResult<BlockId> {
        let index_limit = if let Some(parent_id) = parent {
            let parent_node = self.arena.get(parent_id).ok_or_else(|| stale(parent_id))?;
            if !parent_node.kind().is_container() {
                return Err(DocumentError::NotAContainer {
                    index: parent_id.index(),
                    generation: parent_id.generation(),
                });
            }
            if !kind.can_be_child_of(parent_node.kind()) {
                return Err(DocumentError::Invariant {
                    message: format!(
                        "{} cannot be a child of {}",
                        kind.kind_name(),
                        parent_node.kind().kind_name()
                    ),
                });
            }
            parent_node
                .kind()
                .children()
                .map_or(0, |children| children.len())
        } else {
            if !kind.is_block_level() {
                return Err(DocumentError::Invariant {
                    message: format!("{} cannot be a root block", kind.kind_name()),
                });
            }
            self.root.len()
        };

        if index > index_limit {
            return Err(DocumentError::Invariant {
                message: format!("child index {index} is out of bounds for {index_limit}"),
            });
        }

        let id = self.arena.alloc_with_source(kind, parent, source);
        self.insert_child_at(parent, index, id)?;
        self.rebuild_index();
        Ok(id)
    }

    /// Appends a block under `parent`.
    ///
    /// # Errors
    ///
    /// See [`Self::insert_block`].
    pub fn append_block(
        &mut self,
        parent: Option<BlockId>,
        kind: BlockKind,
    ) -> DocumentResult<BlockId> {
        self.append_block_with_source(parent, kind, SourceHint::default())
    }

    /// Appends a block with explicit source metadata.
    ///
    /// # Errors
    ///
    /// See [`Self::insert_block`].
    pub fn append_block_with_source(
        &mut self,
        parent: Option<BlockId>,
        kind: BlockKind,
        source: SourceHint,
    ) -> DocumentResult<BlockId> {
        let index = if let Some(parent_id) = parent {
            self.children(parent_id)
                .ok_or_else(|| stale(parent_id))?
                .len()
        } else {
            self.root.len()
        };
        self.insert_block_with_source(parent, index, kind, source)
    }

    /// Removes a block and its subtree.
    ///
    /// If the root becomes empty, a fresh empty paragraph is inserted. This is
    /// not transaction-staged; History must snapshot or stage nodes separately.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale id, broken parent link, or cyclic subtree.
    pub fn remove_block(&mut self, id: BlockId) -> DocumentResult<BlockNode> {
        let node = self.arena.get(id).ok_or_else(|| stale(id))?;
        let parent = node.parent();

        let mut subtree = Vec::new();
        let mut visited = HashSet::new();
        self.collect_subtree(id, &mut visited, &mut subtree)?;

        let position =
            self.position_of_child(parent, id)
                .ok_or_else(|| DocumentError::Invariant {
                    message: format!("block {id} is not linked from its parent"),
                })?;
        self.take_child_at(parent, position)?;

        let removed = self.arena.free(id).ok_or_else(|| stale(id))?;
        for descendant in subtree.into_iter().skip(1).rev() {
            let _ = self.arena.free(descendant);
        }

        self.ensure_root_paragraph();
        self.rebuild_index();
        Ok(removed)
    }

    /// Moves a block to a new parent/index, preserving its `BlockId`.
    ///
    /// # Errors
    ///
    /// Returns an error for stale ids, cycles, invalid child types, an
    /// out-of-bounds index, or an operation that would empty a container.
    pub fn move_block(
        &mut self,
        id: BlockId,
        new_parent: Option<BlockId>,
        index: usize,
    ) -> DocumentResult<()> {
        let node = self.arena.get(id).ok_or_else(|| stale(id))?;
        let old_parent = node.parent();
        let child_kind = node.kind();

        if let Some(parent_id) = new_parent {
            if parent_id == id || self.is_ancestor(id, parent_id) {
                return Err(DocumentError::Invariant {
                    message: format!("cannot move block {id} into its own subtree"),
                });
            }
            let parent_node = self.arena.get(parent_id).ok_or_else(|| stale(parent_id))?;
            if !parent_node.kind().is_container() {
                return Err(DocumentError::NotAContainer {
                    index: parent_id.index(),
                    generation: parent_id.generation(),
                });
            }
            if !child_kind.can_be_child_of(parent_node.kind()) {
                return Err(DocumentError::Invariant {
                    message: format!(
                        "{} cannot be a child of {}",
                        child_kind.kind_name(),
                        parent_node.kind().kind_name()
                    ),
                });
            }
        } else if !child_kind.is_block_level() {
            return Err(DocumentError::Invariant {
                message: format!("{} cannot be a root block", child_kind.kind_name()),
            });
        }

        let old_index =
            self.position_of_child(old_parent, id)
                .ok_or_else(|| DocumentError::Invariant {
                    message: format!("block {id} is not linked from its parent"),
                })?;
        let old_len = self.child_count(old_parent)?;
        let new_len = self.child_count(new_parent)?;

        if old_parent != new_parent && old_len == 1 {
            return Err(DocumentError::Invariant {
                message: format!("moving block {id} would empty its current parent"),
            });
        }

        let target = if old_parent == new_parent {
            if index > new_len {
                return Err(DocumentError::Invariant {
                    message: format!("child index {index} is out of bounds for {new_len}"),
                });
            }
            if old_index < index { index - 1 } else { index }
        } else {
            if index > new_len {
                return Err(DocumentError::Invariant {
                    message: format!("child index {index} is out of bounds for {new_len}"),
                });
            }
            index
        };

        self.take_child_at(old_parent, old_index)?;
        self.insert_child_at(new_parent, target, id)?;
        if let Some(node) = self.arena.get_mut(id) {
            node.set_parent(new_parent);
        }
        self.rebuild_index();

        Ok(())
    }

    /// Captures a full structural snapshot.
    #[must_use]
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            root: self.root.clone(),
            arena: self.arena.clone(),
            index: self.index.clone(),
        }
    }

    /// Restores a previously captured snapshot.
    pub fn restore(&mut self, snapshot: DocumentSnapshot) {
        self.root = snapshot.root;
        self.arena = snapshot.arena;
        self.index = snapshot.index;
    }

    /// Returns `true` when `ancestor` contains `node`.
    #[must_use]
    pub fn is_ancestor(&self, ancestor: BlockId, node: BlockId) -> bool {
        let mut current = self.parent(node);
        let mut guard = 0;
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.parent(parent);
            guard += 1;
            if guard > self.arena.slot_count() {
                break;
            }
        }
        false
    }

    /// Validates the whole document tree and the arena.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for empty roots, stale ids, orphan nodes,
    /// cycles, wrong parent links, illegal child types, invalid local kinds,
    /// table column mismatches, or staged nodes.
    pub fn validate(&self) -> DocumentResult<()> {
        self.arena.validate()?;

        if self.arena.staged_len() != 0 {
            return Err(DocumentError::Invariant {
                message: format!("document has {} staged nodes", self.arena.staged_len()),
            });
        }

        if self.root.is_empty() {
            return Err(DocumentError::Invariant {
                message: "document root must contain at least one block".to_owned(),
            });
        }

        let mut visited = HashSet::new();
        for id in &self.root {
            let node = self.arena.get(*id).ok_or_else(|| stale(*id))?;
            if node.parent().is_some() {
                return Err(DocumentError::Invariant {
                    message: format!("root block {id} has a parent"),
                });
            }
            if !node.kind().is_block_level() {
                return Err(DocumentError::Invariant {
                    message: format!("{} cannot be a root block", node.kind().kind_name()),
                });
            }
            self.validate_node(*id, &mut visited)?;
        }

        if visited.len() != self.arena.len() {
            return Err(DocumentError::Invariant {
                message: format!(
                    "document has {} live blocks but only {} are reachable",
                    self.arena.len(),
                    visited.len()
                ),
            });
        }

        self.index.validate(self)?;
        Ok(())
    }

    fn validate_node(&self, id: BlockId, visited: &mut HashSet<BlockId>) -> DocumentResult<()> {
        if !visited.insert(id) {
            return Err(DocumentError::Invariant {
                message: format!("block {id} is reachable more than once"),
            });
        }

        let node = self.arena.get(id).ok_or_else(|| stale(id))?;
        node.kind().validate()?;
        node.source()
            .validate()
            .map_err(|message| DocumentError::Invariant {
                message: format!("block {id} source hint: {message}"),
            })?;

        if let BlockKind::Table { aligns, rows } = node.kind() {
            for row_id in rows {
                let row = self.arena.get(*row_id).ok_or_else(|| stale(*row_id))?;
                let BlockKind::TableRow { cells } = row.kind() else {
                    return Err(DocumentError::Invariant {
                        message: format!("table {id} contains non-row block {row_id}"),
                    });
                };
                if cells.len() != aligns.len() {
                    return Err(DocumentError::Invariant {
                        message: format!(
                            "table row {row_id} has {} cells but table has {} columns",
                            cells.len(),
                            aligns.len()
                        ),
                    });
                }
            }
        }

        if let Some(children) = node.kind().children() {
            for child in children {
                let child_node = self.arena.get(*child).ok_or_else(|| stale(*child))?;
                if child_node.parent() != Some(id) {
                    return Err(DocumentError::Invariant {
                        message: format!("block {child} has an inconsistent parent link"),
                    });
                }
                if !child_node.kind().can_be_child_of(node.kind()) {
                    return Err(DocumentError::Invariant {
                        message: format!(
                            "{} cannot be a child of {}",
                            child_node.kind().kind_name(),
                            node.kind().kind_name()
                        ),
                    });
                }
                self.validate_node(*child, visited)?;
            }
        }

        Ok(())
    }

    fn collect_subtree(
        &self,
        id: BlockId,
        visited: &mut HashSet<BlockId>,
        out: &mut Vec<BlockId>,
    ) -> DocumentResult<()> {
        if !visited.insert(id) {
            return Err(DocumentError::Invariant {
                message: format!("block {id} appears more than once in its subtree"),
            });
        }

        let node = self.arena.get(id).ok_or_else(|| stale(id))?;
        out.push(id);
        if let Some(children) = node.kind().children() {
            for child in children {
                self.collect_subtree(*child, visited, out)?;
            }
        }
        Ok(())
    }

    fn child_count(&self, parent: Option<BlockId>) -> DocumentResult<usize> {
        match parent {
            Some(id) => Ok(self.children(id).ok_or_else(|| stale(id))?.len()),
            None => Ok(self.root.len()),
        }
    }

    fn position_of_child(&self, parent: Option<BlockId>, id: BlockId) -> Option<usize> {
        match parent {
            Some(parent_id) => self
                .children(parent_id)?
                .iter()
                .position(|child| *child == id),
            None => self.root.iter().position(|child| *child == id),
        }
    }

    fn insert_child_at(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        id: BlockId,
    ) -> DocumentResult<()> {
        match parent {
            Some(parent_id) => {
                let node = self
                    .arena
                    .get_mut(parent_id)
                    .ok_or_else(|| stale(parent_id))?;
                let children =
                    node.kind_mut()
                        .children_mut()
                        .ok_or(DocumentError::NotAContainer {
                            index: parent_id.index(),
                            generation: parent_id.generation(),
                        })?;
                if index > children.len() {
                    return Err(DocumentError::Invariant {
                        message: format!("child index {index} is out of bounds"),
                    });
                }
                children.insert(index, id);
            }
            None => {
                if index > self.root.len() {
                    return Err(DocumentError::Invariant {
                        message: format!("root index {index} is out of bounds"),
                    });
                }
                self.root.insert(index, id);
            }
        }
        Ok(())
    }

    fn take_child_at(&mut self, parent: Option<BlockId>, index: usize) -> DocumentResult<BlockId> {
        match parent {
            Some(parent_id) => {
                let node = self
                    .arena
                    .get_mut(parent_id)
                    .ok_or_else(|| stale(parent_id))?;
                let children =
                    node.kind_mut()
                        .children_mut()
                        .ok_or(DocumentError::NotAContainer {
                            index: parent_id.index(),
                            generation: parent_id.generation(),
                        })?;
                if index >= children.len() {
                    return Err(DocumentError::Invariant {
                        message: format!("child index {index} is out of bounds"),
                    });
                }
                Ok(children.remove(index))
            }
            None => {
                if index >= self.root.len() {
                    return Err(DocumentError::Invariant {
                        message: format!("root index {index} is out of bounds"),
                    });
                }
                Ok(self.root.remove(index))
            }
        }
    }

    fn ensure_root_paragraph(&mut self) {
        if self.root.is_empty() {
            let id = self
                .arena
                .alloc(BlockKind::Paragraph(InlineContent::new()), None);
            self.root.push(id);
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// An opaque full structural snapshot of a [`Document`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSnapshot {
    root: Vec<BlockId>,
    arena: NodeArena,
    index: DocumentIndex,
}

fn stale(id: BlockId) -> DocumentError {
    DocumentError::StaleBlockId {
        index: id.index(),
        generation: id.generation(),
    }
}

#[cfg(test)]
mod tests {
    use super::Document;
    use crate::{Align, BlockId, BlockKind, InlineContent};

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn new_document_has_one_empty_paragraph() {
        let document = Document::new();
        assert_eq!(document.root_children().len(), 1);
        assert_eq!(document.block_count(), 1);
        assert_eq!(document.leaf_count(), 1);
        assert!(
            document
                .kind(document.root_children()[0])
                .unwrap()
                .is_leaf()
        );
        assert!(document.validate().is_ok());
    }

    #[test]
    fn inserting_root_blocks_preserves_order() {
        let mut document = Document::new();
        let empty = document.root_children()[0];
        let first = document.insert_block(None, 0, paragraph("first")).unwrap();
        let last = document.append_block(None, paragraph("last")).unwrap();

        assert_eq!(document.root_children(), &[first, empty, last]);
        assert_eq!(document.children(first), None);
        assert!(document.validate().is_ok());
    }

    #[test]
    fn inserting_into_non_container_is_rejected() {
        let mut document = Document::new();
        let paragraph_id = document.root_children()[0];
        assert!(
            document
                .insert_block(Some(paragraph_id), 0, paragraph("child"))
                .is_err()
        );
    }

    #[test]
    fn child_type_constraints_are_enforced() {
        let mut document = Document::new();
        let list = document
            .insert_block(
                None,
                0,
                BlockKind::List {
                    ordered: false,
                    start: 1,
                    tight: true,
                    items: vec![],
                },
            )
            .unwrap();

        assert!(
            document
                .insert_block(Some(list), 0, paragraph("not an item"))
                .is_err()
        );
        assert!(
            document
                .insert_block(
                    Some(list),
                    0,
                    BlockKind::ListItem {
                        task: None,
                        children: vec![],
                    },
                )
                .is_ok()
        );
    }

    #[test]
    fn remove_subtree_frees_all_nodes() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        assert!(document.validate().is_err(), "empty quote is invalid");
        let child = document
            .append_block(Some(quote), paragraph("child"))
            .unwrap();
        assert_eq!(document.block_count(), 3);
        assert!(document.validate().is_ok());

        assert!(document.remove_block(quote).is_ok());
        assert!(document.node(quote).is_none());
        assert!(document.node(child).is_none());
        assert_eq!(document.root_children().len(), 1);
        assert!(document.validate().is_ok());
    }

    #[test]
    fn removing_last_root_block_recreates_empty_paragraph() {
        let mut document = Document::new();
        let id = document.root_children()[0];
        document.remove_block(id).unwrap();
        assert_eq!(document.root_children().len(), 1);
        assert_ne!(document.root_children()[0], id);
        assert!(document.validate().is_ok());
    }

    #[test]
    fn move_preserves_id_and_updates_parent() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        let child = document
            .append_block(Some(quote), paragraph("child"))
            .unwrap();
        let other = document
            .append_block(Some(quote), paragraph("other"))
            .unwrap();

        document.move_block(child, Some(quote), 2).unwrap();
        assert_eq!(document.children(quote).unwrap(), &[other, child][..]);
        assert_eq!(document.parent(child), Some(quote));
        assert!(document.node(child).is_some());
        assert!(document.validate().is_ok());
    }

    #[test]
    fn move_into_own_subtree_is_rejected() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        let child = document
            .append_block(Some(quote), BlockKind::Quote(vec![]))
            .unwrap();
        assert!(document.move_block(quote, Some(child), 0).is_err());
    }

    #[test]
    fn snapshot_restore_roundtrip() {
        let mut document = Document::new();
        document.append_block(None, paragraph("second")).unwrap();
        let snapshot = document.snapshot();
        let before = document.clone();

        document.append_block(None, paragraph("third")).unwrap();
        assert_ne!(document, before);
        document.restore(snapshot);
        assert_eq!(document, before);
        assert!(document.validate().is_ok());
    }

    #[test]
    fn validate_detects_missing_table_row() {
        let mut document = Document::new();
        document
            .insert_block(
                None,
                0,
                BlockKind::Table {
                    aligns: vec![Align::Left],
                    rows: vec![BlockId::new(99, 1)],
                },
            )
            .unwrap();
        assert!(document.validate().is_err());
    }

    #[test]
    fn validate_detects_orphan_nodes() {
        let mut document = Document::new();
        let orphan = document.arena.alloc(paragraph("orphan"), None);
        assert!(document.node(orphan).is_some());
        assert!(document.validate().is_err());
    }

    #[test]
    fn validate_detects_staged_nodes() {
        let mut document = Document::new();
        let id = document.root_children()[0];
        assert!(document.arena.stage(id));
        assert!(document.validate().is_err());
    }

    #[test]
    fn empty_document_after_removing_all_blocks_is_valid() {
        let mut document = Document::new();
        document.append_block(None, paragraph("second")).unwrap();
        let ids = document.root_children().to_vec();
        for id in ids {
            document.remove_block(id).unwrap();
            assert!(document.validate().is_ok());
        }
        assert_eq!(document.root_children().len(), 1);
    }
}
