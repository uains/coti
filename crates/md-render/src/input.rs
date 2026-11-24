//! Input adaptation and viewport state for the render layer.

use md_core::{Point, Rect};
use md_document::DocPos;
use md_editor::{Intent, MoveDirection};
use md_layout::LayoutTree;

/// Platform-independent key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// Printable character input.
    Character(String),
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Enter.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Undo shortcut.
    Undo,
    /// Redo shortcut.
    Redo,
}

/// Maps a key event to an editor intent.
#[must_use]
pub fn key_to_intent(key: &Key, shift: bool, alt: bool) -> Option<Intent> {
    let extend = shift && !alt;
    Some(match key {
        Key::Character(text) => Intent::InsertText(text.clone()),
        Key::Backspace => Intent::DeleteBackward,
        Key::Delete => Intent::DeleteForward,
        Key::Enter => Intent::InsertNewline,
        Key::Tab => Intent::ChangeIndent { outdent: shift },
        Key::Escape => Intent::ImeCancel,
        Key::Left => Intent::Move {
            direction: MoveDirection::Left,
            extend,
        },
        Key::Right => Intent::Move {
            direction: MoveDirection::Right,
            extend,
        },
        Key::Home => Intent::Move {
            direction: MoveDirection::DocStart,
            extend,
        },
        Key::End => Intent::Move {
            direction: MoveDirection::DocEnd,
            extend,
        },
        Key::Undo => Intent::Undo,
        Key::Redo => Intent::Redo,
    })
}

/// Mouse input adapter that maps points through layout hit-testing.
#[derive(Clone, Debug, Default)]
pub struct InputAdapter {
    dragging: bool,
}

impl InputAdapter {
    /// Handles pointer down.
    pub fn mouse_down(&mut self, layout: &LayoutTree, point: Point) -> Option<DocPos> {
        self.dragging = true;
        layout.position_at_point(point)
    }

    /// Handles pointer drag.
    pub fn mouse_drag(&mut self, layout: &LayoutTree, point: Point) -> Option<DocPos> {
        if self.dragging {
            layout.position_at_point(point)
        } else {
            None
        }
    }

    /// Handles pointer up.
    pub fn mouse_up(&mut self) {
        self.dragging = false;
    }

    /// Returns whether a drag is active.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
}

/// Vertical scroll state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    /// Current vertical offset.
    pub offset_y: f32,
    /// Viewport height.
    pub viewport_height: f32,
}

impl ScrollState {
    /// Clamps the offset to the content height.
    pub fn clamp(&mut self, content_height: f32) {
        let max = (content_height - self.viewport_height).max(0.0);
        self.offset_y = self.offset_y.clamp(0.0, max);
    }

    /// Scrolls by a delta and clamps.
    pub fn scroll_by(&mut self, delta: f32, content_height: f32) {
        self.offset_y += delta;
        self.clamp(content_height);
    }

    /// Returns the viewport rectangle in layout coordinates.
    #[must_use]
    pub fn viewport(&self, width: f32) -> Rect {
        Rect::new(0.0, self.offset_y, width, self.viewport_height)
    }
}

/// Caret blink state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretBlink {
    /// Blink period in milliseconds.
    pub period_ms: u64,
}

impl Default for CaretBlink {
    fn default() -> Self {
        Self { period_ms: 500 }
    }
}

impl CaretBlink {
    /// Returns caret visibility at a monotonic time.
    #[must_use]
    pub fn visible_at(self, now_ms: u64) -> bool {
        if self.period_ms == 0 {
            return true;
        }
        (now_ms / self.period_ms).is_multiple_of(2)
    }
}

#[cfg(test)]
mod tests {
    use super::{CaretBlink, InputAdapter, Key, ScrollState, key_to_intent};
    use md_core::{Point, Size};
    use md_document::{DocPos, Document, InlineContent};
    use md_editor::{Intent, MoveDirection};
    use md_layout::{LayoutEnv, LayoutTree, ResolvedStyle};

    #[test]
    fn key_mapping_covers_editing_intents() {
        assert_eq!(
            key_to_intent(&Key::Character("a".to_owned()), false, false),
            Some(Intent::InsertText("a".to_owned()))
        );
        assert_eq!(
            key_to_intent(&Key::Left, true, false),
            Some(Intent::Move {
                direction: MoveDirection::Left,
                extend: true
            })
        );
        assert_eq!(key_to_intent(&Key::Undo, false, false), Some(Intent::Undo));
    }

    #[test]
    fn mouse_click_maps_through_layout() {
        let mut document = Document::new();
        document
            .set_inline_content(document.root_children()[0], InlineContent::from_text("abc"))
            .unwrap();
        let layout = LayoutTree::full(
            &document,
            LayoutEnv::new(Size::new(300.0, 200.0), ResolvedStyle::default()),
        );
        let caret = layout.caret_rect(DocPos::new(1)).unwrap();
        let mut adapter = InputAdapter::default();
        assert_eq!(
            adapter.mouse_down(&layout, Point::new(caret.left() + 0.1, caret.top() + 1.0)),
            Some(DocPos::new(1))
        );
        assert!(adapter.is_dragging());
        adapter.mouse_up();
        assert!(!adapter.is_dragging());
    }

    #[test]
    fn scroll_clamps_and_caret_blinks() {
        let mut scroll = ScrollState {
            offset_y: 0.0,
            viewport_height: 100.0,
        };
        scroll.scroll_by(500.0, 300.0);
        assert_eq!(scroll.offset_y, 200.0);
        assert!(CaretBlink::default().visible_at(0));
        assert!(!CaretBlink::default().visible_at(500));
    }
}
