//! Application configuration and theme selection.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use md_render::{Color, Theme};

/// Named application theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeKind {
    /// Dark ink theme.
    #[default]
    Ink,
    /// Light paper theme.
    Paper,
    /// High-contrast theme.
    Contrast,
}

impl ThemeKind {
    /// Returns the renderer theme.
    #[must_use]
    pub fn render_theme(self) -> Theme {
        let mut theme = Theme::default();
        match self {
            Self::Ink => {
                theme.background = Color(0x1B1D_22FF);
                theme.text = Color(0xE6E8_EEFF);
                theme.muted = Color(0x9AA3_B2FF);
                theme.selection = Color(0x3550_7FFF);
                theme.caret = Color(0x7FB4_FFFF);
                theme.code_background = Color(0x2428_31FF);
                theme.quote_background = Color(0x2023_2AFF);
                theme.border = Color(0x3A40_4DFF);
            }
            Self::Paper => {}
            Self::Contrast => {
                theme.background = Color::WHITE;
                theme.text = Color::BLACK;
                theme.selection = Color(0xFFE0_00FF);
                theme.caret = Color::BLACK;
                theme.code_background = Color(0xF0F0_00FF);
                theme.quote_background = Color(0xE8E8_E8FF);
                theme.border = Color::BLACK;
            }
        }
        theme
    }

    /// Returns the stable config token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Ink => "ink",
            Self::Paper => "paper",
            Self::Contrast => "contrast",
        }
    }

    /// Parses a config token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "ink" => Some(Self::Ink),
            "paper" => Some(Self::Paper),
            "contrast" => Some(Self::Contrast),
            _ => None,
        }
    }
}

/// Persisted application preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct AppConfig {
    /// Active theme.
    pub theme: ThemeKind,
    /// Body font size.
    pub font_size: f32,
    /// Body line height.
    pub line_height: f32,
    /// Recent file paths, most recent first.
    pub recent_files: Vec<PathBuf>,
    /// Maximum recent file count.
    pub max_recent: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemeKind::Ink,
            font_size: 16.0,
            line_height: 24.0,
            recent_files: Vec::new(),
            max_recent: 12,
        }
    }
}

impl AppConfig {
    /// Loads a config file, returning defaults when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an I/O error other than `NotFound`.
    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(source) => Ok(Self::from_toml(&source)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    /// Saves a config file.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the file cannot be written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml())
    }

    /// Serializes to a small TOML subset.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("theme = \"{}\"\n", self.theme.token()));
        output.push_str(&format!("font_size = {:.1}\n", self.font_size));
        output.push_str(&format!("line_height = {:.1}\n", self.line_height));
        output.push_str("recent_files = [");
        for (index, path) in self.recent_files.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push('"');
            output.push_str(
                &path
                    .to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\""),
            );
            output.push('"');
        }
        output.push_str("]\n");
        output
    }

    /// Parses the small TOML subset produced by [`Self::to_toml`].
    #[must_use]
    pub fn from_toml(source: &str) -> Self {
        let mut config = Self::default();
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "theme" => {
                    if let Some(theme) = ThemeKind::parse(value.trim_matches('"')) {
                        config.theme = theme;
                    }
                }
                "font_size" => {
                    if let Ok(size) = value.parse() {
                        config.font_size = size;
                    }
                }
                "line_height" => {
                    if let Ok(height) = value.parse() {
                        config.line_height = height;
                    }
                }
                "recent_files" => {
                    config.recent_files = parse_string_array(value);
                }
                _ => {}
            }
        }
        config.recent_files.truncate(config.max_recent);
        config
    }

    /// Remembers a recent path.
    pub fn remember(&mut self, path: PathBuf) {
        self.recent_files.retain(|existing| existing != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(self.max_recent);
    }
}

fn parse_string_array(value: &str) -> Vec<PathBuf> {
    let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .filter_map(|item| {
            let item = item.trim().trim_matches('"');
            if item.is_empty() {
                None
            } else {
                Some(PathBuf::from(
                    item.replace("\\\\", "\\").replace("\\\"", "\""),
                ))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ThemeKind, parse_string_array};
    use std::path::PathBuf;

    #[test]
    fn config_round_trips() {
        let mut config = AppConfig {
            theme: ThemeKind::Contrast,
            font_size: 18.0,
            ..AppConfig::default()
        };
        config.remember(PathBuf::from("C:/docs/a.md"));
        let parsed = AppConfig::from_toml(&config.to_toml());
        assert_eq!(parsed.theme, ThemeKind::Contrast);
        assert_eq!(parsed.font_size, 18.0);
        assert_eq!(parsed.recent_files, vec![PathBuf::from("C:/docs/a.md")]);
    }

    #[test]
    fn string_array_handles_quotes_and_empty() {
        assert!(parse_string_array("[]").is_empty());
        assert_eq!(
            parse_string_array("[\"a\", \"b\"]"),
            vec![PathBuf::from("a"), PathBuf::from("b")]
        );
    }
}
