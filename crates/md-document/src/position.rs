//! Linear document coordinates and structural position descriptors.

use std::fmt;

use crate::BlockId;

/// A linear position in the document.
///
/// `DocPos` counts UTF-8 bytes for text, one unit per atom, and one unit per
/// inter-leaf Block Separator. It is the only authoritative linear coordinate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DocPos(usize);

impl DocPos {
    /// The zero position.
    pub const ZERO: Self = Self(0);

    /// Creates a document position from a linear unit count.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the linear unit count.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Adds `amount` units, returning `None` on overflow.
    #[must_use]
    pub const fn checked_add(self, amount: usize) -> Option<Self> {
        match self.0.checked_add(amount) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Subtracts `amount` units, returning `None` on underflow.
    #[must_use]
    pub const fn checked_sub(self, amount: usize) -> Option<Self> {
        match self.0.checked_sub(amount) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl fmt::Display for DocPos {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<usize> for DocPos {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<DocPos> for usize {
    fn from(value: DocPos) -> Self {
        value.0
    }
}

/// Which side of an ambiguous boundary a position prefers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    /// Prefer the preceding leaf's end at a separator boundary.
    #[default]
    Upstream,
    /// Prefer the following leaf's start at a separator boundary.
    Downstream,
}

/// A structured position inside a leaf block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// Leaf block containing the position.
    pub block_id: BlockId,
    /// Local offset inside the leaf content.
    pub offset: usize,
    /// Side preference at ambiguous boundaries.
    pub affinity: Affinity,
}

/// A structural insertion point between container children.
///
/// A `None` parent represents a gap at the document root. This position is not
/// forced into the linear `DocPos` space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockGapPosition {
    /// Container parent, or `None` for a root-level gap.
    pub parent: Option<BlockId>,
    /// Insertion index among the parent's children.
    pub child_index: usize,
    /// Side preference at an ambiguous boundary.
    pub affinity: Affinity,
}

#[cfg(test)]
mod tests {
    use super::{Affinity, BlockGapPosition, DocPos, Position};
    use crate::BlockId;

    #[test]
    fn doc_pos_supports_checked_arithmetic() {
        let pos = DocPos::new(3);
        assert_eq!(pos.get(), 3);
        assert_eq!(pos.checked_add(2), Some(DocPos::new(5)));
        assert_eq!(pos.checked_sub(2), Some(DocPos::new(1)));
        assert_eq!(DocPos::ZERO.checked_sub(1), None);
        assert_eq!(pos.to_string(), "3");
        assert_eq!(usize::from(pos), 3);
    }

    #[test]
    fn position_types_expose_affinity_and_block_gaps() {
        let position = Position {
            block_id: BlockId::new(1, 2),
            offset: 4,
            affinity: Affinity::Downstream,
        };
        assert_eq!(position.affinity, Affinity::Downstream);

        let gap = BlockGapPosition {
            parent: None,
            child_index: 2,
            affinity: Affinity::Upstream,
        };
        assert_eq!(gap.child_index, 2);
        assert_eq!(gap.parent, None);
    }
}
