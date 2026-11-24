//! Menu and command model.

/// Application command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// New document.
    New,
    /// Open document.
    Open,
    /// Save document.
    Save,
    /// Save as.
    SaveAs,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Copy.
    Copy,
    /// Paste.
    Paste,
    /// Toggle theme.
    ToggleTheme,
    /// Quit.
    Quit,
}

impl Command {
    /// Returns a stable command id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::New => "file.new",
            Self::Open => "file.open",
            Self::Save => "file.save",
            Self::SaveAs => "file.save_as",
            Self::Undo => "edit.undo",
            Self::Redo => "edit.redo",
            Self::Copy => "edit.copy",
            Self::Paste => "edit.paste",
            Self::ToggleTheme => "view.toggle_theme",
            Self::Quit => "file.quit",
        }
    }

    /// Returns the shortcut label.
    #[must_use]
    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::New => "Ctrl+N",
            Self::Open => "Ctrl+O",
            Self::Save => "Ctrl+S",
            Self::SaveAs => "Ctrl+Shift+S",
            Self::Undo => "Ctrl+Z",
            Self::Redo => "Ctrl+Shift+Z",
            Self::Copy => "Ctrl+C",
            Self::Paste => "Ctrl+V",
            Self::ToggleTheme => "Ctrl+Shift+T",
            Self::Quit => "Ctrl+Q",
        }
    }

    /// Resolves a shortcut label.
    #[must_use]
    pub fn from_shortcut(shortcut: &str) -> Option<Self> {
        const COMMANDS: [Command; 10] = [
            Command::New,
            Command::Open,
            Command::Save,
            Command::SaveAs,
            Command::Undo,
            Command::Redo,
            Command::Copy,
            Command::Paste,
            Command::ToggleTheme,
            Command::Quit,
        ];
        COMMANDS
            .into_iter()
            .find(|command| command.shortcut().eq_ignore_ascii_case(shortcut))
    }
}

/// One menu entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// Display label.
    pub label: &'static str,
    /// Bound command.
    pub command: Command,
}

/// One top-level menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// Menu title.
    pub title: &'static str,
    /// Entries.
    pub items: Vec<MenuItem>,
}

/// Returns the application menu model.
#[must_use]
pub fn menus() -> Vec<Menu> {
    vec![
        Menu {
            title: "文件",
            items: vec![
                item("新建", Command::New),
                item("打开…", Command::Open),
                item("保存", Command::Save),
                item("另存为…", Command::SaveAs),
                item("退出", Command::Quit),
            ],
        },
        Menu {
            title: "编辑",
            items: vec![
                item("撤销", Command::Undo),
                item("重做", Command::Redo),
                item("复制", Command::Copy),
                item("粘贴", Command::Paste),
            ],
        },
        Menu {
            title: "格式",
            items: vec![],
        },
        Menu {
            title: "视图",
            items: vec![item("切换主题", Command::ToggleTheme)],
        },
        Menu {
            title: "主题",
            items: vec![],
        },
        Menu {
            title: "帮助",
            items: vec![],
        },
    ]
}

fn item(label: &'static str, command: Command) -> MenuItem {
    MenuItem { label, command }
}

#[cfg(test)]
mod tests {
    use super::{Command, menus};

    #[test]
    fn shortcuts_are_unique_and_round_trip() {
        let mut shortcuts = Vec::new();
        for command in [
            Command::New,
            Command::Open,
            Command::Save,
            Command::SaveAs,
            Command::Undo,
            Command::Redo,
            Command::Copy,
            Command::Paste,
            Command::ToggleTheme,
            Command::Quit,
        ] {
            assert!(!shortcuts.contains(&command.shortcut()));
            shortcuts.push(command.shortcut());
            assert_eq!(Command::from_shortcut(command.shortcut()), Some(command));
        }
    }

    #[test]
    fn menu_model_has_six_top_level_menus() {
        let menus = menus();
        assert_eq!(menus.len(), 6);
        assert!(menus.iter().any(|menu| menu.title == "文件"));
    }
}
