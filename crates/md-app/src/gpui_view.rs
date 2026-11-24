//! Real GPUI view with a custom `Element` renderer.

use gpui::{
    App, AppContext, Bounds, Context, Element, ElementId, Entity, FocusHandle, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, PaintQuad, ParentElement, Pixels, Render, SharedString, Style, Styled,
    TextAlign, TextRun, Window, WindowBounds, WindowOptions, div, fill, point, px, relative, rgba,
    size,
};
use gpui_platform::application;
use md_core::Size;
use md_render::{Color, PaintCommand};
use std::panic;

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

impl Render for CotiView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .child(DocumentElement { view: cx.entity() })
    }
}

/// Custom GPUI element that paints the layout's paint plan directly.
struct DocumentElement {
    view: Entity<CotiView>,
}

enum PreparedItem {
    Quad(PaintQuad),
    Text {
        line: Box<gpui::ShapedLine>,
        origin: gpui::Point<Pixels>,
        line_height: Pixels,
    },
}

struct PreparedDocument {
    items: Vec<PreparedItem>,
}

impl Element for DocumentElement {
    type RequestLayoutState = ();
    type PrepaintState = PreparedDocument;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("coti-document".into()))
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let height = self
            .view
            .read(cx)
            .window
            .view
            .layout
            .total_height()
            .max(1.0);
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = px(height).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let view = self.view.read(cx);
        let selection = view.window.state.editor().selection().clone();
        let caret = selection
            .primary_selection()
            .map(|selection| selection.head.pos);
        let plan = view
            .window
            .view
            .paint(view.window.state.editor().document(), &selection, caret);
        let scroll = view.window.view.scroll.offset_y;
        let text_style = window.text_style();
        let mut items = Vec::new();
        items.push(PreparedItem::Quad(fill(
            bounds,
            rgba(view.window.view.theme.background.0),
        )));
        for item in plan.items {
            match item.command {
                PaintCommand::FillRect { rect, color } => {
                    items.push(PreparedItem::Quad(fill(
                        paint_bounds(bounds, rect, scroll),
                        rgba(color.0),
                    )));
                }
                PaintCommand::Caret { rect, color } => {
                    items.push(PreparedItem::Quad(fill(
                        paint_bounds(bounds, rect, scroll),
                        rgba(color.0),
                    )));
                }
                PaintCommand::Image { rect, .. } => {
                    items.push(PreparedItem::Quad(fill(
                        paint_bounds(bounds, rect, scroll),
                        rgba(view.window.view.theme.border.0),
                    )));
                }
                PaintCommand::Text {
                    rect,
                    text,
                    color,
                    font_size,
                } => {
                    let run = TextRun {
                        len: text.len(),
                        font: text_style.font(),
                        color: text_color(color),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = window.text_system().shape_line(
                        SharedString::from(text),
                        px(font_size),
                        &[run],
                        None,
                    );
                    items.push(PreparedItem::Text {
                        line: Box::new(line),
                        origin: point(
                            bounds.left() + px(rect.left()),
                            bounds.top() + px(rect.top() - scroll),
                        ),
                        line_height: px(rect.height()),
                    });
                }
            }
        }
        PreparedDocument { items }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for item in &prepaint.items {
            match item {
                PreparedItem::Quad(quad) => window.paint_quad(quad.clone()),
                PreparedItem::Text {
                    line,
                    origin,
                    line_height,
                } => {
                    let _ = line.as_ref().paint(
                        *origin,
                        *line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
        }
    }
}

fn paint_bounds(bounds: Bounds<Pixels>, rect: md_core::Rect, scroll: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(
            bounds.left() + px(rect.left()),
            bounds.top() + px(rect.top() - scroll),
        ),
        size(px(rect.width().max(1.0)), px(rect.height().max(1.0))),
    )
}

fn text_color(color: Color) -> gpui::Hsla {
    rgba(color.0).into()
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
