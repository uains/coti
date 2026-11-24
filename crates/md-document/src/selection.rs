//! Selections, anchors, table-cell ranges, and multi-selection sets.

use crate::{Affinity, BlockId, BlockKind, DocPos, Document, DocumentIndex, PositionError};

/// One end of a selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchor {
    /// Linear document position.
    pub pos: DocPos,
    /// Side preference at ambiguous boundaries.
    pub affinity: Affinity,
}

impl Anchor {
    /// Creates an anchor.
    #[must_use]
    pub const fn new(pos: DocPos, affinity: Affinity) -> Self {
        Self { pos, affinity }
    }
}

/// A table cell coordinate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellPos {
    /// Zero-based row index.
    pub row: usize,
    /// Zero-based column index.
    pub column: usize,
}

impl CellPos {
    /// Creates a cell coordinate.
    #[must_use]
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

/// The kind of selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    /// A linear text range.
    Range,
    /// A whole block node.
    Node(BlockId),
    /// A rectangular table-cell range.
    TableCells {
        /// Table block containing the cells.
        table: BlockId,
        /// Inclusive start cell.
        start: CellPos,
        /// Inclusive end cell.
        end: CellPos,
    },
}

/// One selection with anchor/head direction and an optional goal x coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    /// Fixed end of the selection.
    pub anchor: Anchor,
    /// Moving end of the selection.
    pub head: Anchor,
    /// Selection semantics.
    pub kind: SelectionKind,
    /// Preserved horizontal goal for vertical movement.
    pub goal_x: Option<f32>,
}

impl Selection {
    /// Creates a collapsed range selection.
    #[must_use]
    pub const fn collapsed(pos: DocPos, affinity: Affinity) -> Self {
        Self {
            anchor: Anchor::new(pos, affinity),
            head: Anchor::new(pos, affinity),
            kind: SelectionKind::Range,
            goal_x: None,
        }
    }

    /// Creates a range selection from two anchors.
    #[must_use]
    pub const fn range(anchor: Anchor, head: Anchor) -> Self {
        Self {
            anchor,
            head,
            kind: SelectionKind::Range,
            goal_x: None,
        }
    }

    /// Creates a node selection.
    #[must_use]
    pub const fn node(block_id: BlockId) -> Self {
        Self {
            anchor: Anchor::new(DocPos::ZERO, Affinity::Upstream),
            head: Anchor::new(DocPos::ZERO, Affinity::Upstream),
            kind: SelectionKind::Node(block_id),
            goal_x: None,
        }
    }

    /// Creates a table-cell selection.
    #[must_use]
    pub const fn table_cells(table: BlockId, start: CellPos, end: CellPos) -> Self {
        Self {
            anchor: Anchor::new(DocPos::ZERO, Affinity::Upstream),
            head: Anchor::new(DocPos::ZERO, Affinity::Upstream),
            kind: SelectionKind::TableCells { table, start, end },
            goal_x: None,
        }
    }

    /// Adds a goal x coordinate.
    #[must_use]
    pub const fn with_goal_x(mut self, goal_x: f32) -> Self {
        self.goal_x = Some(goal_x);
        self
    }

    /// Returns the lower endpoint.
    #[must_use]
    pub fn start(&self) -> DocPos {
        self.anchor.pos.min(self.head.pos)
    }

    /// Returns the higher endpoint.
    #[must_use]
    pub fn end(&self) -> DocPos {
        self.anchor.pos.max(self.head.pos)
    }

    /// Returns `true` when the selection is a collapsed range.
    #[must_use]
    pub fn is_collapsed(&self) -> bool {
        matches!(self.kind, SelectionKind::Range) && self.anchor.pos == self.head.pos
    }

    /// Returns `true` when the head is before the anchor.
    #[must_use]
    pub fn is_reversed(&self) -> bool {
        self.head.pos < self.anchor.pos
    }

    /// Clears the goal x coordinate after a horizontal move or content change.
    pub fn clear_goal_x(&mut self) {
        self.goal_x = None;
    }

