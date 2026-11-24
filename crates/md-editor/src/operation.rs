//! Reversible document operations.

use md_core::{TextBuf, TextRange};
use md_document::{BlockId, BlockKind, Document, InlineContent, Marks};

use crate::{EditorError, EditorResult};

/// A minimal document mutation with an inverse.
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    /// Replaces text in an inline or raw leaf.
    ReplaceText {
        /// Target leaf.
        block_id: BlockId,
        /// Local text range.
        range: TextRange,
        /// Replacement text.
        text: String,
        /// Marks for inserted text.
        marks: Marks,
    },
    /// Replaces an entire inline content value.
    SetInlineContent {
        /// Target leaf.
        block_id: BlockId,
        /// New content.
        content: InlineContent,
    },
    /// Replaces raw text.
    SetRawText {
        /// Target raw leaf.
        block_id: BlockId,
        /// New raw text.
        text: TextBuf,
    },
    /// Inserts a block.
    InsertBlock {
        /// Parent container, or `None` for root.
        parent: Option<BlockId>,
        /// Child index.
        index: usize,
        /// Block kind.
        kind: BlockKind,
    },
    /// Removes a leaf block.
    RemoveBlock {
        /// Target block.
        id: BlockId,
    },
    /// Moves a block while preserving its id.
    MoveBlock {
        /// Target block.
        id: BlockId,
        /// New parent.
        new_parent: Option<BlockId>,
        /// New child index.
        index: usize,
    },
    /// Replaces a leaf block's kind.
    SetBlockKind {
        /// Target block.
        id: BlockId,
        /// New kind.
        kind: BlockKind,
    },
    /// Splits an inline leaf at a local offset.
    SplitBlock {
        /// Source block.
        id: BlockId,
        /// Local split offset.
        offset: usize,
    },
    /// Joins two adjacent inline leaves.
    JoinBlocks {
        /// Left block.
        left: BlockId,
        /// Right block.
        right: BlockId,
    },
    /// Indents a list item under its previous sibling.
    IndentListItem {
        /// List item to indent.
        id: BlockId,
    },
    /// Outdents a nested list item to its parent list.
    OutdentListItem {
        /// List item to outdent.
        id: BlockId,
    },
}

/// Result of applying an operation.
#[derive(Clone, Debug)]
pub struct AppliedOperation {
    /// Operation that reverses the applied change.
    pub inverse: Operation,
    /// Blocks whose content or structure was affected.
    pub affected: Vec<BlockId>,
}

