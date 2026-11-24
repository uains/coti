//! Block nodes and the container/leaf rules between them.

use md_core::{TextBuf, TextOffset};

use crate::{BlockId, DocumentError, DocumentResult, ImageRef, InlineContent};

/// The visual alignment of a table column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// No explicit alignment.
    #[default]
    None,
    /// Left-aligned column.
    Left,
    /// Center-aligned column.
    Center,
    /// Right-aligned column.
    Right,
}

/// The source style of a fenced code block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FenceStyle {
    marker: char,
    length: u8,
    indent: u8,
}

impl FenceStyle {
    /// Default backtick fence.
    pub const BACKTICK: Self = Self {
        marker: '`',
        length: 3,
        indent: 0,
    };

    /// Default tilde fence.
    pub const TILDE: Self = Self {
        marker: '~',
        length: 3,
        indent: 0,
    };

    /// Creates a fence style from raw parts. Call [`Self::validate`] before use.
    #[must_use]
    pub const fn new(marker: char, length: u8, indent: u8) -> Self {
        Self {
            marker,
            length,
            indent,
        }
    }

    /// Returns the fence marker character.
    #[must_use]
    pub const fn marker(self) -> char {
        self.marker
    }

    /// Returns the fence length.
    #[must_use]
    pub const fn length(self) -> u8 {
        self.length
    }

    /// Returns the fence indentation.
    #[must_use]
    pub const fn indent(self) -> u8 {
        self.indent
    }

    /// Validates that the marker and length form a legal fenced code block.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for a marker other than `` ` `` / `~` or a
    /// length below three.
    pub fn validate(self) -> DocumentResult<()> {
        if self.marker != '`' && self.marker != '~' {
            return Err(DocumentError::Invariant {
                message: format!("invalid code fence marker {:?}", self.marker),
            });
        }
        if self.length < 3 {
            return Err(DocumentError::Invariant {
                message: format!("code fence length {} is below three", self.length),
            });
        }
        Ok(())
    }
}

impl Default for FenceStyle {
    fn default() -> Self {
        Self::BACKTICK
    }
}

/// The semantic kind and payload of a block node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// A paragraph with inline content.
    Paragraph(InlineContent),

    /// A heading with level 1..=6 and inline content.
    Heading {
        /// Heading level.
        level: u8,
        /// Heading inline content.
        content: InlineContent,
    },

    /// A table cell with inline content only.
    TableCell {
        /// Cell inline content.
        content: InlineContent,
    },

    /// A fenced code block.
    CodeBlock {
        /// Optional info-string language.
        lang: Option<String>,
        /// Raw code text.
        text: TextBuf,
        /// Source fence style.
        fence: FenceStyle,
    },

    /// A raw HTML block.
    HtmlBlock(TextBuf),

    /// A raw math block.
    MathBlock(TextBuf),

    /// A YAML frontmatter block.
    FrontMatter(TextBuf),

    /// A blockquote containing block-level children.
    Quote(Vec<BlockId>),

    /// A list containing list items.
    List {
        /// Whether the list is ordered.
        ordered: bool,
        /// First number for an ordered list.
        start: u32,
        /// Whether the list is tight.
        tight: bool,
        /// List item ids.
        items: Vec<BlockId>,
    },

    /// A list item containing block-level children.
    ListItem {
        /// Optional task checkbox state.
        task: Option<bool>,
        /// Child block ids.
        children: Vec<BlockId>,
    },

    /// A table containing rows.
    Table {
        /// Column alignments.
        aligns: Vec<Align>,
        /// Row ids.
        rows: Vec<BlockId>,
    },

    /// A table row containing cells.
    TableRow {
        /// Cell ids.
        cells: Vec<BlockId>,
    },

    /// A thematic break.
    ThematicBreak,

    /// A block-level image.
    ImageBlock {
        /// Image destination and title.
        image: ImageRef,
        /// Alternative text.
        alt: String,
    },
}