    /// Validates this selection against a document and its index.
    ///
    /// # Errors
    ///
    /// Returns a selection error for illegal positions, stale blocks, invalid
    /// table references, or reversed table-cell ranges.
    pub fn validate(&self, document: &Document, index: &DocumentIndex) -> SelectionResult<()> {
        match self.kind {
            SelectionKind::Range => {
                validate_range_anchor(document, index, self.anchor)?;
                validate_range_anchor(document, index, self.head)?;
                Ok(())
            }
            SelectionKind::Node(block_id) => {
                if document.node(block_id).is_none() {
                    return Err(SelectionError::StaleBlock {
                        index: block_id.index(),
                        generation: block_id.generation(),
                    });
                }
                Ok(())
            }
            SelectionKind::TableCells { table, start, end } => {
                let Some(kind) = document.kind(table) else {
                    return Err(SelectionError::StaleBlock {
                        index: table.index(),
                        generation: table.generation(),
                    });
                };
                let BlockKind::Table { aligns, rows } = kind else {
                    return Err(SelectionError::NotATable {
                        index: table.index(),
                        generation: table.generation(),
                    });
                };
                if start > end {
                    return Err(SelectionError::ReversedCellRange);
                }
                for cell in [start, end] {
                    if cell.row >= rows.len() || cell.column >= aligns.len() {
                        return Err(SelectionError::CellOutOfBounds {
                            row: cell.row,
                            column: cell.column,
                        });
                    }
                }
                Ok(())
            }
        }
    }
}

/// A set of selections with one primary selection.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionSet {
    ranges: Vec<Selection>,
    primary: usize,
}

impl SelectionSet {
    /// Creates a set containing one selection.
    #[must_use]
    pub fn new(selection: Selection) -> Self {
        Self {
            ranges: vec![selection],
            primary: 0,
        }
    }

    /// Creates a set from selections, sorting them in descending `DocPos` order.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty set or an out-of-bounds primary index.
    pub fn from_selections(selections: Vec<Selection>, primary: usize) -> SelectionResult<Self> {
        if selections.is_empty() {
            return Err(SelectionError::Empty);
        }
        if primary >= selections.len() {
            return Err(SelectionError::PrimaryOutOfBounds {
                primary,
                len: selections.len(),
            });
        }

        let mut set = Self {
            ranges: selections,
            primary,
        };
        set.sort_descending();
        Ok(set)
    }

    /// Returns the number of selections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Returns `true` when the set has no selections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Returns the primary index.
    #[must_use]
    pub const fn primary(&self) -> usize {
        self.primary
    }

    /// Returns the primary selection.
    #[must_use]
    pub fn primary_selection(&self) -> Option<&Selection> {
        self.ranges.get(self.primary)
    }

    /// Returns all selections in their current order.
    #[must_use]
    pub fn selections(&self) -> &[Selection] {
        &self.ranges
    }

    /// Returns an iterator over selections.
    pub fn iter(&self) -> impl Iterator<Item = &Selection> {
        self.ranges.iter()
    }

    /// Appends a selection and re-sorts in descending order.
    pub fn push(&mut self, selection: Selection) {
        self.ranges.push(selection);
        self.sort_descending();
    }

    /// Sets the primary selection index.
    ///
    /// # Errors
    ///
    /// Returns an error when `primary` is out of bounds.
    pub fn set_primary(&mut self, primary: usize) -> SelectionResult<()> {
        if primary >= self.ranges.len() {
            return Err(SelectionError::PrimaryOutOfBounds {
                primary,
                len: self.ranges.len(),
            });
        }
        self.primary = primary;
        Ok(())
    }

    /// Sorts selections by descending start position, preserving the primary.
    pub fn sort_descending(&mut self) {
        let primary = self.primary;
        let mut indexed: Vec<(usize, Selection)> = self.ranges.drain(..).enumerate().collect();
        indexed.sort_by(|(_, left), (_, right)| {
            right
                .start()
                .cmp(&left.start())
                .then_with(|| right.end().cmp(&left.end()))
        });
        self.primary = indexed
            .iter()
            .position(|(old_index, _)| *old_index == primary)
            .unwrap_or(0);
        self.ranges = indexed
            .into_iter()
            .map(|(_, selection)| selection)
            .collect();
    }