impl Operation {
    /// Applies the operation to a document.
    ///
    /// # Errors
    ///
    /// Returns an editor error when the target is stale, incompatible, or the
    /// operation cannot be inverted.
    pub fn apply(&self, document: &mut Document) -> EditorResult<AppliedOperation> {
        match self {
            Self::ReplaceText {
                block_id,
                range,
                text,
                marks,
            } => apply_replace_text(document, *block_id, *range, text, marks.clone()),
            Self::SetInlineContent { block_id, content } => {
                let old = document
                    .kind(*block_id)
                    .and_then(BlockKind::inline_content)
                    .cloned()
                    .ok_or_else(|| not_inline(*block_id))?;
                document.set_inline_content(*block_id, content.clone())?;
                Ok(AppliedOperation {
                    inverse: Self::SetInlineContent {
                        block_id: *block_id,
                        content: old,
                    },
                    affected: vec![*block_id],
                })
            }
            Self::SetRawText { block_id, text } => {
                let old = raw_text(document, *block_id)
                    .ok_or_else(|| not_raw(*block_id))?
                    .clone();
                document.set_raw_text(*block_id, text.clone())?;
                Ok(AppliedOperation {
                    inverse: Self::SetRawText {
                        block_id: *block_id,
                        text: old,
                    },
                    affected: vec![*block_id],
                })
            }
            Self::InsertBlock {
                parent,
                index,
                kind,
            } => {
                let id = document.insert_block(*parent, *index, kind.clone())?;
                Ok(AppliedOperation {
                    inverse: Self::RemoveBlock { id },
                    affected: vec![id],
                })
            }
            Self::RemoveBlock { id } => {
                let node = document.node(*id).ok_or_else(|| stale(*id))?;
                let kind = node.kind().clone();
                if kind.is_container() {
                    return Err(EditorError::Transaction {
                        message: "removing a container is not invertible in phase 5".to_owned(),
                    });
                }
                let parent = node.parent();
                let index = child_index(document, parent, *id)?;
                document.remove_block(*id)?;
                Ok(AppliedOperation {
                    inverse: Self::InsertBlock {
                        parent,
                        index,
                        kind,
                    },
                    affected: vec![*id],
                })
            }
            Self::MoveBlock {
                id,
                new_parent,
                index,
            } => {
                let node = document.node(*id).ok_or_else(|| stale(*id))?;
                let old_parent = node.parent();
                let old_index = child_index(document, old_parent, *id)?;
                document.move_block(*id, *new_parent, *index)?;
                Ok(AppliedOperation {
                    inverse: Self::MoveBlock {
                        id: *id,
                        new_parent: old_parent,
                        index: old_index,
                    },
                    affected: vec![*id],
                })
            }
            Self::SetBlockKind { id, kind } => {
                let old = document.kind(*id).cloned().ok_or_else(|| stale(*id))?;
                document.set_block_kind(*id, kind.clone())?;
                Ok(AppliedOperation {
                    inverse: Self::SetBlockKind { id: *id, kind: old },
                    affected: vec![*id],
                })
            }
            Self::SplitBlock { id, offset } => {
                let new_id = document.split_block(*id, *offset)?;
                Ok(AppliedOperation {
                    inverse: Self::JoinBlocks {
                        left: *id,
                        right: new_id,
                    },
                    affected: vec![*id, new_id],
                })
            }
            Self::JoinBlocks { left, right } => {
                let left_len = document
                    .kind(*left)
                    .and_then(BlockKind::leaf_len)
                    .ok_or_else(|| stale(*left))?;
                document.join_blocks(*left, *right)?;
                Ok(AppliedOperation {
                    inverse: Self::SplitBlock {
                        id: *left,
                        offset: left_len,
                    },
                    affected: vec![*left, *right],
                })
            }
            Self::IndentListItem { id } => {
                indent_list_item(document, *id)?;
                Ok(AppliedOperation {
                    inverse: Self::OutdentListItem { id: *id },
                    affected: vec![*id],
                })
            }
            Self::OutdentListItem { id } => {
                outdent_list_item(document, *id)?;
                Ok(AppliedOperation {
                    inverse: Self::IndentListItem { id: *id },
                    affected: vec![*id],
                })
            }
        }
    }
}

fn indent_list_item(document: &mut Document, id: BlockId) -> EditorResult<()> {
    let snapshot = document.snapshot();
    let result = indent_list_item_inner(document, id);
    if result.is_err() {
        document.restore(snapshot);
    }
    result
}

fn indent_list_item_inner(document: &mut Document, id: BlockId) -> EditorResult<()> {
    if !matches!(document.kind(id), Some(BlockKind::ListItem { .. })) {
        return Err(EditorError::InvalidIntent {
            message: format!("block {id} is not a list item"),
        });
    }
    let list_id = document
        .parent(id)
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "list item has no parent list".to_owned(),
        })?;
    let Some(BlockKind::List {
        ordered,
        start,
        tight,
        items,
    }) = document.kind(list_id).cloned()
    else {
        return Err(EditorError::InvalidIntent {
            message: "list item parent is not a list".to_owned(),
        });
    };
    let index =
        items
            .iter()
            .position(|item| *item == id)
            .ok_or_else(|| EditorError::InvalidIntent {
                message: "list item is not linked from its list".to_owned(),
            })?;
    if index == 0 {
        return Ok(());
    }
    let previous = items[index - 1];
    let previous_children = document
        .children(previous)
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "previous list item has no children".to_owned(),
        })?
        .to_vec();
    let target_list = if let Some(last) = previous_children.last() {
        if matches!(document.kind(*last), Some(BlockKind::List { .. })) {
            *last
        } else {
            document.insert_block(
                Some(previous),
                previous_children.len(),
                BlockKind::List {
                    ordered,
                    start,
                    tight,
                    items: vec![],
                },
            )?
        }
    } else {
        document.insert_block(
            Some(previous),
            0,
            BlockKind::List {
                ordered,
                start,
                tight,
                items: vec![],
            },
        )?
    };
    let target_index = document
        .children(target_list)
        .map_or(0, |children| children.len());
    document.move_block(id, Some(target_list), target_index)?;
    Ok(())
}

