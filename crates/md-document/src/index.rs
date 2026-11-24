//! Derived document index: leaf order, block locations, parent links, and
//! Fenwick prefix sums for `DocPos` lookup.

use std::collections::{HashMap, HashSet};

use crate::{
    Affinity, BlockId, DocPos, Document, DocumentError, DocumentResult, Position, PositionError,
    PositionResult,
};

/// Location metadata for one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockLocation {
    /// Parent container, or `None` for a root block.
    pub parent: Option<BlockId>,
    /// Index inside the parent's child vector.
    pub index_in_parent: usize,
    /// Depth below the root; root children have depth zero.
    pub depth: usize,
    /// Preorder leaf index for a leaf block.
    pub leaf_index: Option<usize>,
    /// First `DocPos` covered by this subtree.
    pub doc_start: usize,
    /// Exclusive last `DocPos` covered by this subtree.
    pub doc_end: usize,
}

impl BlockLocation {
    /// Returns the covered `DocPos` length.
    #[must_use]
    pub const fn doc_len(&self) -> usize {
        self.doc_end.saturating_sub(self.doc_start)
    }
}

/// A leaf lookup result for a linear document position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafHit {
    /// Leaf block containing the position.
    pub block_id: BlockId,
    /// Preorder leaf index.
    pub leaf_index: usize,
    /// Local offset inside the leaf content, in `DocPos` units.
    pub offset: usize,
}

/// Derived index for document positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentIndex {
    leaf_order: Vec<BlockId>,
    leaf_lengths: Vec<usize>,
    block_locations: HashMap<BlockId, BlockLocation>,
    parent_index: HashMap<BlockId, BlockId>,
    length_index: FenwickTree,
    doc_length: usize,
}

impl DocumentIndex {
    /// Rebuilds the index from a document tree.
    #[must_use]
    pub fn rebuild(document: &Document) -> Self {
        let mut builder = IndexBuilder {
            document,
            visited: HashSet::new(),
            leaf_order: Vec::new(),
            leaf_lengths: Vec::new(),
            cells: Vec::new(),
            block_locations: HashMap::new(),
            parent_index: HashMap::new(),
            cursor: 0,
        };

        for (index, id) in document.root_children().iter().enumerate() {
            builder.visit(*id, None, index, 0);
        }

        let doc_length = if builder.leaf_order.is_empty() {
            0
        } else {
            builder.cursor.saturating_sub(1)
        };

        Self {
            leaf_order: builder.leaf_order,
            leaf_lengths: builder.leaf_lengths,
            block_locations: builder.block_locations,
            parent_index: builder.parent_index,
            length_index: FenwickTree::from_values(&builder.cells),
            doc_length,
        }
    }

    /// Returns the total document length in `DocPos` units.
    #[must_use]
    pub fn doc_length(&self) -> usize {
        self.doc_length
    }

    /// Returns the preorder leaf sequence.
    #[must_use]
    pub fn leaf_order(&self) -> &[BlockId] {
        &self.leaf_order
    }

