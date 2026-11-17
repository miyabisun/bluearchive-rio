//! What the user chose in the window, kept for the next start in `settings.json` beside the
//! database: the theme, and where the window was left. A file missing or unreadable, or a
//! value this version does not know, means the default for that choice (as Aoi's).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The theme: light, dark, or the OS's (自動, the default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    Light,
    Dark,
    #[default]
    Auto,
}

impl ThemeChoice {
    /// The settings page's choices, in order, with their names.
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::Auto, "自動"),
        (Self::Light, "ライト"),
        (Self::Dark, "ダーク"),
    ];
}

/// Where the window was left: the monitor, by the id GPUI gives it, and the window's place
/// and size in that monitor's logical pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub display: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub window: Option<Placement>,
}

impl Settings {
    /// The settings `text` holds, each choice that cannot be read at its default.
    pub fn parse(text: &str) -> Self {
        let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(text) else {
            return Self::default();
        };
        let mut settings = Self::default();
        if let Some(theme) = fields.get("theme") {
            settings.theme = ThemeChoice::deserialize(theme.clone()).unwrap_or_default();
        }
        if let Some(window) = fields.get("window") {
            settings.window = Placement::deserialize(window.clone())
                .ok()
                .filter(|placement| placement.width > 0 && placement.height > 0);
        }
        settings
    }

    /// The settings kept at `path`, the defaults when there are none.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::parse(&text))
    }

    /// Keeps the settings at `path` for the next start: written whole through a file beside
    /// it, or not at all.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |error: std::io::Error| format!("{}: {error}", path.display());
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        let part = path.with_extension("part");
        std::fs::write(&part, text).map_err(fail)?;
        std::fs::rename(&part, path).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use super::{Placement, Settings, ThemeChoice};

    #[test]
    fn what_was_chosen_comes_back() {
        let dir = std::env::temp_dir().join(format!("rio-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default(), "no file yet");
        let chosen = Settings {
            theme: ThemeChoice::Light,
            window: Some(Placement {
                display: "monitor".to_owned(),
                x: -1200,
                y: 40,
                width: 720,
                height: 820,
            }),
        };
        chosen.save(&path).unwrap();
        assert_eq!(Settings::load(&path), chosen);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_choice_that_does_not_read_is_the_default() {
        let read = Settings::parse(r#"{"theme": "sepia", "window": {"display": "m", "x": 0}}"#);
        assert_eq!(read, Settings::default());
        assert_eq!(Settings::parse("not json"), Settings::default());
        assert_eq!(
            Settings::parse(r#"{"theme": "dark", "later": 1}"#).theme,
            ThemeChoice::Dark,
            "a field of a later version is passed over"
        );
    }
}