fn outdent_list_item(document: &mut Document, id: BlockId) -> EditorResult<()> {
    let snapshot = document.snapshot();
    let result = outdent_list_item_inner(document, id);
    if result.is_err() {
        document.restore(snapshot);
    }
    result
}

fn outdent_list_item_inner(document: &mut Document, id: BlockId) -> EditorResult<()> {
    if !matches!(document.kind(id), Some(BlockKind::ListItem { .. })) {
        return Err(EditorError::InvalidIntent {
            message: format!("block {id} is not a list item"),
        });
    }
    let list_id = document
        .parent(id)
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "list item has no parent list".to_owned(),
        })?;
    let list_parent = document
        .parent(list_id)
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "root-level list cannot be outdented".to_owned(),
        })?;
    if !matches!(document.kind(list_parent), Some(BlockKind::ListItem { .. })) {
        return Err(EditorError::InvalidIntent {
            message: "nested list parent is not a list item".to_owned(),
        });
    }
    let grand_list = document
        .parent(list_parent)
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "list item parent has no parent list".to_owned(),
        })?;
    if !matches!(document.kind(grand_list), Some(BlockKind::List { .. })) {
        return Err(EditorError::InvalidIntent {
            message: "list item parent is not inside a list".to_owned(),
        });
    }
    let grand_index = document
        .children(grand_list)
        .and_then(|children| children.iter().position(|child| *child == list_parent))
        .ok_or_else(|| EditorError::InvalidIntent {
            message: "parent list item is not linked".to_owned(),
        })?;
    document.move_block_allow_empty(id, Some(grand_list), grand_index + 1)?;

    if document
        .children(list_id)
        .is_some_and(|children| children.is_empty())
    {
        document.remove_block(list_id)?;
    }
    Ok(())
}

fn apply_replace_text(
    document: &mut Document,
    block_id: BlockId,
    range: TextRange,
    text: &str,
    marks: Marks,
) -> EditorResult<AppliedOperation> {
    let kind = document
        .kind(block_id)
        .cloned()
        .ok_or_else(|| stale(block_id))?;

    if let Some(content) = kind.inline_content() {
        let old = content.clone();
        let mut content = old.clone();
        content.replace_text(range, text, marks)?;
        document.set_inline_content(block_id, content)?;
        return Ok(AppliedOperation {
            inverse: Operation::SetInlineContent {
                block_id,
                content: old,
            },
            affected: vec![block_id],
        });
    }

    if let Some(raw) = raw_text(document, block_id) {
        let old = raw.clone();
        let mut raw = old.clone();
        raw.replace(range, text)?;
        document.set_raw_text(block_id, raw)?;
        return Ok(AppliedOperation {
            inverse: Operation::SetRawText {
                block_id,
                text: old,
            },
            affected: vec![block_id],
        });
    }

    Err(EditorError::InvalidIntent {
        message: format!("block {block_id} is not a text leaf"),
    })
}

fn raw_text(document: &Document, id: BlockId) -> Option<&TextBuf> {
    match document.kind(id)? {
        BlockKind::CodeBlock { text, .. }
        | BlockKind::HtmlBlock(text)
        | BlockKind::MathBlock(text)
        | BlockKind::FrontMatter(text) => Some(text),
        _ => None,
    }
}

fn child_index(document: &Document, parent: Option<BlockId>, id: BlockId) -> EditorResult<usize> {
    let position = match parent {
        Some(parent) => document
            .children(parent)
            .ok_or_else(|| stale(parent))?
            .iter()
            .position(|child| *child == id),
        None => document
            .root_children()
            .iter()
            .position(|child| *child == id),
    };
    position.ok_or_else(|| EditorError::Transaction {
        message: format!("block {id} is not linked from its parent"),
    })
}

