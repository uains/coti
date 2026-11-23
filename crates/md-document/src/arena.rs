//! Block arena with generation-checked ids and transaction-owned staging.

use crate::{BlockId, BlockKind, BlockNode, DocumentError, DocumentResult};

#[derive(Clone, Debug)]
struct NodeSlot {
    generation: u32,
    node: Option<BlockNode>,
    staged: bool,
}

/// Storage for [`BlockNode`]s with stable ids and explicit lifecycle control.
///
/// Slots are never exposed directly. A node can be live, staged (detached but
/// still recoverable), or free. Recycling a slot increments its generation so
/// stale ids fail lookup.
#[derive(Clone, Debug, Default)]
pub struct NodeArena {
    slots: Vec<NodeSlot>,
    free_list: Vec<u32>,
    live_len: usize,
}

impl NodeArena {
    /// Creates an empty arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of live nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live_len
    }

    /// Returns `true` when no live nodes remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live_len == 0
    }

    /// Returns the number of staged nodes.
    #[must_use]
    pub fn staged_len(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.staged && slot.node.is_some())
            .count()
    }

    /// Returns the number of allocated slots, including staged and free slots.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Allocates a live node and returns its id.
    ///
    /// # Panics
    ///
    /// Panics only if the `u32` slot index space is exhausted.
    pub fn alloc(&mut self, kind: BlockKind, parent: Option<BlockId>) -> BlockId {
        while let Some(index) = self.free_list.pop() {
            let Some(slot) = self.slots.get_mut(index as usize) else {
                continue;
            };
            if slot.node.is_some() || slot.staged {
                continue;
            }

            let id = BlockId::new(index, slot.generation);
            slot.node = Some(BlockNode::new(id, parent, kind));
            self.live_len += 1;
            return id;
        }

        let index = u32::try_from(self.slots.len()).expect("arena index space exhausted");
        let id = BlockId::new(index, 1);
        self.slots.push(NodeSlot {
            generation: 1,
            node: Some(BlockNode::new(id, parent, kind)),
            staged: false,
        });
        self.live_len += 1;
        id
    }

    /// Returns a live node by id.
    #[must_use]
    pub fn get(&self, id: BlockId) -> Option<&BlockNode> {
        let slot = self.slots.get(id.index() as usize)?;
        if slot.generation != id.generation() || slot.staged {
            return None;
        }
        slot.node.as_ref()
    }

    /// Returns a mutable live node for controlled document mutation.
    #[allow(dead_code)] // Used by Document structural operations in the next step.
    pub(crate) fn get_mut(&mut self, id: BlockId) -> Option<&mut BlockNode> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        if slot.generation != id.generation() || slot.staged {
            return None;
        }
        slot.node.as_mut()
    }

    /// Returns `true` when `id` refers to a live node.
    #[must_use]
    pub fn contains(&self, id: BlockId) -> bool {
        self.get(id).is_some()
    }

    /// Returns a staged node by id.
    #[must_use]
    pub fn staged_node(&self, id: BlockId) -> Option<&BlockNode> {
        let slot = self.slots.get(id.index() as usize)?;
        if slot.generation != id.generation() || !slot.staged {
            return None;
        }
        slot.node.as_ref()
    }

    /// Returns `true` when `id` refers to a staged node.
    #[must_use]
    pub fn is_staged(&self, id: BlockId) -> bool {
        self.staged_node(id).is_some()
    }

    /// Iterates live nodes in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &BlockNode> {
        self.slots
            .iter()
            .filter(|slot| !slot.staged)
            .filter_map(|slot| slot.node.as_ref())
    }

    /// Iterates staged nodes in slot order.
    pub fn staged_iter(&self) -> impl Iterator<Item = &BlockNode> {
        self.slots
            .iter()
            .filter(|slot| slot.staged)
            .filter_map(|slot| slot.node.as_ref())
    }

    /// Detaches a live node without recycling its slot.
    ///
    /// Returns `false` when the id is stale, already staged, or unknown.
    pub fn stage(&mut self, id: BlockId) -> bool {
        let Some(slot) = self.slots.get_mut(id.index() as usize) else {
            return false;
        };
        if slot.generation != id.generation() || slot.staged || slot.node.is_none() {
            return false;
        }

        slot.staged = true;
        self.live_len -= 1;
        true
    }

    /// Restores a staged node with its original id and generation.
    ///
    /// Returns `false` when the id is not staged.
    pub fn restore(&mut self, id: BlockId) -> bool {
        let Some(slot) = self.slots.get_mut(id.index() as usize) else {
            return false;
        };
        if slot.generation != id.generation() || !slot.staged || slot.node.is_none() {
            return false;
        }

        slot.staged = false;
        self.live_len += 1;
        true
    }

    /// Permanently releases a staged node, invalidating its id.
    ///
    /// Returns the removed node when the id was staged.
    pub fn release_staged(&mut self, id: BlockId) -> Option<BlockNode> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        if slot.generation != id.generation() || !slot.staged {
            return None;
        }

        let node = slot.node.take()?;
        slot.staged = false;
        self.recycle_slot(id.index());
        Some(node)
    }

    /// Immediately frees a live node.
    ///
    /// Transactional callers should prefer [`Self::stage`] followed by either
    /// [`Self::restore`] or [`Self::release_staged`].
    pub fn free(&mut self, id: BlockId) -> Option<BlockNode> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        if slot.generation != id.generation() || slot.staged {
            return None;
        }

        let node = slot.node.take()?;
        self.live_len -= 1;
        self.recycle_slot(id.index());
        Some(node)
    }

    /// Validates slot generations, live counts, staged nodes, and the free list.
    ///
    /// # Errors
    ///
    /// Returns an arena invariant error when internal state is inconsistent.
    pub fn validate(&self) -> DocumentResult<()> {
        let mut live = 0;
        let mut staged = 0;
        let mut free = 0;

        for (index, slot) in self.slots.iter().enumerate() {
            if slot.generation == 0 {
                return Err(arena_error(format!("slot {index} has generation zero")));
            }

            match &slot.node {
                Some(node) => {
                    let expected = BlockId::new(index as u32, slot.generation);
                    if node.id() != expected {
                        return Err(arena_error(format!(
                            "slot {index} holds node {} but expected {expected}",
                            node.id()
                        )));
                    }
                    if slot.staged {
                        staged += 1;
                    } else {
                        live += 1;
                    }
                }
                None => {
                    if slot.staged {
                        return Err(arena_error(format!(
                            "slot {index} is staged without a node"
                        )));
                    }
                    free += 1;
                }
            }
        }

        if live != self.live_len {
            return Err(arena_error(format!(
                "live_len {} does not match live slots {live}",
                self.live_len
            )));
        }

        let mut seen = std::collections::HashSet::new();
        for index in &self.free_list {
            if !seen.insert(*index) {
                return Err(arena_error(format!(
                    "free list contains duplicate slot {index}"
                )));
            }
            let Some(slot) = self.slots.get(*index as usize) else {
                return Err(arena_error(format!(
                    "free list points outside arena at slot {index}"
                )));
            };
            if slot.node.is_some() || slot.staged {
                return Err(arena_error(format!(
                    "free list points at occupied slot {index}"
                )));
            }
        }

        if self.free_list.len() > free {
            return Err(arena_error(format!(
                "free list has {} entries but only {free} free slots",
                self.free_list.len()
            )));
        }

        if staged != self.staged_len() {
            return Err(arena_error(format!(
                "staged_len {} does not match staged slots {staged}",
                self.staged_len()
            )));
        }

        Ok(())
    }

    fn recycle_slot(&mut self, index: u32) {
        let Some(slot) = self.slots.get_mut(index as usize) else {
            return;
        };

        // Exhausted generations are left out of the free list. Reusing a slot
        // after wrapping would make old ids valid again.
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.free_list.push(index);
        }
    }

    #[cfg(test)]
    fn free_list_len(&self) -> usize {
        self.free_list.len()
    }
}

