//! Real GPUI view with a custom `Element` renderer.

use gpui::ClipboardItem;
use gpui::{
    App, AppContext, Bounds, ClickEvent, Context, Corners, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, FontWeight, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement, Pixels, Render, RenderImage,
    ScrollDelta, ScrollEvent, SharedString, StatefulInteractiveElement, Style, Styled, TextAlign,
    TextRun, TitlebarOptions, UTF16Selection, Window, WindowBounds, WindowDecorations,
    WindowOptions, WindowTextSystem, div, fill, point, px, relative, rgba, size,
};
use gpui_platform::application;
use md_core::Size;
use md_layout::{LineMetrics, MeasureStyle, TextMeasurer};
use md_render::{Color, PaintCommand};
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

use crate::app::{AppState, AppWindow};
use crate::commands::{self, Command};
use crate::config::AppConfig;

const TITLEBAR_HEIGHT: f32 = 36.0;
use crate::features::{self, SearchState};

/// GPUI view backed by the headless application state.
pub struct CotiView {
    window: AppWindow,
    focus_handle: FocusHandle,
    dragging: bool,
    drag_anchor: Option<md_document::DocPos>,
    search_open: bool,
    search: SearchState,
    outline_open: bool,
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    menu_open: Option<usize>,
}

impl CotiView {
    /// Creates a view for an application state.
    #[must_use]
    pub fn new(state: AppState, viewport: Size, cx: &mut Context<Self>) -> Self {
        Self {
            window: AppWindow::new(state, viewport),
            focus_handle: cx.focus_handle(),
            dragging: false,
            drag_anchor: None,
            search_open: false,
            search: SearchState::default(),
            outline_open: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            menu_open: None,
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

    fn new_document(&mut self) {
        let config = self.window.state.config().clone();
        self.window = AppWindow::new(AppState::new(config), self.window.viewport);
    }

    fn open_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown"])
            .pick_file()
        else {
            return;
        };
        let config = self.window.state.config().clone();
        match AppState::open_with_config(&path, config) {
            Ok(state) => {
                self.window = AppWindow::new(state, self.window.viewport);
            }
            Err(error) => {
                tracing::warn!(target: "md_app", %error, "open failed");
            }
        }
    }

    fn save_as_dialog(&mut self) {
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown"])
            .set_file_name("untitled.md")
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("md");
        }
        if let Err(error) = self.window.state.save_as(&path) {
            tracing::warn!(target: "md_app", %error, "save failed");
        }
    }

    fn open_search(&mut self) {
        self.search_open = true;
        self.search = SearchState::default();
        self.outline_open = false;
        self.palette_open = false;
    }

    fn search_key(&mut self, key: &str, text: Option<&str>, cx: &mut Context<Self>) {
        match key {
            "escape" => {
                self.search_open = false;
            }
            "enter" => {
                self.search.next_match();
            }
            "backspace" => {
                self.search.query.pop();
                self.run_search();
            }
            _ => {
                if let Some(text) = text {
                    self.search.query.push_str(text);
                    self.run_search();
                }
            }
        }
        cx.notify();
    }

    fn run_search(&mut self) {
        let source = self.window.state.serialize();
        let query = self.search.query.clone();
        self.search.search(&source, &query, false);
    }

    fn palette_filtered(&self) -> Vec<Command> {
        features::filter_commands(&self.palette_query)
    }

    fn palette_key(
        &mut self,
        key: &str,
        text: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match key {
            "escape" => self.palette_open = false,
            "enter" => {
                let commands = self.palette_filtered();
                if let Some(command) = commands.get(self.palette_index).copied() {
                    self.palette_open = false;
                    self.execute_command(command, window, cx);
                }
            }
            "up" => self.palette_index = self.palette_index.saturating_sub(1),
            "down" => {
                let count = self.palette_filtered().len();
                if count > 0 {
                    self.palette_index = (self.palette_index + 1).min(count - 1);
                }
            }
            "backspace" => {
                self.palette_query.pop();
                self.palette_index = 0;
            }
            _ => {
                if let Some(text) = text {
                    self.palette_query.push_str(text);
                    self.palette_index = 0;
                }
            }
        }
        cx.notify();
    }