fn stale(id: BlockId) -> EditorError {
    EditorError::Transaction {
        message: format!("block {id} is stale or unknown"),
    }
}

fn not_inline(id: BlockId) -> EditorError {
    EditorError::InvalidIntent {
        message: format!("block {id} has no inline content"),
    }
}

fn not_raw(id: BlockId) -> EditorError {
    EditorError::InvalidIntent {
        message: format!("block {id} has no raw text"),
    }
}

#[cfg(test)]
mod tests {
    use super::Operation;
    use md_core::{TextOffset, TextRange};
    use md_document::{BlockId, BlockKind, Document, InlineContent, MarkFlags, Marks};

    fn paragraph(text: &str) -> BlockKind {
        BlockKind::Paragraph(InlineContent::from_text(text))
    }

    fn text_range(start: usize, end: usize) -> TextRange {
        TextRange::new(TextOffset::new(start), TextOffset::new(end))
    }

    #[test]
    fn replace_text_inverse_restores_inline_content() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        document
            .set_inline_content(root, InlineContent::from_text("abc"))
            .unwrap();

        let operation = Operation::ReplaceText {
            block_id: root,
            range: text_range(1, 2),
            text: "X".to_owned(),
            marks: Marks::with_flags(MarkFlags::STRONG),
        };
        let applied = operation.apply(&mut document).unwrap();
        assert_eq!(
            document.kind(root).unwrap().inline_content().unwrap().len(),
            3
        );

