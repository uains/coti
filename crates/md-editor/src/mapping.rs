//! Linear position mapping primitives.

use md_document::{Affinity, DocPos};

/// A mapped position with deletion status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedPosition {
    /// Mapped linear position.
    pub pos: DocPos,
    /// Preserved affinity.
    pub affinity: Affinity,
    /// Whether the original position fell inside deleted content.
    pub deleted: bool,
}

/// One linear mapping primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingOp {
    /// Insert `len` units at `at`.
    Insert {
        /// Insertion position.
        at: usize,
        /// Inserted length.
        len: usize,
    },
    /// Delete `from..to`.
    Delete {
        /// Inclusive start.
        from: usize,
        /// Exclusive end.
        to: usize,
    },
    /// Move `len` units from `from` to `to`.
    Move {
        /// Original start.
        from: usize,
        /// Destination start.
        to: usize,
        /// Moved length.
        len: usize,
    },
}

/// Ordered mapping primitives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PositionMapping {
    ops: Vec<MappingOp>,
}

impl PositionMapping {
    /// Creates an empty mapping.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one primitive.
    pub fn push(&mut self, op: MappingOp) {
        self.ops.push(op);
    }

    /// Appends all primitives from another mapping.
    pub fn compose(&mut self, other: &Self) {
        self.ops.extend_from_slice(&other.ops);
    }

    /// Returns the primitives.
    #[must_use]
    pub fn ops(&self) -> &[MappingOp] {
        &self.ops
    }

    /// Maps a position through the ordered primitives.
    #[must_use]
    pub fn map(&self, pos: DocPos, affinity: Affinity) -> MappedPosition {
        let mut current = pos.get();
        let mut deleted = false;

        for op in &self.ops {
            match *op {
                MappingOp::Insert { at, len } => {
                    if current >= at {
                        current += len;
                    }
                }
                MappingOp::Delete { from, to } => {
                    if current >= to {
                        current -= to - from;
                    } else if current > from {
                        current = from;
                        deleted = true;
                    } else if current == from && from != to {
                        deleted = true;
                    }
                }
                MappingOp::Move { from, to, len } => {
                    if current >= from && current < from + len {
                        current = to + (current - from);
                    } else if from < to && current >= from + len && current < to {
                        current = current.saturating_sub(len);
                    } else if from < to && current == to {
                        current = to.saturating_sub(len);
                    } else if to < from && current >= to && current < from {
                        current += len;
                    }
                }
            }
        }

        MappedPosition {
            pos: DocPos::new(current),
            affinity,
            deleted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MappingOp, PositionMapping};
    use md_document::{Affinity, DocPos};

    fn map(mapping: &PositionMapping, pos: usize) -> usize {
        mapping
            .map(DocPos::new(pos), Affinity::Downstream)
            .pos
            .get()
    }

    #[test]
    fn insert_shifts_positions_at_or_after_insertion() {
        let mut mapping = PositionMapping::new();
        mapping.push(MappingOp::Insert { at: 3, len: 2 });
        assert_eq!(map(&mapping, 2), 2);
        assert_eq!(map(&mapping, 3), 5);
        assert_eq!(map(&mapping, 7), 9);
    }

    #[test]
    fn delete_shifts_and_marks_inside_positions() {
        let mut mapping = PositionMapping::new();
        mapping.push(MappingOp::Delete { from: 2, to: 5 });
        assert_eq!(map(&mapping, 1), 1);
        assert_eq!(map(&mapping, 2), 2);
        assert_eq!(map(&mapping, 3), 2);
        assert_eq!(map(&mapping, 5), 2);
        assert_eq!(map(&mapping, 7), 4);
        assert!(mapping.map(DocPos::new(3), Affinity::Upstream).deleted);
    }

    #[test]
    fn move_maps_positions_inside_moved_range() {
        let mut mapping = PositionMapping::new();
        mapping.push(MappingOp::Move {
            from: 1,
            to: 4,
            len: 2,
        });
        assert_eq!(map(&mapping, 0), 0);
        assert_eq!(map(&mapping, 1), 4);
        assert_eq!(map(&mapping, 2), 5);
        assert_eq!(map(&mapping, 3), 1);
        assert_eq!(map(&mapping, 4), 2);
    }

    #[test]
    fn compose_applies_primitives_in_order() {
        let mut first = PositionMapping::new();
        first.push(MappingOp::Insert { at: 1, len: 1 });
        let mut second = PositionMapping::new();
        second.push(MappingOp::Delete { from: 0, to: 1 });
        first.compose(&second);
        assert_eq!(map(&first, 0), 0);
        assert_eq!(map(&first, 1), 1);
        assert_eq!(map(&first, 2), 2);
    }
}