fn arena_error(message: String) -> DocumentError {
    DocumentError::Invariant { message }
}

#[cfg(test)]
mod tests {
    use super::NodeArena;
    use crate::{BlockId, BlockKind, InlineContent};

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn alloc_returns_live_node_with_generation_one() {
        let mut arena = NodeArena::new();
        let id = arena.alloc(paragraph("a"), None);
        assert_eq!(id, BlockId::new(0, 1));
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.get(id).unwrap().kind(), &paragraph("a"));
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn free_invalidates_old_id_and_reuse_gets_new_generation() {
        let mut arena = NodeArena::new();
        let old = arena.alloc(paragraph("a"), None);
        assert!(arena.free(old).is_some());
        assert!(arena.get(old).is_none());
        assert!(!arena.contains(old));

        let new = arena.alloc(paragraph("b"), None);
        assert_eq!(new.index(), old.index());
        assert_eq!(new.generation(), old.generation() + 1);
        assert!(arena.get(old).is_none());
        assert!(arena.get(new).is_some());
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn staging_hides_node_but_restore_keeps_id() {
        let mut arena = NodeArena::new();
        let id = arena.alloc(paragraph("a"), None);
        assert!(arena.stage(id));
        assert!(arena.get(id).is_none());
        assert!(arena.is_staged(id));
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.staged_len(), 1);
        assert_eq!(arena.staged_node(id).unwrap().kind(), &paragraph("a"));
        assert_eq!(arena.iter().count(), 0);
        assert_eq!(arena.staged_iter().count(), 1);

        assert!(arena.restore(id));
        assert!(arena.get(id).is_some());
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.staged_len(), 0);
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn release_staged_invalidates_id() {
        let mut arena = NodeArena::new();
        let id = arena.alloc(paragraph("a"), None);
        assert!(arena.stage(id));
        let released = arena.release_staged(id).unwrap();
        assert_eq!(released.kind(), &paragraph("a"));
        assert!(arena.get(id).is_none());
        assert!(arena.staged_node(id).is_none());

        let reused = arena.alloc(paragraph("b"), None);
        assert_eq!(reused.index(), id.index());
        assert_eq!(reused.generation(), id.generation() + 1);
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn invalid_operations_return_false_or_none() {
        let mut arena = NodeArena::new();
        let id = arena.alloc(paragraph("a"), None);
        assert!(!arena.stage(BlockId::new(9, 1)));
        assert!(!arena.restore(id));
        assert!(arena.free(BlockId::new(9, 1)).is_none());
        assert!(!arena.stage(BlockId::new(9, 1)));
        assert!(arena.free(id).is_some());
        assert!(!arena.stage(id));
        assert!(arena.free(id).is_none());
        assert!(arena.release_staged(id).is_none());
    }

    #[test]
    fn iter_skips_staged_and_free_slots() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(paragraph("a"), None);
        let b = arena.alloc(paragraph("b"), None);
        let c = arena.alloc(paragraph("c"), None);
        assert!(arena.stage(b));
        assert!(arena.free(c).is_some());
        let live: Vec<_> = arena.iter().map(|node| node.id()).collect();
        assert_eq!(live, vec![a]);
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn free_list_reuses_slots_and_validate_detects_clean_state() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(paragraph("a"), None);
        let b = arena.alloc(paragraph("b"), None);
        assert_eq!(arena.free_list_len(), 0);
        assert!(arena.free(a).is_some());
        assert_eq!(arena.free_list_len(), 1);
        let reused = arena.alloc(paragraph("c"), None);
        assert_eq!(reused.index(), a.index());
        assert_eq!(arena.free_list_len(), 0);
        assert!(arena.get(b).is_some());
        assert!(arena.validate().is_ok());
    }

    #[test]
    fn get_mut_updates_kind_without_changing_identity() {
        let mut arena = NodeArena::new();
        let id = arena.alloc(paragraph("a"), None);
        arena
            .get_mut(id)
            .unwrap()
            .kind_mut()
            .clone_from(&paragraph("b"));
        assert_eq!(arena.get(id).unwrap().kind(), &paragraph("b"));
        assert_eq!(arena.get(id).unwrap().id(), id);
    }
}