        applied.inverse.apply(&mut document).unwrap();
        assert_eq!(
            document
                .kind(root)
                .unwrap()
                .inline_content()
                .unwrap()
                .runs()[0]
                .text_ref()
                .unwrap()
                .as_str(),
            "abc"
        );
    }

    #[test]
    fn replace_text_works_on_raw_blocks() {
        let mut document = Document::new();
        let root = document.root_children()[0];
        document
            .set_block_kind(
                root,
                BlockKind::CodeBlock {
                    lang: Some("rust".to_owned()),
                    text: "abc".into(),
                    fence: md_document::FenceStyle::BACKTICK,
                },
            )
            .unwrap();

        let applied = Operation::ReplaceText {
            block_id: root,
            range: text_range(0, 1),
            text: "X".to_owned(),
            marks: Marks::plain(),
        }
        .apply(&mut document)
        .unwrap();
        assert_eq!(document.kind(root).unwrap().leaf_len(), Some(3));
        applied.inverse.apply(&mut document).unwrap();
        assert!(
            matches!(document.kind(root).unwrap(), BlockKind::CodeBlock { text, .. } if text.as_str() == "abc")
        );
    }

    #[test]
    fn insert_block_inverse_removes_it() {
        let mut document = Document::new();
        let id = document.append_block(None, paragraph("x")).unwrap();
        let operation = Operation::InsertBlock {
            parent: None,
            index: 0,
            kind: paragraph("new"),
        };
        let applied = operation.apply(&mut document).unwrap();
        applied.inverse.apply(&mut document).unwrap();
        assert!(document.node(id).is_some());
        assert_eq!(document.root_children().len(), 2);
    }

    #[test]
    fn remove_leaf_inverse_reinserts_content() {
        let mut document = Document::new();
        let id = document.append_block(None, paragraph("remove me")).unwrap();
        let operation = Operation::RemoveBlock { id };
        let applied = operation.apply(&mut document).unwrap();
        assert!(document.node(id).is_none());
        applied.inverse.apply(&mut document).unwrap();
        assert!(document.root_children().iter().any(|child| matches!(document.kind(*child), Some(BlockKind::Paragraph(content)) if content.len() == 9)));
    }

    #[test]
    fn move_block_inverse_restores_order() {
        let mut document = Document::new();
        let first = document.append_block(None, paragraph("a")).unwrap();
        let second = document.append_block(None, paragraph("b")).unwrap();
        let operation = Operation::MoveBlock {
            id: first,
            new_parent: None,
            index: 3,
        };
        let applied = operation.apply(&mut document).unwrap();
        assert_eq!(
            document.root_children(),
            &[document.root_children()[0], second, first]
        );
        applied.inverse.apply(&mut document).unwrap();
        assert_eq!(document.root_children()[1], first);
        assert_eq!(document.root_children()[2], second);
    }

    #[test]
    fn set_block_kind_inverse_restores_kind() {
        let mut document = Document::new();
        let id = document.root_children()[0];
        let operation = Operation::SetBlockKind {
            id,
            kind: BlockKind::Heading {
                level: 2,
                content: InlineContent::from_text("title"),
            },
        };
        let applied = operation.apply(&mut document).unwrap();
        assert!(matches!(
            document.kind(id),
            Some(BlockKind::Heading { level: 2, .. })
        ));
        applied.inverse.apply(&mut document).unwrap();
        assert!(matches!(document.kind(id), Some(BlockKind::Paragraph(_))));
    }

    #[test]
    fn removing_container_is_rejected() {
        let mut document = Document::new();
        let quote = document
            .insert_block(None, 0, BlockKind::Quote(vec![]))
            .unwrap();
        let operation = Operation::RemoveBlock { id: quote };
        assert!(operation.apply(&mut document).is_err());
    }

    #[test]
    fn stale_block_is_reported() {
        let mut document = Document::new();
        let operation = Operation::SetInlineContent {
            block_id: BlockId::new(99, 1),
            content: InlineContent::new(),
        };
        assert!(operation.apply(&mut document).is_err());
    }

    #[test]
    fn split_block_inverse_joins_again() {
        let mut document = Document::new();
        let id = document.append_block(None, paragraph("abc")).unwrap();
        let applied = Operation::SplitBlock { id, offset: 1 }
            .apply(&mut document)
            .unwrap();
        assert_eq!(document.root_children().len(), 3);
        applied.inverse.apply(&mut document).unwrap();
        assert_eq!(document.kind(id).unwrap().leaf_len(), Some(3));
    }

    #[test]
    fn join_blocks_inverse_splits_at_left_length() {
        let mut document = Document::new();
        let left = document.append_block(None, paragraph("a")).unwrap();
        let right = document.append_block(None, paragraph("bc")).unwrap();
        let applied = Operation::JoinBlocks { left, right }
            .apply(&mut document)
            .unwrap();
        assert!(document.node(right).is_none());
        applied.inverse.apply(&mut document).unwrap();
        assert_eq!(document.kind(left).unwrap().leaf_len(), Some(1));
    }

    #[test]
    fn indent_list_item_creates_nested_list() {
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
        let first = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document.append_block(Some(first), paragraph("a")).unwrap();
        let second = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document.append_block(Some(second), paragraph("b")).unwrap();

        let applied = Operation::IndentListItem { id: second }
            .apply(&mut document)
            .unwrap();
        assert!(document.validate().is_ok());
        assert!(
            document
                .children(first)
                .unwrap()
                .iter()
                .any(|child| matches!(document.kind(*child), Some(BlockKind::List { .. })))
        );
        applied.inverse.apply(&mut document).unwrap();
        assert!(document.validate().is_ok());
        assert_eq!(document.children(list).unwrap().len(), 2);
    }

    #[test]
    fn indent_first_item_is_noop() {
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
        let first = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document.append_block(Some(first), paragraph("a")).unwrap();
        assert!(
            Operation::IndentListItem { id: first }
                .apply(&mut document)
                .is_ok()
        );
        assert_eq!(document.children(list).unwrap().len(), 1);
    }

    #[test]
    fn outdent_nested_item_removes_empty_list() {
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
        let first = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document.append_block(Some(first), paragraph("a")).unwrap();
        let second = document
            .append_block(
                Some(list),
                BlockKind::ListItem {
                    task: None,
                    children: vec![],
                },
            )
            .unwrap();
        document.append_block(Some(second), paragraph("b")).unwrap();
        Operation::IndentListItem { id: second }
            .apply(&mut document)
            .unwrap();
        Operation::OutdentListItem { id: second }
            .apply(&mut document)
            .unwrap();
        assert!(document.validate().is_ok());
        assert_eq!(document.children(list).unwrap().len(), 2);
    }
}