    /// Returns the number of leaves.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.leaf_order.len()
    }

    /// Returns location metadata for a block.
    #[must_use]
    pub fn block_location(&self, id: BlockId) -> Option<&BlockLocation> {
        self.block_locations.get(&id)
    }

    /// Returns the parent recorded in the index.
    #[must_use]
    pub fn parent(&self, id: BlockId) -> Option<BlockId> {
        self.parent_index.get(&id).copied()
    }

    /// Returns the preorder leaf index for a leaf block.
    #[must_use]
    pub fn leaf_index(&self, id: BlockId) -> Option<usize> {
        self.block_location(id)?.leaf_index
    }

    /// Returns the leaf's content length.
    #[must_use]
    pub fn leaf_len(&self, leaf_index: usize) -> Option<usize> {
        self.leaf_lengths.get(leaf_index).copied()
    }

    /// Returns the `DocPos` where a leaf's content starts.
    #[must_use]
    pub fn leaf_start(&self, leaf_index: usize) -> Option<usize> {
        if leaf_index >= self.leaf_order.len() {
            return None;
        }
        Some(self.length_index.prefix_sum(leaf_index))
    }

    /// Resolves a linear position to its leaf and local content offset.
    #[must_use]
    pub fn leaf_at(&self, pos: usize) -> Option<LeafHit> {
        if pos > self.doc_length || self.leaf_order.is_empty() {
            return None;
        }

        let leaf_index = self.length_index.lower_bound(pos.checked_add(1)?)?;
        let block_id = *self.leaf_order.get(leaf_index)?;
        let leaf_len = *self.leaf_lengths.get(leaf_index)?;
        let start = self.length_index.prefix_sum(leaf_index);
        let offset = pos.checked_sub(start)?;

        if offset > leaf_len {
            return None;
        }

        Some(LeafHit {
            block_id,
            leaf_index,
            offset,
        })
    }

    /// Resolves a linear position to a structured leaf position.
    ///
    /// Separator endpoints are resolved according to `affinity`: `Upstream`
    /// prefers the preceding leaf's end, `Downstream` prefers the following
    /// leaf's start. Empty leaves remain addressable with `Downstream`.
    ///
    /// # Errors
    ///
    /// Returns an error when `pos` is beyond the document or the index points at
    /// a non-leaf or stale block.
    pub fn resolve_position(
        &self,
        document: &Document,
        pos: DocPos,
        affinity: Affinity,
    ) -> PositionResult<Position> {
        let linear = pos.get();
        if linear > self.doc_length {
            return Err(PositionError::DocPosOutOfBounds {
                pos: linear,
                len: self.doc_length,
            });
        }

        let hit = self
            .leaf_at(linear)
            .ok_or(PositionError::DocPosOutOfBounds {
                pos: linear,
                len: self.doc_length,
            })?;
        let leaf_len = self
            .leaf_len(hit.leaf_index)
            .ok_or(PositionError::IllegalBoundary)?;
        let kind = document
            .kind(hit.block_id)
            .ok_or_else(|| stale_position(hit.block_id))?;
        let expected_len = kind.leaf_len().ok_or_else(|| PositionError::NotALeaf {
            index: hit.block_id.index(),
            generation: hit.block_id.generation(),
        })?;
        if expected_len != leaf_len || !kind.is_valid_leaf_offset(hit.offset) {
            return Err(PositionError::IllegalBoundary);
        }

        if affinity == Affinity::Upstream && hit.offset == 0 && hit.leaf_index > 0 {
            let previous_index = hit.leaf_index - 1;
            let block_id = *self
                .leaf_order
                .get(previous_index)
                .ok_or(PositionError::IllegalBoundary)?;
            document
                .kind(block_id)
                .ok_or_else(|| stale_position(block_id))?;
            return Ok(Position {
                block_id,
                offset: self.leaf_len(previous_index).unwrap_or(0),
                affinity,
            });
        }

        if affinity == Affinity::Downstream
            && leaf_len > 0
            && hit.offset == leaf_len
            && hit.leaf_index + 1 < self.leaf_count()
        {
            let next_index = hit.leaf_index + 1;
            let block_id = *self
                .leaf_order
                .get(next_index)
                .ok_or(PositionError::IllegalBoundary)?;
            document
                .kind(block_id)
                .ok_or_else(|| stale_position(block_id))?;
            return Ok(Position {
                block_id,
                offset: 0,
                affinity,
            });
        }

        Ok(Position {
            block_id: hit.block_id,
            offset: hit.offset,
            affinity,
        })
    }

    /// Converts a structured leaf position into a linear document position.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale block, a non-leaf block, an out-of-bounds
    /// offset, or an offset inside a UTF-8 code point / atom.
    pub fn to_doc_pos(&self, document: &Document, position: Position) -> PositionResult<DocPos> {
        let location = self
            .block_location(position.block_id)
            .ok_or_else(|| stale_position(position.block_id))?;
        let leaf_index = location.leaf_index.ok_or_else(|| PositionError::NotALeaf {
            index: position.block_id.index(),
            generation: position.block_id.generation(),
        })?;
        let kind = document
            .kind(position.block_id)
            .ok_or_else(|| stale_position(position.block_id))?;
        let leaf_len = kind.leaf_len().ok_or_else(|| PositionError::NotALeaf {
            index: position.block_id.index(),
            generation: position.block_id.generation(),
        })?;

        if position.offset > leaf_len {
            return Err(PositionError::OffsetOutOfBounds {
                offset: position.offset,
                len: leaf_len,
            });
        }
        if !kind.is_valid_leaf_offset(position.offset) {
            return Err(PositionError::IllegalBoundary);
        }

        let start = self
            .leaf_start(leaf_index)
            .ok_or(PositionError::IllegalBoundary)?;
        Ok(DocPos::new(start + position.offset))
    }

    /// Returns `true` when `pos` is within the indexed document length.
    #[must_use]
    pub fn is_valid_doc_pos(&self, pos: DocPos) -> bool {
        pos.get() <= self.doc_length
    }

    /// Validates that this index matches a fresh rebuild of `document`.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when any derived field is stale.
    pub fn validate(&self, document: &Document) -> DocumentResult<()> {
        let expected = Self::rebuild(document);
        if self != &expected {
            return Err(DocumentError::Invariant {
                message: "document index is stale or inconsistent".to_owned(),
            });
        }
        Ok(())
    }
}