    fn execute_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::New => self.new_document(),
            Command::Open => self.open_dialog(),
            Command::Save => {
                if self.window.state.path().is_none() {
                    self.save_as_dialog();
                } else {
                    let _ = self.window.state.save();
                }
            }
            Command::SaveAs => self.save_as_dialog(),
            Command::Undo => {
                let _ = self.window.state.edit(md_editor::Intent::Undo);
            }
            Command::Redo => {
                let _ = self.window.state.edit(md_editor::Intent::Redo);
            }
            Command::Copy => {
                let text = self.window.state.editor().copy_text();
                if !text.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            Command::Paste => {
                if let Some(item) = cx.read_from_clipboard()
                    && let Some(text) = item.text()
                {
                    let _ = self.window.state.edit(md_editor::Intent::Paste(text));
                }
            }
            Command::ToggleTheme => {
                let next = self.window.state.config().theme.next();
                self.window.state.config_mut().theme = next;
            }
            Command::Quit => {
                window.remove_window();
                return;
            }
        }
        self.window.sync_view();
        cx.notify();
    }

    fn ime_text(&self) -> String {
        let document = self.window.state.editor().document();
        let mut parts = Vec::new();
        for leaf in document.index().leaf_order() {
            let text = match document.kind(*leaf) {
                Some(md_document::BlockKind::Paragraph(content))
                | Some(md_document::BlockKind::Heading { content, .. })
                | Some(md_document::BlockKind::TableCell { content }) => content.plain_text(),
                Some(md_document::BlockKind::CodeBlock { text, .. })
                | Some(md_document::BlockKind::HtmlBlock(text))
                | Some(md_document::BlockKind::MathBlock(text))
                | Some(md_document::BlockKind::FrontMatter(text)) => text.as_str().to_owned(),
                _ => String::new(),
            };
            parts.push(text);
        }
        let mut text = parts.join("\n");
        if let Some(composition) = self.window.state.editor().composition()
            && let Some(selection) = self.window.state.editor().selection().primary_selection()
        {
            let caret = self.doc_to_utf16(selection.head.pos);
            let byte = Self::utf16_to_byte(&text, caret);
            text.insert_str(byte, &composition.text);
        }
        text
    }

    fn doc_to_utf16(&self, pos: md_document::DocPos) -> usize {
        let document = self.window.state.editor().document();
        let Some(hit) = document.index().leaf_at(pos.get()) else {
            return 0;
        };
        let mut utf16 = 0;
        for (index, leaf) in document.index().leaf_order().iter().enumerate() {
            if index > 0 {
                utf16 += 1;
            }
            let text = self.leaf_text(*leaf);
            if index == hit.leaf_index {
                return utf16 + Self::utf16_len(&text[..hit.offset.min(text.len())]);
            }
            utf16 += Self::utf16_len(&text);
        }
        utf16
    }

    fn utf16_to_doc(&self, utf16_offset: usize) -> md_document::DocPos {
        let document = self.window.state.editor().document();
        let mut consumed = 0;
        for (index, leaf) in document.index().leaf_order().iter().enumerate() {
            if index > 0 {
                if utf16_offset <= consumed {
                    return md_document::DocPos::new(
                        document.index().leaf_start(index).unwrap_or(0),
                    );
                }
                consumed += 1;
            }
            let text = self.leaf_text(*leaf);
            let length = Self::utf16_len(&text);
            if utf16_offset <= consumed + length {
                let byte = Self::utf16_to_byte(&text, utf16_offset - consumed);
                let start = document.index().leaf_start(index).unwrap_or(0);
                return md_document::DocPos::new(start + byte);
            }
            consumed += length;
        }
        md_document::DocPos::new(document.index().doc_length())
    }

    fn leaf_text(&self, leaf: md_document::BlockId) -> String {
        let document = self.window.state.editor().document();
        match document.kind(leaf) {
            Some(md_document::BlockKind::Paragraph(content))
            | Some(md_document::BlockKind::Heading { content, .. })
            | Some(md_document::BlockKind::TableCell { content }) => content.plain_text(),
            Some(md_document::BlockKind::CodeBlock { text, .. })
            | Some(md_document::BlockKind::HtmlBlock(text))
            | Some(md_document::BlockKind::MathBlock(text))
            | Some(md_document::BlockKind::FrontMatter(text)) => text.as_str().to_owned(),
            _ => String::new(),
        }
    }

    fn utf16_len(text: &str) -> usize {
        text.encode_utf16().count()
    }

    fn utf16_to_byte(text: &str, units: usize) -> usize {
        let mut consumed = 0;
        for (byte, character) in text.char_indices() {
            if consumed + character.len_utf16() > units {
                return byte;
            }
            consumed += character.len_utf16();
        }
        text.len()
    }

    fn select_utf16(&mut self, range: std::ops::Range<usize>) {
        let start = self.utf16_to_doc(range.start);
        let end = self.utf16_to_doc(range.end);
        let selection = md_document::SelectionSet::new(md_document::Selection::range(
            md_document::Anchor::new(start, md_document::Affinity::Upstream),
            md_document::Anchor::new(end, md_document::Affinity::Downstream),
        ));
        let _ = self.window.state.editor_mut().set_selection(selection);
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => f32::from(delta.y),
            ScrollDelta::Lines(delta) => delta.y * self.window.view.theme.line_height,
        };
        let height = self.window.view.layout.total_height();
        self.window.view.scroll.scroll_by(delta, height);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        if self.search_open {
            self.search_key(key, event.keystroke.key_char.as_deref(), cx);
            return;
        }
        if self.palette_open {
            self.palette_key(key, event.keystroke.key_char.as_deref(), window, cx);
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("f") {
            self.open_search();
            cx.notify();
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("k") {
            self.palette_open = true;
            self.palette_query.clear();
            self.palette_index = 0;
            cx.notify();
            return;
        }
        if modifiers.control && modifiers.shift && key.eq_ignore_ascii_case("o") {
            self.outline_open = !self.outline_open;
            cx.notify();
            return;
        }
        if modifiers.control && modifiers.shift && key.eq_ignore_ascii_case("t") {
            let next = self.window.state.config().theme.next();
            self.window.state.config_mut().theme = next;
            self.window.sync_view();
            cx.notify();
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("o") {
            self.open_dialog();
            cx.notify();
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("n") {
            self.new_document();
            cx.notify();
            return;
        }
        if modifiers.control && key.eq_ignore_ascii_case("s") {
            if modifiers.shift || self.window.state.path().is_none() {
                self.save_as_dialog();
            } else {
                let _ = self.window.state.save();
            }
            cx.notify();
            return;
        }
        let intent = if modifiers.control && key.eq_ignore_ascii_case("z") {
            if modifiers.shift {
                md_editor::Intent::Redo
            } else {
                md_editor::Intent::Undo
            }
        } else if event.keystroke.key_char.is_some() {
            return;
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
            (f32::from(event.position.y) - TITLEBAR_HEIGHT) + self.window.view.scroll.offset_y,
        );
        if let Some(position) = self.window.view.layout.position_at_point(point) {
            self.dragging = true;
            self.drag_anchor = Some(position);
            let selection = md_document::SelectionSet::new(md_document::Selection::collapsed(
                position,
                md_document::Affinity::Downstream,
            ));
            let _ = self.window.state.editor_mut().set_selection(selection);
            self.window.sync_view();
            cx.notify();
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging {
            return;
        }
        let point = md_core::Point::new(
            f32::from(event.position.x),
            (f32::from(event.position.y) - TITLEBAR_HEIGHT) + self.window.view.scroll.offset_y,
        );
        let Some(position) = self.window.view.layout.position_at_point(point) else {
            return;
        };
        let anchor = self.drag_anchor.unwrap_or(position);
        let (start, end) = if anchor <= position {
            (anchor, position)
        } else {
            (position, anchor)
        };
        let selection = md_document::SelectionSet::new(md_document::Selection::range(
            md_document::Anchor::new(start, md_document::Affinity::Upstream),
            md_document::Anchor::new(end, md_document::Affinity::Downstream),
        ));
        if self
            .window
            .state
            .editor_mut()
            .set_selection(selection)
            .is_ok()
        {
            cx.notify();
        }
    }

    fn on_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.dragging = false;
        self.drag_anchor = None;
    }
}

impl Render for CotiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
        let font = window.text_style().font();
        let mut measurer = SysMeasurer {
            sys: window.text_system(),
            font,
        };
        self.window.relayout_with(&mut measurer);
        window.set_window_title(&self.window.state.title());
        let background = rgba(self.window.view.theme.background.0);
        let surface = rgba(self.window.view.theme.surface.0);
        let border = rgba(self.window.view.theme.border.0);
        let muted = rgba(self.window.view.theme.muted.0);
        let title = self.window.state.title();

        let minimize = div()
            .id("window-minimize")
            .h_full()
            .w(px(44.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(muted)
            .hover(|style| style.bg(rgba(0x8080_8022)))
            .on_click(cx.listener(|_, _, window, _| window.minimize_window()))
            .child("—");
        let maximize = div()
            .id("window-maximize")
            .h_full()
            .w(px(44.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(muted)
            .hover(|style| style.bg(rgba(0x8080_8022)))
            .on_click(cx.listener(|_, _, window, _| window.zoom_window()))
            .child("□");
        let close = div()
            .id("window-close")
            .h_full()
            .w(px(44.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(muted)
            .hover(|style| style.bg(rgba(0xE811_2322)))
            .on_click(cx.listener(|_, _, window, _| window.remove_window()))
            .child("×");
        let drag_area = div().id("titlebar-drag").flex_1().h_full().on_mouse_down(
            MouseButton::Left,
            cx.listener(|_, _, window, _| window.start_window_move()),
        );

        let menu_items = commands::menus();
        let mut titlebar = div()
            .h(px(36.0))
            .w_full()
            .flex()
            .items_center()
            .bg(surface)
            .border_b_1()
            .border_color(border);
        for (index, menu) in menu_items.iter().enumerate() {
            let active = self.menu_open == Some(index);
            let mut button = div()
                .id(ElementId::Name(format!("menu-{index}").into()))
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .text_size(px(12.0))
                .text_color(muted)
                .cursor_pointer()
                .hover(|style| style.bg(rgba(0x8080_8022)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.menu_open = if this.menu_open == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                    cx.notify();
                }))
                .child(menu.title);
            if active {
                button = button.bg(rgba(0x8080_8022));
            }
            titlebar = titlebar.child(button);
        }
        titlebar = titlebar
            .child(
                div()
                    .px_2()
                    .text_size(px(13.0))
                    .text_color(muted)
                    .child(title),
            )
            .child(drag_area)
            .child(toolbar_label(
                "搜索",
                cx.listener(|this, _, _, cx| {
                    this.open_search();
                    cx.notify();
                }),
            ))
            .child(toolbar_label(
                "大纲",
                cx.listener(|this, _, _, cx| {
                    this.outline_open = !this.outline_open;
                    cx.notify();
                }),
            ))
            .child(toolbar_label(
                "命令",
                cx.listener(|this, _, _, cx| {
                    this.palette_open = true;
                    this.palette_query.clear();
                    this.palette_index = 0;
                    cx.notify();
                }),
            ))
            .child(toolbar_label(
                "主题",
                cx.listener(|this, _, _, cx| {
                    let next = this.window.state.config().theme.next();
                    this.window.state.config_mut().theme = next;
                    this.window.sync_view();
                    cx.notify();
                }),
            ))
            .child(minimize)
            .child(maximize)
            .child(close);

        let body = div()
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(DocumentElement { view: cx.entity() });

        let mut content_row = div().flex_1().w_full().flex().overflow_hidden().child(body);
        if self.outline_open {
            let items = features::build_outline(self.window.state.editor().document());
            let mut panel = div()
                .h_full()
                .w(px(240.0))
                .flex()
                .flex_col()
                .p_2()
                .gap_1()
                .bg(surface)
                .border_r_1()
                .border_color(border)
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child("文档大纲"),
                );
            for (index, item) in items.into_iter().enumerate() {
                let pos = item.pos;
                panel = panel.child(
                    div()
                        .id(ElementId::Name(format!("outline-{index}").into()))
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0x8080_8022)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let selection =
                                md_document::SelectionSet::new(md_document::Selection::collapsed(
                                    pos,
                                    md_document::Affinity::Downstream,
                                ));
                            let _ = this.window.state.editor_mut().set_selection(selection);
                            this.window.sync_view();
                            cx.notify();
                        }))
                        .child(format!(
                            "{}{}",
                            "  ".repeat(item.level.saturating_sub(1) as usize),
                            item.title
                        )),
                );
            }
            content_row = content_row.child(panel);
        }

        let status = features::status_bar(
            self.window.state.editor().document(),
            self.window
                .state
                .editor()
                .selection()
                .primary_selection()
                .map_or(md_document::DocPos::ZERO, |selection| selection.head.pos),
        );
        let status_bar = div()
            .h(px(26.0))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(surface)
            .border_t_1()
            .border_color(border)
            .text_size(px(11.0))
            .text_color(muted)
            .child(format!(
                "{} · {} 字 · {} 块 · {} 分钟 · pos {}",
                if self.window.state.is_modified() {
                    "未保存"
                } else {
                    "已保存"
                },
                status.characters,
                status.blocks,
                status.reading_minutes,
                status.position.get()
            ))
            .child(format!(
                "Coti · {}",
                self.window.state.config().theme.token()
            ));

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(titlebar)
            .child(content_row)
            .child(status_bar);

        if let Some(index) = self.menu_open
            && let Some(menu) = menu_items.get(index)
        {
            let mut panel = div()
                .absolute()
                .top(px(36.0))
                .left(px(8.0 + index as f32 * 52.0))
                .w(px(190.0))
                .flex()
                .flex_col()
                .p_1()
                .rounded(px(6.0))
                .bg(surface)
                .border_1()
                .border_color(border);
            for item in &menu.items {
                let command = item.command;
                panel = panel.child(
                    div()
                        .id(ElementId::Name(
                            format!("menu-item-{}", item.command.id()).into(),
                        ))
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0x8080_8022)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.menu_open = None;
                            this.execute_command(command, window, cx);
                        }))
                        .child(item.label),
                );
            }
            root = root.child(panel);
        }

        if self.search_open {
            let count = if self.search.matches.is_empty() {
                "0/0".to_owned()
            } else {
                format!(
                    "{}/{}",
                    self.search.current.map_or(0, |index| index + 1),
                    self.search.matches.len()
                )
            };
            root = root.child(
                div()
                    .absolute()
                    .top(px(44.0))
                    .right(px(16.0))
                    .w(px(360.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(px(8.0))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .child(div().flex_1().text_size(px(13.0)).text_color(muted).child(
                        if self.search.query.is_empty() {
                            "搜索文档…".to_owned()
                        } else {
                            self.search.query.clone()
                        },
                    ))
                    .child(div().text_size(px(11.0)).text_color(muted).child(count))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(muted)
                            .child("Enter 下一个 · Esc 关闭"),
                    ),
            );
        }

        if self.palette_open {
            let commands = self.palette_filtered();
            let mut panel = div()
                .w(px(460.0))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded(px(8.0))
                .bg(surface)
                .border_1()
                .border_color(border)
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_size(px(13.0))
                        .text_color(muted)
                        .child(if self.palette_query.is_empty() {
                            "输入命令…".to_owned()
                        } else {
                            self.palette_query.clone()
                        }),
                );
            for (index, command) in commands.into_iter().take(8).enumerate() {
                let mut row = div()
                    .id(ElementId::Name(format!("palette-{}", command.id()).into()))
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .text_color(muted)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.palette_open = false;
                        this.execute_command(command, window, cx);
                    }))
                    .child(format!("{}   {}", command.id(), command.shortcut()));
                if index == self.palette_index {
                    row = row.bg(rgba(0x8080_8022));
                }
                panel = panel.child(row);
            }
            root = root.child(
                div()
                    .absolute()
                    .top(px(70.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .flex()
                    .justify_center()
                    .child(panel),
            );
        }

        root
    }
}