impl BlockKind {
    /// Returns `true` when this kind stores inline or raw content directly.
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        !self.is_container()
    }

    /// Returns `true` when this kind owns child block ids.
    #[must_use]
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            Self::Quote(_)
                | Self::List { .. }
                | Self::ListItem { .. }
                | Self::Table { .. }
                | Self::TableRow { .. }
        )
    }

    /// Returns child block ids for a container.
    #[must_use]
    pub fn children(&self) -> Option<&[BlockId]> {
        match self {
            Self::Quote(children) => Some(children),
            Self::List { items, .. } => Some(items),
            Self::ListItem { children, .. } => Some(children),
            Self::Table { rows, .. } => Some(rows),
            Self::TableRow { cells, .. } => Some(cells),
            _ => None,
        }
    }

    /// Returns a mutable child vector for structural document operations.
    #[allow(dead_code)] // Used by Document structural operations in the next step.
    pub(crate) fn children_mut(&mut self) -> Option<&mut Vec<BlockId>> {
        match self {
            Self::Quote(children) => Some(children),
            Self::List { items, .. } => Some(items),
            Self::ListItem { children, .. } => Some(children),
            Self::Table { rows, .. } => Some(rows),
            Self::TableRow { cells, .. } => Some(cells),
            _ => None,
        }
    }

    /// Returns inline content for inline-bearing leaves.
    #[must_use]
    pub fn inline_content(&self) -> Option<&InlineContent> {
        match self {
            Self::Paragraph(content) | Self::TableCell { content } => Some(content),
            Self::Heading { content, .. } => Some(content),
            _ => None,
        }
    }

    /// Returns the leaf content length in `DocPos` units.
    ///
    /// Text-bearing leaves use UTF-8 byte length; atomic leaves occupy one
    /// unit; containers return `None`.
    #[must_use]
    pub fn leaf_len(&self) -> Option<usize> {
        match self {
            Self::Paragraph(content) | Self::TableCell { content } => Some(content.len()),
            Self::Heading { content, .. } => Some(content.len()),
            Self::CodeBlock { text, .. }
            | Self::HtmlBlock(text)
            | Self::MathBlock(text)
            | Self::FrontMatter(text) => Some(text.len_bytes()),
            Self::ThematicBreak | Self::ImageBlock { .. } => Some(1),
            _ => None,
        }
    }

    /// Returns `true` when `offset` is a legal local offset for this leaf.
    ///
    /// Inline leaves require UTF-8/atom boundaries; raw leaves require UTF-8
    /// character boundaries; atomic leaves accept only 0 or 1.
    #[must_use]
    pub fn is_valid_leaf_offset(&self, offset: usize) -> bool {
        match self {
            Self::Paragraph(content) | Self::TableCell { content } => {
                content.is_valid_offset(offset)
            }
            Self::Heading { content, .. } => content.is_valid_offset(offset),
            Self::CodeBlock { text, .. }
            | Self::HtmlBlock(text)
            | Self::MathBlock(text)
            | Self::FrontMatter(text) => {
                offset <= text.len_bytes() && text.is_char_boundary(TextOffset::new(offset))
            }
            Self::ThematicBreak | Self::ImageBlock { .. } => offset <= 1,
            _ => false,
        }
    }

    /// Returns mutable inline content for structural document operations.
    #[allow(dead_code)] // Used by Document structural operations in the next step.
    pub(crate) fn inline_content_mut(&mut self) -> Option<&mut InlineContent> {
        match self {
            Self::Paragraph(content) | Self::TableCell { content } => Some(content),
            Self::Heading { content, .. } => Some(content),
            _ => None,
        }
    }

    /// Returns a stable kind name for diagnostics.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Paragraph(_) => "paragraph",
            Self::Heading { .. } => "heading",
            Self::TableCell { .. } => "table_cell",
            Self::CodeBlock { .. } => "code_block",
            Self::HtmlBlock(_) => "html_block",
            Self::MathBlock(_) => "math_block",
            Self::FrontMatter(_) => "front_matter",
            Self::Quote(_) => "quote",
            Self::List { .. } => "list",
            Self::ListItem { .. } => "list_item",
            Self::Table { .. } => "table",
            Self::TableRow { .. } => "table_row",
            Self::ThematicBreak => "thematic_break",
            Self::ImageBlock { .. } => "image_block",
        }
    }

    /// Returns `true` when this kind may appear at the document root or inside
    /// a quote/list item.
    #[must_use]
    pub const fn is_block_level(&self) -> bool {
        matches!(
            self,
            Self::Paragraph(_)
                | Self::Heading { .. }
                | Self::CodeBlock { .. }
                | Self::HtmlBlock(_)
                | Self::MathBlock(_)
                | Self::FrontMatter(_)
                | Self::Quote(_)
                | Self::List { .. }
                | Self::Table { .. }
                | Self::ThematicBreak
                | Self::ImageBlock { .. }
        )
    }

    /// Returns `true` when `self` may be a child of `parent`.
    #[must_use]
    pub fn can_be_child_of(&self, parent: &Self) -> bool {
        match parent {
            Self::List { .. } => matches!(self, Self::ListItem { .. }),
            Self::Table { .. } => matches!(self, Self::TableRow { .. }),
            Self::TableRow { .. } => matches!(self, Self::TableCell { .. }),
            Self::Quote(_) | Self::ListItem { .. } => self.is_block_level(),
            _ => false,
        }
    }

    /// Validates local kind invariants.
    ///
    /// Cross-node rules such as table row/cell counts and parent references are
    /// checked by `Document`.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when a heading level, fence, empty container,
    /// or inline content is invalid.
    pub fn validate(&self) -> DocumentResult<()> {
        match self {
            Self::Paragraph(content) | Self::TableCell { content } => content.validate(),
            Self::Heading { level, content } => {
                if !(1..=6).contains(level) {
                    return Err(DocumentError::Invariant {
                        message: format!("heading level {level} is outside 1..=6"),
                    });
                }
                content.validate()
            }
            Self::CodeBlock { fence, .. } => fence.validate(),
            Self::Quote(children) if children.is_empty() => Err(DocumentError::Invariant {
                message: "quote must contain at least one child".to_owned(),
            }),
            Self::List { items, .. } if items.is_empty() => Err(DocumentError::Invariant {
                message: "list must contain at least one item".to_owned(),
            }),
            Self::ListItem { children, .. } if children.is_empty() => {
                Err(DocumentError::Invariant {
                    message: "list item must contain at least one child".to_owned(),
                })
            }
            Self::Table { aligns, rows } => {
                if aligns.is_empty() {
                    return Err(DocumentError::Invariant {
                        message: "table must contain at least one column".to_owned(),
                    });
                }
                if rows.is_empty() {
                    return Err(DocumentError::Invariant {
                        message: "table must contain at least one row".to_owned(),
                    });
                }
                Ok(())
            }
            Self::TableRow { cells } if cells.is_empty() => Err(DocumentError::Invariant {
                message: "table row must contain at least one cell".to_owned(),
            }),
            _ => Ok(()),
        }
    }
}

