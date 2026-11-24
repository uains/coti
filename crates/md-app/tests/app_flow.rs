//! End-to-end app state, file IO, and window flow tests.

use std::path::PathBuf;

use md_app::app::AppState;
use md_app::config::{AppConfig, ThemeKind};
use md_app::{AppWindow, CloseDecision};
use md_core::Size;
use md_editor::{Intent, MoveDirection};

fn temp_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("coti-flow-{name}-{stamp}.md"))
}

#[test]
fn open_edit_save_reopen_keeps_content() {
    let path = temp_path("roundtrip");
    std::fs::write(&path, "# Hello\n\nbody\n").unwrap();

    let mut state = AppState::open(&path).unwrap();
    assert!(!state.is_modified());
    state
        .editor_mut()
        .dispatch(Intent::Move {
            direction: MoveDirection::DocEnd,
            extend: false,
        })
        .unwrap();
    state
        .editor_mut()
        .dispatch(Intent::InsertText(" tail".to_owned()))
        .unwrap();
    assert!(state.is_modified());
    assert_eq!(state.close_decision(), CloseDecision::PromptSave);
    state.save().unwrap();
    assert!(!state.is_modified());

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("tail"));

    let reopened = AppState::open(&path).unwrap();
    assert!(!reopened.is_modified());
    assert!(reopened.serialize().contains("tail"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn save_as_updates_path_and_recent_files() {
    let source = temp_path("source");
    let target = temp_path("target");
    std::fs::write(&source, "alpha\n").unwrap();
    let mut state = AppState::open(&source).unwrap();
    state
        .editor_mut()
        .dispatch(Intent::InsertText("beta".to_owned()))
        .unwrap();
    state.save_as(&target).unwrap();
    assert_eq!(state.path(), Some(target.as_path()));
    assert!(state.config().recent_files.contains(&target));
    assert!(std::fs::read_to_string(&target).unwrap().contains("beta"));
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(target);
}

#[test]
fn external_change_is_detected_after_save() {
    let path = temp_path("external");
    std::fs::write(&path, "one").unwrap();
    let state = AppState::open(&path).unwrap();
    assert!(!state.external_change_detected());
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "two").unwrap();
    assert!(state.external_change_detected());
    let _ = std::fs::remove_file(path);
}

#[test]
fn config_persists_and_config_path_round_trips() {
    let path = temp_path("config");
    let mut config = AppConfig {
        theme: ThemeKind::Paper,
        ..AppConfig::default()
    };
    config.remember(PathBuf::from("C:/docs/note.md"));
    config.save(&path).unwrap();
    let loaded = AppConfig::load(&path).unwrap();
    assert_eq!(loaded.theme, ThemeKind::Paper);
    assert_eq!(loaded.recent_files, config.recent_files);
    let _ = std::fs::remove_file(path);
}

#[test]
fn window_builds_paint_plan_for_document() {
    let path = temp_path("window");
    std::fs::write(&path, "window body\n").unwrap();
    let state = AppState::open(&path).unwrap();
    let mut view = AppWindow::new(state, Size::new(600.0, 400.0));
    view.relayout_with(&mut TestMeasurer);
    let selection = view.state.editor().selection().clone();
    let plan = view
        .view
        .paint(view.state.editor().document(), &selection, None);
    assert!(!plan.items.is_empty());
    let _ = std::fs::remove_file(path);
}

struct TestMeasurer;

impl md_layout::TextMeasurer for TestMeasurer {
    fn advance(&self, text: &str, style: &md_layout::MeasureStyle) -> f32 {
        text.chars().count() as f32 * style.font_size * 0.6
    }

    fn atom_advance(&self, _kind: &md_document::AtomKind, style: &md_layout::MeasureStyle) -> f32 {
        style.font_size
    }

    fn line_metrics(&self, style: &md_layout::MeasureStyle) -> md_layout::LineMetrics {
        md_layout::LineMetrics::new(
            style.line_height * 0.8,
            style.line_height * 0.2,
            style.line_height,
        )
    }
}
