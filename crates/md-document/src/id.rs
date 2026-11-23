//! Stable block identity.

use std::fmt;

/// A stable block identity made of an arena slot index and a generation.
///
/// A `BlockId` becomes stale when its slot is recycled. All lookups must verify
/// both the index and the generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId {
    index: u32,
    generation: u32,
}

impl BlockId {
    /// Creates a block id from raw parts.
    ///
    /// The caller is responsible for using a generation returned by the arena;
    /// arbitrary ids simply fail lookup.
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns the arena slot index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns the generation expected for the slot.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.index, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::BlockId;

    #[test]
    fn block_id_exposes_parts_and_formats() {
        let id = BlockId::new(3, 7);
        assert_eq!(id.index(), 3);
        assert_eq!(id.generation(), 7);
        assert_eq!(id.to_string(), "3:7");
        assert!(id < BlockId::new(3, 8));
        assert!(BlockId::new(3, 8) < BlockId::new(4, 1));
    }
}
