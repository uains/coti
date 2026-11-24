//! Application state, file IO, window state, and close policy.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use md_core::Size;
use md_editor::{Editor, Intent};
use md_layout::ResolvedStyle;
use md_markdown::serialize_document;
use md_render::RenderView;

use crate::config::AppConfig;

/// Whether a close request needs a save prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDecision {
    /// Document has no unsaved changes.
    Clean,
    /// Document has unsaved changes.
    PromptSave,
}

/// Current document and its file state.
pub struct AppState {
    editor: Editor,
    path: Option<PathBuf>,
    original_source: String,
    saved_modified: Option<SystemTime>,
    config: AppConfig,
}

impl AppState {
    /// Creates an empty untitled document.
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        let mut editor = Editor::new(md_document::Document::new());
        editor.mark_saved();
        Self {
            editor,
            path: None,
            original_source: String::new(),
            saved_modified: None,
            config,
        }
    }

    /// Opens a Markdown file.
    ///
    /// # Errors
    ///
    /// Returns an error when reading or parsing fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_config(path, AppConfig::default())
    }

    /// Opens a Markdown file with an existing config.
    ///
    /// # Errors
    ///
    /// Returns an error when reading or parsing fails.
    pub fn open_with_config(path: impl AsRef<Path>, mut config: AppConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let source =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let markdown =
            md_markdown::parse(&source).with_context(|| format!("parse {}", path.display()))?;
        let mut editor = Editor::new(markdown.into_document());
        editor.mark_saved();
        let saved_modified = fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        config.remember(path.clone());
        Ok(Self {
            editor,
            path: Some(path),
            original_source: source,
            saved_modified,
            config,
        })
    }

    /// Returns the editor.
    #[must_use]
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Returns mutable editor access.
    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    /// Returns the current path.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the window title.
    #[must_use]
    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map_or_else(
                || "未命名 — Coti".to_owned(),
                |name| format!("{name} — Coti"),
            )
    }

    /// Returns whether the document has unsaved edits.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.editor.is_modified()
    }

    /// Returns the config.
    #[must_use]
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Returns mutable config access.
    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// Returns the current serialized Markdown.
    #[must_use]
    pub fn serialize(&self) -> String {
        if self.editor.is_modified() {
            serialize_document(self.editor.document(), &self.original_source)
        } else {
            self.original_source.clone()
        }
    }

    /// Saves to the current path.
    ///
    /// # Errors
    ///
    /// Returns an error when no path is set or writing fails.
    pub fn save(&mut self) -> Result<()> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| anyhow!("document has no path; use save_as"))?;
        self.save_to(path)
    }

    /// Saves to a new path.
    ///
    /// # Errors
    ///
    /// Returns an error when writing fails.
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.path = Some(path.as_ref().to_path_buf());
        self.save()
    }

    /// Saves and updates the save point.
    ///
    /// # Errors
    ///
    /// Returns an error when writing fails.
    pub fn save_to(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        let content = self.serialize();
        atomic_write(&path, content.as_bytes())
            .with_context(|| format!("save {}", path.display()))?;
        self.editor.mark_saved();
        self.original_source = content;
        self.saved_modified = fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        self.path = Some(path.clone());
        self.config.remember(path);
        Ok(())
    }

    /// Returns whether the file changed on disk since open/save.
    #[must_use]
    pub fn external_change_detected(&self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        let current = fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        current != self.saved_modified
    }

    /// Returns the close decision.
    #[must_use]
    pub fn close_decision(&self) -> CloseDecision {
        if self.is_modified() {
            CloseDecision::PromptSave
        } else {
            CloseDecision::Clean
        }
    }

    /// Replaces the document with a new empty one.
    pub fn new_document(&mut self) {
        self.editor = Editor::new(md_document::Document::new());
        self.editor.mark_saved();
        self.path = None;
        self.original_source.clear();
        self.saved_modified = None;
    }

    /// Applies an editing intent.
    ///
    /// # Errors
    ///
    /// Returns an editor error when the intent fails.
    pub fn edit(&mut self, intent: Intent) -> md_editor::EditorResult<md_editor::RuleId> {
        self.editor.dispatch(intent)
    }
}