    /// Validates every selection plus ordering and non-overlap of range pairs.
    ///
    /// # Errors
    ///
    /// Returns an empty/primary error, an individual selection error, or an
    /// ordering/overlap error.
    pub fn validate(&self, document: &Document, index: &DocumentIndex) -> SelectionResult<()> {
        if self.ranges.is_empty() {
            return Err(SelectionError::Empty);
        }
        if self.primary >= self.ranges.len() {
            return Err(SelectionError::PrimaryOutOfBounds {
                primary: self.primary,
                len: self.ranges.len(),
            });
        }

        for selection in &self.ranges {
            selection.validate(document, index)?;
        }

        for pair in self.ranges.windows(2) {
            let previous = &pair[0];
            let next = &pair[1];
            if previous.start() < next.start() {
                return Err(SelectionError::UnsortedRanges);
            }
            if matches!(previous.kind, SelectionKind::Range)
                && matches!(next.kind, SelectionKind::Range)
                && previous.start() < next.end()
            {
                return Err(SelectionError::OverlappingRanges);
            }
        }

        Ok(())
    }
}

/// Errors produced while constructing or validating selections.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionError {
    /// A selection set cannot be empty.
    #[error("selection set must contain at least one selection")]
    Empty,

    /// The primary selection index is out of bounds.
    #[error("primary selection index {primary} is out of bounds for {len} selections")]
    PrimaryOutOfBounds {
        /// Requested primary index.
        primary: usize,
        /// Number of selections.
        len: usize,
    },

    /// A selection endpoint is beyond the document length.
    #[error("selection position {pos} is out of bounds for document length {len}")]
    DocPosOutOfBounds {
        /// Rejected position.
        pos: usize,
        /// Document length.
        len: usize,
    },

    /// A selection endpoint is not on a legal document boundary.
    #[error("selection position {pos} is not a legal document boundary")]
    IllegalDocPos {
        /// Rejected position.
        pos: usize,
    },

    /// A selected block is stale or unknown.
    #[error("selected block {index}:{generation} is stale or unknown")]
    StaleBlock {
        /// Arena slot index.
        index: u32,
        /// Generation.
        generation: u32,
    },

    /// A table-cell selection references a non-table block.
    #[error("selected block {index}:{generation} is not a table")]
    NotATable {
        /// Arena slot index.
        index: u32,
        /// Generation.
        generation: u32,
    },

    /// A cell coordinate is outside the table.
    #[error("table cell ({row}, {column}) is outside the table")]
    CellOutOfBounds {
        /// Row index.
        row: usize,
        /// Column index.
        column: usize,
    },

    /// The table-cell range is reversed.
    #[error("table cell range start is after end")]
    ReversedCellRange,

    /// Two range selections overlap.
    #[error("selection ranges overlap")]
    OverlappingRanges,

    /// Range selections are not sorted in descending start order.
    #[error("selection ranges are not sorted in descending DocPos order")]
    UnsortedRanges,
}

/// Result alias for selection operations.
pub type SelectionResult<T> = Result<T, SelectionError>;