/// A block tree node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockNode {
    id: BlockId,
    parent: Option<BlockId>,
    kind: BlockKind,
}

impl BlockNode {
    #[allow(dead_code)] // Used by NodeArena allocation in the next step.
    pub(crate) fn new(id: BlockId, parent: Option<BlockId>, kind: BlockKind) -> Self {
        Self { id, parent, kind }
    }

    /// Returns the stable node identity.
    #[must_use]
    pub const fn id(&self) -> BlockId {
        self.id
    }

    /// Returns the parent container id, or `None` at the root.
    #[must_use]
    pub const fn parent(&self) -> Option<BlockId> {
        self.parent
    }

    /// Returns the node kind.
    #[must_use]
    pub const fn kind(&self) -> &BlockKind {
        &self.kind
    }

    #[allow(dead_code)] // Used by Document structural operations in the next step.
    pub(crate) fn kind_mut(&mut self) -> &mut BlockKind {
        &mut self.kind
    }

    #[allow(dead_code)] // Used by Document structural operations in the next step.
    pub(crate) fn set_parent(&mut self, parent: Option<BlockId>) {
        self.parent = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::{Align, BlockKind, BlockNode, FenceStyle};
    use crate::{BlockId, DocumentError, ImageRef, InlineContent};
    use md_core::TextBuf;

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    fn list_item() -> BlockKind {
        BlockKind::ListItem {
            task: None,
            children: vec![BlockId::new(1, 1)],
        }
    }

    #[test]
    fn paragraphs_are_leaves_and_quotes_are_containers() {
        let paragraph = paragraph("hello");
        assert!(paragraph.is_leaf());
        assert!(!paragraph.is_container());
        assert!(paragraph.children().is_none());

        let quote = BlockKind::Quote(vec![BlockId::new(1, 1)]);
        assert!(!quote.is_leaf());
        assert!(quote.is_container());
        assert_eq!(quote.children().unwrap().len(), 1);
    }

    #[test]
    fn container_child_types_are_restricted() {
        let list = BlockKind::List {
            ordered: false,
            start: 1,
            tight: true,
            items: vec![],
        };
        assert!(list_item().can_be_child_of(&list));
        assert!(!paragraph("x").can_be_child_of(&list));

        let table = BlockKind::Table {
            aligns: vec![Align::Left],
            rows: vec![],
        };
        assert!(
            BlockKind::TableRow {
                cells: vec![BlockId::new(2, 1)]
            }
            .can_be_child_of(&table)
        );
        assert!(!paragraph("x").can_be_child_of(&table));

        let row = BlockKind::TableRow {
            cells: vec![BlockId::new(3, 1)],
        };
        assert!(
            BlockKind::TableCell {
                content: InlineContent::from_text("cell")
            }
            .can_be_child_of(&row)
        );
        assert!(!paragraph("x").can_be_child_of(&row));

        let quote = BlockKind::Quote(vec![BlockId::new(4, 1)]);
        assert!(paragraph("x").can_be_child_of(&quote));
        assert!(!list_item().can_be_child_of(&quote));
    }

    #[test]
    fn heading_levels_are_validated() {
        for level in 1..=6 {
            let heading = BlockKind::Heading {
                level,
                content: InlineContent::from_text("title"),
            };
            assert!(heading.validate().is_ok());
        }

        for level in [0, 7, 255] {
            let heading = BlockKind::Heading {
                level,
                content: InlineContent::new(),
            };
            assert!(matches!(
                heading.validate(),
                Err(DocumentError::Invariant { .. })
            ));
        }
    }

    #[test]
    fn code_fences_are_validated() {
        let text = TextBuf::from("fn main() {}");
        let code = BlockKind::CodeBlock {
            lang: Some("rust".to_owned()),
            text,
            fence: FenceStyle::BACKTICK,
        };
        assert!(code.validate().is_ok());

        let invalid = BlockKind::CodeBlock {
            lang: None,
            text: TextBuf::from("x"),
            fence: FenceStyle::new('x', 3, 0),
        };
        assert!(matches!(
            invalid.validate(),
            Err(DocumentError::Invariant { .. })
        ));

        let short = BlockKind::CodeBlock {
            lang: None,
            text: TextBuf::from("x"),
            fence: FenceStyle::new('`', 2, 0),
        };
        assert!(matches!(
            short.validate(),
            Err(DocumentError::Invariant { .. })
        ));
    }

    #[test]
    fn empty_containers_are_rejected() {
        assert!(matches!(
            BlockKind::Quote(vec![]).validate(),
            Err(DocumentError::Invariant { .. })
        ));
        assert!(matches!(
            BlockKind::List {
                ordered: false,
                start: 1,
                tight: true,
                items: vec![]
            }
            .validate(),
            Err(DocumentError::Invariant { .. })
        ));
        assert!(matches!(
            BlockKind::ListItem {
                task: None,
                children: vec![]
            }
            .validate(),
            Err(DocumentError::Invariant { .. })
        ));
        assert!(matches!(
            BlockKind::Table {
                aligns: vec![Align::None],
                rows: vec![]
            }
            .validate(),
            Err(DocumentError::Invariant { .. })
        ));
        assert!(matches!(
            BlockKind::TableRow { cells: vec![] }.validate(),
            Err(DocumentError::Invariant { .. })
        ));
    }

    #[test]
    fn block_node_keeps_identity_when_moved() {
        let id = BlockId::new(5, 2);
        let mut node = BlockNode::new(id, Some(BlockId::new(1, 1)), paragraph("x"));
        assert_eq!(node.id(), id);
        assert_eq!(node.parent(), Some(BlockId::new(1, 1)));

        node.set_parent(Some(BlockId::new(2, 1)));
        assert_eq!(node.id(), id);
        assert_eq!(node.parent(), Some(BlockId::new(2, 1)));
    }

    #[test]
    fn image_block_stores_alt_and_reference() {
        let block = BlockKind::ImageBlock {
            image: ImageRef::new("cover.png").with_title("Cover"),
            alt: "cover".to_owned(),
        };
        assert!(block.is_leaf());
        assert_eq!(block.kind_name(), "image_block");
        assert!(block.validate().is_ok());
    }

    #[test]
    fn children_mut_exposes_container_children_only() {
        let mut quote = BlockKind::Quote(vec![BlockId::new(1, 1)]);
        quote.children_mut().unwrap().push(BlockId::new(2, 1));
        assert_eq!(quote.children().unwrap().len(), 2);

        let mut paragraph = paragraph("x");
        assert!(paragraph.children_mut().is_none());
    }

    #[test]
    fn leaf_lengths_cover_inline_raw_and_atomic_blocks() {
        assert_eq!(paragraph("你好").leaf_len(), Some(6));
        assert_eq!(
            BlockKind::Heading {
                level: 1,
                content: InlineContent::from_text("title")
            }
            .leaf_len(),
            Some(5)
        );
        assert_eq!(
            BlockKind::CodeBlock {
                lang: None,
                text: TextBuf::from("fn main() {}"),
                fence: FenceStyle::BACKTICK
            }
            .leaf_len(),
            Some(12)
        );
        assert_eq!(BlockKind::ThematicBreak.leaf_len(), Some(1));
        assert_eq!(BlockKind::Quote(vec![BlockId::new(1, 1)]).leaf_len(), None);
    }

    #[test]
    fn leaf_offsets_respect_boundaries() {
        let paragraph = paragraph("你");
        assert!(paragraph.is_valid_leaf_offset(0));
        assert!(paragraph.is_valid_leaf_offset(3));
        assert!(!paragraph.is_valid_leaf_offset(1));
        assert!(!paragraph.is_valid_leaf_offset(4));

        let code = BlockKind::CodeBlock {
            lang: None,
            text: TextBuf::from("你"),
            fence: FenceStyle::BACKTICK,
        };
        assert!(code.is_valid_leaf_offset(3));
        assert!(!code.is_valid_leaf_offset(1));

        assert!(BlockKind::ThematicBreak.is_valid_leaf_offset(0));
        assert!(BlockKind::ThematicBreak.is_valid_leaf_offset(1));
        assert!(!BlockKind::ThematicBreak.is_valid_leaf_offset(2));
        assert!(!BlockKind::Quote(vec![BlockId::new(1, 1)]).is_valid_leaf_offset(0));
    }
}