/// Atomic write: write a sibling temp file, fsync, then rename.
///
/// # Errors
///
/// Returns an I/O error when writing, syncing, or renaming fails.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let temp = path.with_extension(format!("coti-tmp-{suffix}"));
    let result = (|| {
        fs::write(&temp, bytes)?;
        fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
        match fs::rename(&temp, path) {
            Ok(()) => Ok(()),
            Err(rename_error) => {
                // Windows may reject replacing an existing file. Fall back to
                // a direct write after the temp file has been fully synced.
                if path.exists() {
                    fs::write(path, bytes)?;
                    let _ = fs::remove_file(&temp);
                    Ok(())
                } else {
                    Err(rename_error)
                }
            }
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Window state combining app state, layout, and render view.
pub struct AppWindow {
    /// Application state.
    pub state: AppState,
    /// Render view.
    pub view: RenderView,
    /// Viewport size.
    pub viewport: Size,
    layout_revision: u64,
    layout_width: f32,
}

impl AppWindow {
    /// Creates a window and lays out the current document.
    #[must_use]
    pub fn new(state: AppState, viewport: Size) -> Self {
        let view = Self::build_view(&state, viewport);
        Self {
            state,
            view,
            viewport,
            layout_revision: u64::MAX,
            layout_width: viewport.width,
        }
    }

    /// Rebuilds the render view after edits or resize.
    pub fn sync_view(&mut self) {
        self.view = Self::build_view(&self.state, self.viewport);
        self.layout_revision = u64::MAX;
    }

    /// Updates viewport size and re-syncs layout.
    pub fn resize(&mut self, viewport: Size) {
        self.viewport = viewport;
        self.sync_view();
    }

    /// Rebuilds layout using a real text measurer (for GPUI).
    pub fn relayout_with(&mut self, measurer: &mut dyn md_layout::TextMeasurer) {
        let revision = self.state.editor().document_revision();
        if self.layout_revision == revision && self.layout_width == self.viewport.width {
            return;
        }
        let style = ResolvedStyle {
            font_size: self.state.config().font_size,
            line_height: self.state.config().line_height,
            ..ResolvedStyle::default()
        };
        self.view.layout = md_layout::layout_with_measurer(
            self.state.editor().document(),
            md_layout::LayoutEnv::new(self.viewport, style),
            measurer,
        );
        self.view.scroll.clamp(self.view.layout.total_height());
        self.layout_revision = revision;
        self.layout_width = self.viewport.width;
    }

    fn build_view(state: &AppState, viewport: Size) -> RenderView {
        let theme = state.config().theme.render_theme();
        let style = ResolvedStyle {
            font_size: state.config().font_size,
            line_height: state.config().line_height,
            ..ResolvedStyle::default()
        };
        RenderView::sync(state.editor().document(), viewport, style, theme)
    }

    /// Scrolls so the caret is visible, or centers it in typewriter mode.
    pub fn scroll_to_caret(&mut self, caret: md_document::DocPos, typewriter: bool) {
        let Some(rect) = self.view.layout.caret_rect(caret) else {
            return;
        };
        if typewriter {
            self.view.scroll.offset_y = rect.top() - self.viewport.height * 0.5;
        } else {
            let height = self.viewport.height;
            if rect.top() < self.view.scroll.offset_y {
                self.view.scroll.offset_y = rect.top();
            } else if rect.bottom() > self.view.scroll.offset_y + height {
                self.view.scroll.offset_y = rect.bottom() - height;
            }
        }
        self.view.scroll.clamp(self.view.layout.total_height());
    }
}

/// Default config path under the user config directory.
#[must_use]
pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join("coti").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::{AppState, atomic_write, default_config_path};
    use crate::config::AppConfig;
    use md_editor::Intent;

    fn unique(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("coti-{name}-{stamp}.md"))
    }

    #[test]
    fn new_document_is_clean() {
        let state = AppState::new(AppConfig::default());
        assert!(!state.is_modified());
        assert!(state.path().is_none());
        assert_eq!(state.title(), "未命名 — Coti");
        assert!(default_config_path().ends_with("config.toml"));
    }

    #[test]
    fn edit_marks_modified_and_close_prompts() {
        let mut state = AppState::new(AppConfig::default());
        state.edit(Intent::InsertText("x".to_owned())).unwrap();
        assert!(state.is_modified());
        assert_eq!(state.close_decision(), super::CloseDecision::PromptSave);
    }

    #[test]
    fn atomic_write_replaces_content() {
        let path = unique("atomic");
        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let _ = std::fs::remove_file(path);
    }
}