fn stale_position(id: BlockId) -> PositionError {
    PositionError::StaleBlockId {
        index: id.index(),
        generation: id.generation(),
    }
}

struct IndexBuilder<'a> {
    document: &'a Document,
    visited: HashSet<BlockId>,
    leaf_order: Vec<BlockId>,
    leaf_lengths: Vec<usize>,
    cells: Vec<usize>,
    block_locations: HashMap<BlockId, BlockLocation>,
    parent_index: HashMap<BlockId, BlockId>,
    cursor: usize,
}

impl IndexBuilder<'_> {
    fn visit(
        &mut self,
        id: BlockId,
        parent: Option<BlockId>,
        index_in_parent: usize,
        depth: usize,
    ) -> Option<usize> {
        if !self.visited.insert(id) {
            return None;
        }

        let node = self.document.node(id)?;
        let start = self.cursor;
        let mut end = start;
        let mut leaf_index = None;

        if let Some(leaf_len) = node.kind().leaf_len() {
            leaf_index = Some(self.leaf_order.len());
            self.leaf_order.push(id);
            self.leaf_lengths.push(leaf_len);
            self.cells.push(leaf_len.saturating_add(1));
            self.cursor = self.cursor.saturating_add(leaf_len).saturating_add(1);
            end = start.saturating_add(leaf_len);
        } else if let Some(children) = node.kind().children() {
            for (child_position, child) in children.iter().enumerate() {
                if let Some(child_end) = self.visit(*child, Some(id), child_position, depth + 1) {
                    end = child_end;
                }
            }
        }

        self.block_locations.insert(
            id,
            BlockLocation {
                parent,
                index_in_parent,
                depth,
                leaf_index,
                doc_start: start,
                doc_end: end,
            },
        );

        if let Some(parent_id) = parent {
            self.parent_index.insert(id, parent_id);
        }

        Some(end)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FenwickTree {
    tree: Vec<usize>,
}
impl FenwickTree {
    fn from_values(values: &[usize]) -> Self {
        let mut tree = vec![0; values.len() + 1];
        tree[1..].copy_from_slice(values);

        for index in 1..=values.len() {
            let parent = index + index.isolate_lowest_one();
            if parent < tree.len() {
                tree[parent] = tree[parent].saturating_add(tree[index]);
            }
        }

        Self { tree }
    }

    fn prefix_sum(&self, end: usize) -> usize {
        let mut index = end.min(self.tree.len().saturating_sub(1));
        let mut sum: usize = 0;
        while index > 0 {
            sum = sum.saturating_add(self.tree[index]);
            index -= index.isolate_lowest_one();
        }
        sum
    }

    fn lower_bound(&self, target: usize) -> Option<usize> {
        if target == 0 {
            return None;
        }

        let len = self.tree.len().saturating_sub(1);
        let mut step = 1;
        while step <= len {
            step <<= 1;
        }
        step >>= 1;

        let mut index = 0;
        let mut remaining = target;
        while step > 0 {
            let next = index + step;
            if next <= len && self.tree[next] < remaining {
                index = next;
                remaining -= self.tree[next];
            }
            step >>= 1;
        }

        (index < len).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::FenwickTree;
    use crate::{
        Affinity, BlockId, BlockKind, DocPos, Document, InlineContent, Position, PositionError,
    };

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn empty_paragraph_index_has_one_leaf_and_zero_length() {
        let document = Document::new();
        let index = document.index();
        assert_eq!(index.leaf_count(), 1);
        assert_eq!(index.doc_length(), 0);
        assert_eq!(index.leaf_start(0), Some(0));
        assert_eq!(index.leaf_at(0).unwrap().offset, 0);
        assert!(index.validate(&document).is_ok());
    }

    #[test]
    fn index_tracks_leaf_starts_and_separators() {
        let mut document = Document::new();
        document.append_block(None, paragraph("Hi")).unwrap();
        document.append_block(None, paragraph("世界")).unwrap();
        let index = document.index();

        assert_eq!(index.leaf_count(), 3);
        assert_eq!(index.doc_length(), 10);
        assert_eq!(index.leaf_start(0), Some(0));
        assert_eq!(index.leaf_start(1), Some(1));
        assert_eq!(index.leaf_start(2), Some(4));

        let first = index.leaf_at(0).unwrap();
        assert_eq!(first.offset, 0);
        let second = index.leaf_at(1).unwrap();
        assert_eq!(second.offset, 0);
        let second_end = index.leaf_at(3).unwrap();
        assert_eq!(second_end.offset, 2);
        let third = index.leaf_at(4).unwrap();
        assert_eq!(third.offset, 0);
        let last = index.leaf_at(10).unwrap();
        assert_eq!(last.offset, 6);
        assert!(index.leaf_at(11).is_none());
    }

    #[test]
    fn index_tracks_container_locations_and_parents() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        let child = document.append_block(Some(quote), paragraph("x")).unwrap();
        let index = document.index();

        let quote_location = index.block_location(quote).unwrap();
        assert_eq!(quote_location.parent, None);
        assert_eq!(quote_location.leaf_index, None);
        assert_eq!(quote_location.doc_start, 0);
        assert_eq!(quote_location.doc_end, 1);

        let child_location = index.block_location(child).unwrap();
        assert_eq!(child_location.parent, Some(quote));
        assert_eq!(child_location.depth, 1);
        assert_eq!(child_location.leaf_index, Some(0));
        assert_eq!(index.parent(child), Some(quote));
        assert!(index.validate(&document).is_ok());
    }

    #[test]
    fn index_rebuilds_after_structural_changes() {
        let mut document = Document::new();
        let id = document.append_block(None, paragraph("abc")).unwrap();
        assert_eq!(document.index().doc_length(), 4);
        document.move_block(id, None, 0).unwrap();
        assert_eq!(document.index().leaf_at(0).unwrap().block_id, id);
        document.remove_block(id).unwrap();
        assert_eq!(document.index().doc_length(), 0);
        assert!(document.index().validate(&document).is_ok());
    }

    #[test]
    fn stale_index_is_detected() {
        let mut document = Document::new();
        document.append_block(None, paragraph("abc")).unwrap();
        let index = document.index().clone();
        document.append_block(None, paragraph("d")).unwrap();
        assert!(index.validate(&document).is_err());
    }

    #[test]
    fn fenwick_lower_bound_finds_expected_cells() {
        let tree = FenwickTree::from_values(&[1, 3, 7]);
        assert_eq!(tree.prefix_sum(0), 0);
        assert_eq!(tree.prefix_sum(1), 1);
        assert_eq!(tree.prefix_sum(2), 4);
        assert_eq!(tree.prefix_sum(3), 11);
        assert_eq!(tree.lower_bound(1), Some(0));
        assert_eq!(tree.lower_bound(2), Some(1));
        assert_eq!(tree.lower_bound(4), Some(1));
        assert_eq!(tree.lower_bound(5), Some(2));
        assert_eq!(tree.lower_bound(11), Some(2));
        assert_eq!(tree.lower_bound(12), None);
    }

    #[test]
    fn separator_positions_resolve_by_affinity() {
        let mut document = Document::new();
        let empty = document.root_children()[0];
        let hi = document.append_block(None, paragraph("Hi")).unwrap();
        let world = document.append_block(None, paragraph("世界")).unwrap();
        let index = document.index();

        let resolve = |pos, affinity| {
            index
                .resolve_position(&document, DocPos::new(pos), affinity)
                .unwrap()
        };

        assert_eq!(
            resolve(0, Affinity::Upstream),
            Position {
                block_id: empty,
                offset: 0,
                affinity: Affinity::Upstream
            }
        );
        assert_eq!(
            resolve(1, Affinity::Upstream).block_id,
            empty,
            "separator end upstream goes to preceding leaf"
        );
        assert_eq!(resolve(1, Affinity::Upstream).offset, 0);
        assert_eq!(resolve(1, Affinity::Downstream).block_id, hi);
        assert_eq!(resolve(1, Affinity::Downstream).offset, 0);

        assert_eq!(resolve(3, Affinity::Upstream).block_id, hi);
        assert_eq!(resolve(3, Affinity::Upstream).offset, 2);
        assert_eq!(resolve(3, Affinity::Downstream).block_id, world);
        assert_eq!(resolve(3, Affinity::Downstream).offset, 0);

        assert_eq!(resolve(4, Affinity::Upstream).block_id, hi);
        assert_eq!(resolve(4, Affinity::Upstream).offset, 2);
        assert_eq!(resolve(4, Affinity::Downstream).block_id, world);
        assert_eq!(resolve(4, Affinity::Downstream).offset, 0);

        assert_eq!(resolve(10, Affinity::Upstream).offset, 6);
        assert_eq!(resolve(10, Affinity::Downstream).offset, 6);
    }

    #[test]
    fn every_doc_pos_roundtrips_with_at_least_one_affinity() {
        let mut document = Document::new();
        document.append_block(None, paragraph("Hi")).unwrap();
        document.append_block(None, paragraph("世界")).unwrap();
        let index = document.index();

        for pos in 0..=index.doc_length() {
            let hit = index.leaf_at(pos).unwrap();
            let kind = document.kind(hit.block_id).unwrap();
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

            assert!(
                upstream_back == DocPos::new(pos) || downstream_back == DocPos::new(pos),
                "pos {pos} did not roundtrip: {upstream:?} / {downstream:?}"
            );
        }
    }

    #[test]
    fn empty_leaf_is_addressable_with_downstream() {
        let mut document = Document::new();
        let first_empty = document.root_children()[0];
        document.append_block(None, paragraph("Hi")).unwrap();
        let middle_empty = document.insert_block(None, 2, paragraph("")).unwrap();
        let tail = document.append_block(None, paragraph("B")).unwrap();
        let index = document.index();

        assert_ne!(first_empty, middle_empty);
        let empty_start = index.block_location(middle_empty).unwrap().doc_start;
        let downstream = index
            .resolve_position(&document, DocPos::new(empty_start), Affinity::Downstream)
            .unwrap();
        assert_eq!(downstream.block_id, middle_empty);
        assert_eq!(downstream.offset, 0);

        let upstream = index
            .resolve_position(&document, DocPos::new(empty_start), Affinity::Upstream)
            .unwrap();
        assert_eq!(upstream.offset, 2, "upstream lands at previous leaf end");
        assert_eq!(
            index.to_doc_pos(&document, downstream).unwrap(),
            DocPos::new(empty_start)
        );
        assert_eq!(index.leaf_at(index.doc_length()).unwrap().block_id, tail);
    }

    #[test]
    fn position_errors_are_specific() {
        let mut document = Document::new();
        let paragraph_id = document.root_children()[0];
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        let index = document.index();

        assert!(matches!(
            index.resolve_position(&document, DocPos::new(1), Affinity::Upstream),
            Err(PositionError::DocPosOutOfBounds { .. })
        ));

        assert!(matches!(
            index.to_doc_pos(
                &document,
                Position {
                    block_id: paragraph_id,
                    offset: 1,
                    affinity: Affinity::Upstream
                }
            ),
            Err(PositionError::OffsetOutOfBounds { .. })
        ));

        assert!(matches!(
            index.to_doc_pos(
                &document,
                Position {
                    block_id: BlockId::new(99, 1),
                    offset: 0,
                    affinity: Affinity::Upstream
                }
            ),
            Err(PositionError::StaleBlockId { .. })
        ));

        assert!(matches!(
            index.to_doc_pos(
                &document,
                Position {
                    block_id: quote,
                    offset: 0,
                    affinity: Affinity::Upstream
                }
            ),
            Err(PositionError::NotALeaf { .. })
        ));
    }

    #[test]
    fn resolve_rejects_offsets_inside_code_points() {
        let mut document = Document::new();
        let cjk = document.append_block(None, paragraph("你好")).unwrap();
        let index = document.index();
        let hit = index.leaf_at(2).unwrap();
        assert_eq!(hit.block_id, cjk);
        assert_eq!(hit.offset, 1);

        assert!(matches!(
            index.resolve_position(&document, DocPos::new(2), Affinity::Downstream),
            Err(PositionError::IllegalBoundary)
        ));
        assert!(matches!(
            index.to_doc_pos(
                &document,
                Position {
                    block_id: cjk,
                    offset: 1,
                    affinity: Affinity::Downstream
                }
            ),
            Err(PositionError::IllegalBoundary)
        ));
    }
}
