//! Semantic intents accepted by the editor.

use md_document::MarkFlags;

/// Horizontal/structural cursor movement direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDirection {
    /// Previous grapheme or atom.
    Left,
    /// Next grapheme or atom.
    Right,
    /// Document start.
    DocStart,
    /// Document end.
    DocEnd,
}

/// Block style requested by a structural intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockStyle {
    /// Plain paragraph.
    Paragraph,
    /// Heading level 1..=6.
    Heading(u8),
    /// Blockquote.
    Quote,
    /// Fenced code block.
    CodeBlock,
}

/// A user intention, independent of platform keycodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Insert literal text at the current selection.
    InsertText(String),
    /// Split the current block or insert a new block.
    InsertNewline,
    /// Delete the previous grapheme or structure boundary.
    DeleteBackward,
    /// Delete the next grapheme or structure boundary.
    DeleteForward,
    /// Move or extend selections.
    Move {
        /// Movement direction.
        direction: MoveDirection,
        /// Whether to keep the anchor.
        extend: bool,
    },
    /// Toggle one or more inline marks.
    ToggleMark(MarkFlags),
    /// Change the current block style.
    SetBlockKind(BlockStyle),
    /// Toggle list structure.
    ToggleList {
        /// Whether the list is ordered.
        ordered: bool,
    },
    /// Indent or outdent the current list item.
    ChangeIndent {
        /// Whether to outdent.
        outdent: bool,
    },
    /// Paste plain text or Markdown.
    Paste(String),
    /// Undo the last history entry.
    Undo,
    /// Redo the last undone entry.
    Redo,
    /// Set IME marked text.
    ImeSetMarked(String),
    /// Commit IME composition.
    ImeCommit(String),
    /// Cancel IME composition.
    ImeCancel,
}

impl Intent {
    /// Returns a stable name for tracing and tests.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::InsertText(_) => "insert_text",
            Self::InsertNewline => "insert_newline",
            Self::DeleteBackward => "delete_backward",
            Self::DeleteForward => "delete_forward",
            Self::Move { .. } => "move",
            Self::ToggleMark(_) => "toggle_mark",
            Self::SetBlockKind(_) => "set_block_kind",
            Self::ToggleList { .. } => "toggle_list",
            Self::ChangeIndent { .. } => "change_indent",
            Self::Paste(_) => "paste",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::ImeSetMarked(_) => "ime_set_marked",
            Self::ImeCommit(_) => "ime_commit",
            Self::ImeCancel => "ime_cancel",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockStyle, Intent, MoveDirection};
    use md_document::MarkFlags;

    #[test]
    fn intent_names_are_stable() {
        assert_eq!(Intent::InsertText("x".to_owned()).name(), "insert_text");
        assert_eq!(
            Intent::Move {
                direction: MoveDirection::Left,
                extend: false
            }
            .name(),
            "move"
        );
        assert_eq!(Intent::ImeCancel.name(), "ime_cancel");
        assert_eq!(
            Intent::SetBlockKind(BlockStyle::Paragraph).name(),
            "set_block_kind"
        );
        assert_eq!(Intent::ToggleMark(MarkFlags::STRONG).name(), "toggle_mark");
    }
}