impl EntityInputHandler for CotiView {
    fn text_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.ime_text();
        let start = Self::utf16_to_byte(&text, range.start);
        let end = Self::utf16_to_byte(&text, range.end);
        (start <= end).then(|| text[start..end].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.window.state.editor().selection().primary_selection()?;
        let anchor = self.doc_to_utf16(selection.anchor.pos);
        let head = self.doc_to_utf16(selection.head.pos);
        Some(UTF16Selection {
            range: anchor.min(head)..anchor.max(head),
            reversed: head < anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        let composition = self.window.state.editor().composition()?;
        let selection = self.window.state.editor().selection().primary_selection()?;
        let caret = self.doc_to_utf16(selection.head.pos);
        Some(caret..caret + Self::utf16_len(&composition.text))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.window.state.edit(md_editor::Intent::ImeCancel);
        self.window.sync_view();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search_open {
            self.search.query.push_str(text);
            self.run_search();
            window.notify_text_input_changed(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.palette_open {
            self.palette_query.push_str(text);
            self.palette_index = 0;
            window.notify_text_input_changed(&self.focus_handle);
            cx.notify();
            return;
        }
        if let Some(range) = range {
            self.select_utf16(range);
        }
        let intent = if self.window.state.editor().composition().is_some() {
            md_editor::Intent::ImeCommit(text.to_owned())
        } else {
            md_editor::Intent::InsertText(text.to_owned())
        };
        let _ = self.window.state.edit(intent);
        self.window.sync_view();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search_open || self.palette_open {
            self.replace_text_in_range(range, new_text, window, cx);
            return;
        }
        if let Some(range) = range {
            self.select_utf16(range);
        }
        let intent = if new_text.is_empty() {
            md_editor::Intent::ImeCancel
        } else {
            md_editor::Intent::ImeSetMarked(new_text.to_owned())
        };
        let _ = self.window.state.edit(intent);
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let _ = range_utf16;
        let caret = self.window.view.layout.caret_rect(
            self.window
                .state
                .editor()
                .selection()
                .primary_selection()?
                .head
                .pos,
        )?;
        Some(Bounds {
            origin: point(
                element_bounds.origin.x + px(caret.left()),
                element_bounds.origin.y + px(caret.top()),
            ),
            size: size(px(caret.width()), px(caret.height())),
        })
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        let start = self.utf16_to_doc(range_utf16.start);
        let end = self.utf16_to_doc(range_utf16.end);
        self.window
            .view
            .layout
            .rects_for_range(start, end)
            .into_iter()
            .map(|rect| Bounds {
                origin: point(
                    element_bounds.origin.x + px(rect.left()),
                    element_bounds.origin.y + px(rect.top()),
                ),
                size: size(px(rect.width()), px(rect.height())),
            })
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let position = self
            .window
            .view
            .layout
            .position_at_point(md_core::Point::new(f32::from(point.x), f32::from(point.y)))?;
        Some(self.doc_to_utf16(position))
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(Self::utf16_len(&self.ime_text()))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_utf16(range_utf16);
        cx.notify();
    }
}

/// Custom GPUI element that paints the layout's paint plan directly.
struct DocumentElement {
    view: Entity<CotiView>,
}

enum PreparedItem {
    Quad(PaintQuad),
    Image {
        bounds: Bounds<Pixels>,
        image: Arc<RenderImage>,
    },
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
                PaintCommand::Image { rect, source } => {
                    let image_bounds = paint_bounds(bounds, rect, scroll);
                    let base = view.window.state.path().and_then(Path::parent);
                    if let Some(image) = load_local_image(base, &source) {
                        items.push(PreparedItem::Image {
                            bounds: image_bounds,
                            image,
                        });
                    } else {
                        items.push(PreparedItem::Quad(fill(
                            image_bounds,
                            rgba(view.window.view.theme.border.0),
                        )));
                    }
                }
                PaintCommand::Text {
                    rect,
                    text,
                    color,
                    font_size,
                    font_weight,
                    mono,
                } => {
                    let mut font = if mono {
                        gpui::font("Consolas")
                    } else {
                        text_style.font()
                    };
                    font.weight = FontWeight(font_weight);
                    let run = TextRun {
                        len: text.len(),
                        font,
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
        let composition = view
            .window
            .state
            .editor()
            .composition()
            .map(|c| c.text.clone());
        if let Some(text) = composition
            && !text.is_empty()
            && let Some(selection) = view.window.state.editor().selection().primary_selection()
            && let Some(caret) = view.window.view.layout.caret_rect(selection.head.pos)
        {
            let run = TextRun {
                len: text.len(),
                font: text_style.font(),
                color: text_color(view.window.view.theme.accent),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window.text_system().shape_line(
                SharedString::from(text),
                px(view.window.view.theme.font_size),
                &[run],
                None,
            );
            items.push(PreparedItem::Text {
                line: Box::new(line),
                origin: point(
                    bounds.left() + px(caret.left()),
                    bounds.top() + px(caret.top() - scroll),
                ),
                line_height: px(caret.height()),
            });
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
        let focus = self.view.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(_bounds, self.view.clone()),
            cx,
        );
        for item in &prepaint.items {
            match item {
                PreparedItem::Quad(quad) => window.paint_quad(quad.clone()),
                PreparedItem::Image { bounds, image } => {
                    let _ = window.paint_image(
                        *bounds,
                        *bounds,
                        Corners::default(),
                        image.clone(),
                        0,
                        false,
                    );
                }
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

fn toolbar_label(
    label: &'static str,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("toolbar-{label}").into()))
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(0x8080_8022)))
        .on_click(listener)
        .child(label)
}

thread_local! {
    static IMAGE_CACHE: RefCell<HashMap<PathBuf, Option<Arc<RenderImage>>>> =
        RefCell::new(HashMap::new());
}

fn load_local_image(base: Option<&Path>, source: &str) -> Option<Arc<RenderImage>> {
    if source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("data:")
    {
        return None;
    }
    let path = PathBuf::from(source);
    let path = if path.is_absolute() {
        path
    } else if let Some(base) = base {
        base.join(path)
    } else {
        path
    };
    let path = path.canonicalize().unwrap_or(path);
    if let Some(cached) = IMAGE_CACHE.with(|cache| cache.borrow().get(&path).cloned()) {
        return cached;
    }
    let loaded = image::ImageReader::open(&path)
        .ok()
        .and_then(|reader| reader.decode().ok())
        .map(|image| {
            let frame = image::Frame::new(image.to_rgba8());
            Arc::new(RenderImage::new(vec![frame]))
        });
    IMAGE_CACHE.with(|cache| {
        cache.borrow_mut().insert(path, loaded.clone());
    });
    loaded
}

fn text_color(color: Color) -> gpui::Hsla {
    rgba(color.0).into()
}

struct SysMeasurer<'a> {
    sys: &'a WindowTextSystem,
    font: gpui::Font,
}

impl SysMeasurer<'_> {
    fn font_for(&self, style: &MeasureStyle) -> gpui::Font {
        let mut font = if style.mono {
            gpui::font("Consolas")
        } else {
            self.font.clone()
        };
        font.weight = FontWeight(style.weight);
        font
    }

    fn run(&self, len: usize, style: &MeasureStyle) -> TextRun {
        TextRun {
            len,
            font: self.font_for(style),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }
}

impl TextMeasurer for SysMeasurer<'_> {
    fn advance(&self, text: &str, style: &MeasureStyle) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        if text.contains(['\n', '\r']) {
            return self
                .sys
                .layout_line(" ", px(style.font_size), &[self.run(1, style)], None)
                .width
                .into();
        }
        let run = self.run(text.len(), style);
        f32::from(
            self.sys
                .layout_line(text, px(style.font_size), &[run], None)
                .width,
        )
    }

    fn cluster_advances(&self, text: &str, style: &MeasureStyle) -> Vec<f32> {
        if text.is_empty() {
            return Vec::new();
        }
        if text.contains(['\n', '\r']) {
            return text
                .graphemes(true)
                .map(|grapheme| self.advance(grapheme, style))
                .collect();
        }
        let run = self.run(text.len(), style);
        let layout = self
            .sys
            .layout_line(text, px(style.font_size), &[run], None);
        let mut boundaries: Vec<usize> =
            text.grapheme_indices(true).map(|(byte, _)| byte).collect();
        boundaries.push(text.len());
        boundaries
            .windows(2)
            .map(|pair| {
                let start = f32::from(layout.x_for_index(pair[0]));
                let end = f32::from(layout.x_for_index(pair[1]));
                (end - start).max(0.0)
            })
            .collect()
    }

    fn atom_advance(&self, kind: &md_document::AtomKind, style: &MeasureStyle) -> f32 {
        match kind {
            md_document::AtomKind::HardBreak | md_document::AtomKind::SoftBreak => 0.0,
            md_document::AtomKind::Image(_) => style.font_size * 4.0,
            md_document::AtomKind::InlineMath(source) => self.advance(source.as_str(), style),
            md_document::AtomKind::FootnoteRef(label) => self.advance(label, style) * 0.6,
        }
    }

    fn line_metrics(&self, style: &MeasureStyle) -> LineMetrics {
        let layout = self
            .sys
            .layout_line("x", px(style.font_size), &[self.run(1, style)], None);
        LineMetrics::new(
            f32::from(layout.ascent),
            f32::from(layout.descent),
            style.line_height,
        )
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
        let bounds = Bounds {
            origin: point(px(40.0), px(40.0)),
            size: size(px(1000.0), px(600.0)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_decorations: Some(WindowDecorations::Client),
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: None,
            }),
            ..Default::default()
        };
        let path = path.clone();
        cx.open_window(options, |_, cx| {
            cx.new(|cx| CotiView::from_path(path, Size::new(1000.0, 564.0), cx))
        })
        .expect("create Coti window");
        cx.activate(true);
    });
    Ok(())
}
