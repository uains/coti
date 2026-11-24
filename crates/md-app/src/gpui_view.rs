//! Real GPUI view and window bootstrap.

use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};
use gpui_platform::application;
use md_core::Size;
use md_render::PaintCommand;

use crate::app::{AppState, AppWindow};
use crate::config::AppConfig;

/// GPUI view backed by the headless application state.
pub struct CotiView {
    window: AppWindow,
    focus_handle: FocusHandle,
}

impl CotiView {
    /// Creates a view for an application state.
    #[must_use]
    pub fn new(state: AppState, viewport: Size, cx: &mut Context<Self>) -> Self {
        Self {
            window: AppWindow::new(state, viewport),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Creates a view from a config.
    #[must_use]
    pub fn from_config(config: AppConfig, viewport: Size, cx: &mut Context<Self>) -> Self {
        Self::new(AppState::new(config), viewport, cx)
    }

    /// Opens a Markdown file or falls back to an empty document.
    #[must_use]
    pub fn from_path(
        path: Option<std::path::PathBuf>,
        viewport: Size,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = path.and_then(|path| match AppState::open(path) {
            Ok(state) => Some(state),
            Err(error) => {
                tracing::warn!(target: "md_app", %error, "could not open document");
                None
            }
        });
        Self::new(
            state.unwrap_or_else(|| AppState::new(AppConfig::default())),
            viewport,
            cx,
        )
    }

    /// Returns the app state.
    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.window.state
    }

    /// Returns mutable app state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.window.state
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        if modifiers.control && key.eq_ignore_ascii_case("s") {
            let _ = self.window.state.save();
            return;
        }
        let intent = if modifiers.control && key.eq_ignore_ascii_case("z") {
            if modifiers.shift {
                md_editor::Intent::Redo
            } else {
                md_editor::Intent::Undo
            }
        } else if let Some(character) = &event.keystroke.key_char {
            md_editor::Intent::InsertText(character.clone())
        } else {
            match key {
                "backspace" => md_editor::Intent::DeleteBackward,
                "delete" => md_editor::Intent::DeleteForward,
                "enter" => md_editor::Intent::InsertNewline,
                "tab" => md_editor::Intent::ChangeIndent {
                    outdent: modifiers.shift,
                },
                "escape" => md_editor::Intent::ImeCancel,
                "left" => md_editor::Intent::Move {
                    direction: md_editor::MoveDirection::Left,
                    extend: modifiers.shift,
                },
                "right" => md_editor::Intent::Move {
                    direction: md_editor::MoveDirection::Right,
                    extend: modifiers.shift,
                },
                "home" => md_editor::Intent::Move {
                    direction: md_editor::MoveDirection::DocStart,
                    extend: modifiers.shift,
                },
                "end" => md_editor::Intent::Move {
                    direction: md_editor::MoveDirection::DocEnd,
                    extend: modifiers.shift,
                },
                _ => return,
            }
        };
        if self.window.state.edit(intent).is_ok() {
            self.window.sync_view();
            cx.notify();
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let point = md_core::Point::new(
            f32::from(event.position.x),
            f32::from(event.position.y) + self.window.view.scroll.offset_y,
        );
        if let Some(position) = self.window.view.layout.position_at_point(point) {
            let selection = md_document::SelectionSet::new(md_document::Selection::collapsed(
                position,
                md_document::Affinity::Downstream,
            ));
            let _ = self.window.state.editor_mut().set_selection(selection);
            self.window.sync_view();
            cx.notify();
        }
    }
}

fn gpui_color(color: md_render::Color) -> gpui::Rgba {
    rgb((color.0 >> 8) & 0x00FF_FFFF)
}

impl Render for CotiView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection = self.window.state.editor().selection().clone();
        let caret = selection
            .primary_selection()
            .map(|selection| selection.head.pos);
        let plan = self
            .window
            .view
            .paint(self.window.state.editor().document(), &selection, caret);
        let theme = self.window.view.theme;
        let scroll = self.window.view.scroll.offset_y;

        let mut canvas = div()
            .absolute()
            .left(px(0.0))
            .top(px(-scroll))
            .w_full()
            .h(px(self.window.view.layout.total_height().max(1.0)));

        for item in plan.items {
            match item.command {
                PaintCommand::FillRect { rect, color } => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(rect.left()))
                            .top(px(rect.top()))
                            .w(px(rect.width().max(1.0)))
                            .h(px(rect.height().max(1.0)))
                            .bg(gpui_color(color)),
                    );
                }
                PaintCommand::Text {
                    rect,
                    text,
                    color,
                    font_size,
                } => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(rect.left()))
                            .top(px(rect.top()))
                            .w(px(rect.width().max(1.0)))
                            .h(px(rect.height().max(1.0)))
                            .text_size(px(font_size))
                            .text_color(gpui_color(color))
                            .child(text),
                    );
                }
                PaintCommand::Caret { rect, color } => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(rect.left()))
                            .top(px(rect.top()))
                            .w(px(rect.width().max(1.0)))
                            .h(px(rect.height().max(1.0)))
                            .bg(gpui_color(color)),
                    );
                }
                PaintCommand::Image { rect, source } => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(rect.left()))
                            .top(px(rect.top()))
                            .w(px(rect.width().max(1.0)))
                            .h(px(rect.height().max(1.0)))
                            .border_1()
                            .border_color(gpui_color(theme.border))
                            .child(source),
                    );
                }
            }
        }

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(gpui_color(theme.background))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .child(canvas)
    }
}

/// Runs the GPUI application window.
///
/// # Errors
///
/// Returns an error if the platform application cannot start.
pub fn run_gui() -> anyhow::Result<()> {
    run_gui_with_path(None)
}

/// Runs the GPUI window with an optional document path.
///
/// # Errors
///
/// Returns an error if the platform application cannot start.
pub fn run_gui_with_path(path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let path = path.clone();
        cx.open_window(options, |_, cx| {
            cx.new(|cx| CotiView::from_path(path, Size::new(1000.0, 760.0), cx))
        })
        .expect("create Coti window");
        cx.activate(true);
    });
    Ok(())
}