fn validate_range_anchor(
    document: &Document,
    index: &DocumentIndex,
    anchor: Anchor,
) -> SelectionResult<()> {
    index
        .resolve_position(document, anchor.pos, anchor.affinity)
        .map(|_| ())
        .map_err(|error| match error {
            PositionError::DocPosOutOfBounds { pos, .. } => SelectionError::DocPosOutOfBounds {
                pos,
                len: index.doc_length(),
            },
            PositionError::IllegalBoundary | PositionError::OffsetOutOfBounds { .. } => {
                SelectionError::IllegalDocPos {
                    pos: anchor.pos.get(),
                }
            }
            PositionError::StaleBlockId { index, generation }
            | PositionError::NotALeaf { index, generation } => {
                SelectionError::StaleBlock { index, generation }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{Anchor, CellPos, Selection, SelectionError, SelectionKind, SelectionSet};
    use crate::{Affinity, Align, BlockKind, DocPos, Document, InlineContent};

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    #[test]
    fn range_orders_endpoints_and_detects_collapse() {
        let selection = Selection::range(
            Anchor::new(DocPos::new(5), Affinity::Downstream),
            Anchor::new(DocPos::new(2), Affinity::Upstream),
        );
        assert_eq!(selection.start(), DocPos::new(2));
        assert_eq!(selection.end(), DocPos::new(5));
        assert!(selection.is_reversed());
        assert!(!selection.is_collapsed());

        let collapsed = Selection::collapsed(DocPos::new(3), Affinity::Upstream);
        assert!(collapsed.is_collapsed());
        assert!(!collapsed.is_reversed());
    }

    #[test]
    fn node_selection_is_not_collapsed() {
        let node = Selection::node(crate::BlockId::new(1, 1));
        assert!(!node.is_collapsed());
        assert_eq!(node.kind, SelectionKind::Node(crate::BlockId::new(1, 1)));
    }

    #[test]
    fn selection_set_sorts_descending_and_preserves_primary() {
        let first = Selection::collapsed(DocPos::new(1), Affinity::Upstream);
        let second = Selection::collapsed(DocPos::new(5), Affinity::Upstream);
        let third = Selection::collapsed(DocPos::new(3), Affinity::Upstream);
        let set = SelectionSet::from_selections(vec![first, second, third], 2).unwrap();

        assert_eq!(set.len(), 3);
        assert_eq!(set.primary(), 1);
        assert_eq!(set.selections()[0].start(), DocPos::new(5));
        assert_eq!(set.selections()[1].start(), DocPos::new(3));
        assert_eq!(set.selections()[2].start(), DocPos::new(1));
        assert_eq!(set.primary_selection().unwrap().start(), DocPos::new(3));
    }

    #[test]
    fn selection_set_rejects_bad_primary_and_empty() {
        assert_eq!(
            SelectionSet::from_selections(vec![], 0).unwrap_err(),
            SelectionError::Empty
        );
        let selection = Selection::collapsed(DocPos::ZERO, Affinity::Upstream);
        assert!(matches!(
            SelectionSet::from_selections(vec![selection], 1),
            Err(SelectionError::PrimaryOutOfBounds { primary: 1, len: 1 })
        ));
    }

    #[test]
    fn selection_set_detects_overlap_and_order() {
        let mut document = Document::new();
        document.append_block(None, paragraph("abcd")).unwrap();
        let index = document.index();
        let overlapping = SelectionSet::from_selections(
            vec![
                Selection::range(
                    Anchor::new(DocPos::new(0), Affinity::Upstream),
                    Anchor::new(DocPos::new(2), Affinity::Upstream),
                ),
                Selection::range(
                    Anchor::new(DocPos::new(0), Affinity::Upstream),
                    Anchor::new(DocPos::new(3), Affinity::Upstream),
                ),
            ],
            0,
        )
        .unwrap();
        assert_eq!(
            overlapping.validate(&document, index).unwrap_err(),
            SelectionError::OverlappingRanges
        );
    }

    #[test]
    fn selection_validation_checks_docpos() {
        let mut document = Document::new();
        document.append_block(None, paragraph("你")).unwrap();
        let index = document.index();

        let valid = Selection::collapsed(DocPos::ZERO, Affinity::Upstream);
        assert!(valid.validate(&document, index).is_ok());

        let illegal = Selection::collapsed(DocPos::new(2), Affinity::Upstream);
        assert!(matches!(
            illegal.validate(&document, index),
            Err(SelectionError::IllegalDocPos { pos: 2 })
        ));

        let out_of_bounds = Selection::collapsed(DocPos::new(99), Affinity::Upstream);
        assert!(matches!(
            out_of_bounds.validate(&document, index),
            Err(SelectionError::DocPosOutOfBounds { pos: 99, .. })
        ));
    }

    #[test]
    fn table_cell_selection_validates_bounds() {
        let mut document = Document::new();
        let table = document
            .insert_block(
                None,
                0,
                BlockKind::Table {
                    aligns: vec![Align::Left, Align::Right],
                    rows: vec![],
                },
            )
            .unwrap();
        let row = document
            .append_block(Some(table), BlockKind::TableRow { cells: vec![] })
            .unwrap();
        document
            .append_block(
                Some(row),
                BlockKind::TableCell {
                    content: InlineContent::from_text("a"),
                },
            )
            .unwrap();
        document
            .append_block(
                Some(row),
                BlockKind::TableCell {
                    content: InlineContent::from_text("b"),
                },
            )
            .unwrap();
        assert!(document.validate().is_ok());
        let index = document.index();

        let valid = Selection::table_cells(table, CellPos::new(0, 0), CellPos::new(0, 1));
        assert!(valid.validate(&document, index).is_ok());

        let out_of_bounds = Selection::table_cells(table, CellPos::new(0, 0), CellPos::new(1, 1));
        assert!(matches!(
            out_of_bounds.validate(&document, index),
            Err(SelectionError::CellOutOfBounds { .. })
        ));

        let reversed = Selection::table_cells(table, CellPos::new(0, 1), CellPos::new(0, 0));
        assert_eq!(
            reversed.validate(&document, index).unwrap_err(),
            SelectionError::ReversedCellRange
        );
    }
}
