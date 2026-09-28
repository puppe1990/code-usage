//! The single star selection, persisted to `~/Library/Application Support/code-usage/preferences.json`
//! (override with `CODE_USAGE_CONFIG`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Favorite {
    Grok,
    CommandCode,
    OpenCode,
    Codex,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Translucent,
}

pub const DEFAULT_ORDER: [Favorite; 4] = [
    Favorite::CommandCode,
    Favorite::Grok,
    Favorite::OpenCode,
    Favorite::Codex,
];

pub fn complete_order(order: &[Favorite]) -> Vec<Favorite> {
    let mut out = Vec::new();
    for id in order {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    for id in DEFAULT_ORDER {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

impl Favorite {
    pub const ALL: [Favorite; 4] = [
        Favorite::Grok,
        Favorite::CommandCode,
        Favorite::OpenCode,
        Favorite::Codex,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Favorite::Grok => "Grok",
            Favorite::CommandCode => "Command Code",
            Favorite::OpenCode => "OpenCode",
            Favorite::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub favorite: Option<Favorite>,
    #[serde(default)]
    pub hidden: Vec<Favorite>,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub order: Vec<Favorite>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            favorite: Some(Favorite::CommandCode),
            hidden: Vec::new(),
            theme: Theme::Dark,
            order: DEFAULT_ORDER.to_vec(),
        }
    }
}

impl Preferences {
    pub fn load() -> Self {
        load(&path())
    }

    pub fn save(&self) -> Result<(), String> {
        save(&path(), self)
    }

    pub fn with_favorite(favorite: Option<Favorite>) -> Self {
        Self {
            favorite,
            hidden: Vec::new(),
            theme: Theme::Dark,
            order: DEFAULT_ORDER.to_vec(),
        }
    }
}

pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("code-usage")
        .join("preferences.json")
}

pub fn path() -> PathBuf {
    std::env::var("CODE_USAGE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_path())
}

/// Reads the preferences file, falling back to the defaults for a missing or broken file.
pub fn load(path: &Path) -> Preferences {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Preferences::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Preferences::default();
    };
    let hidden = hidden_from(&value);
    let theme = theme_from(&value);
    let order = complete_order(&order_from(&value));
    if let Some(entry) = value.get("favorite") {
        return match entry.as_str() {
            Some(id) => parse_favorite(id)
                .map(|favorite| Preferences {
                    favorite: Some(favorite),
                    hidden,
                    theme,
                    order,
                })
                .unwrap_or_default(),
            None => Preferences {
                favorite: None,
                hidden,
                theme,
                order,
            },
        };
    }

    // legacy shape: {"favorites": ["commandCode", ...]} — keep the first known entry
    if let Some(entries) = value.get("favorites").and_then(|list| list.as_array()) {
        let favorite = entries
            .iter()
            .filter_map(|entry| entry.as_str())
            .find_map(parse_favorite);

        return match favorite {
            Some(favorite) => Preferences {
                favorite: Some(favorite),
                hidden,
                theme,
                order,
            },
            None => Preferences::default(),
        };
    }

    Preferences {
        favorite: Preferences::default().favorite,
        hidden,
        theme,
        order,
    }
}

fn theme_from(value: &serde_json::Value) -> Theme {
    value
        .get("theme")
        .and_then(|entry| entry.as_str())
        .and_then(|id| serde_json::from_value(serde_json::Value::String(id.to_string())).ok())
        .unwrap_or_default()
}

fn order_from(value: &serde_json::Value) -> Vec<Favorite> {
    value
        .get("order")
        .and_then(|list| list.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .filter_map(parse_favorite)
                .collect()
        })
        .unwrap_or_default()
}

fn hidden_from(value: &serde_json::Value) -> Vec<Favorite> {
    value
        .get("hidden")
        .and_then(|list| list.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .filter_map(parse_favorite)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_favorite(id: &str) -> Option<Favorite> {
    serde_json::from_value::<Favorite>(serde_json::Value::String(id.to_string())).ok()
}

/// Writes the preferences file, creating parent directories as needed.
pub fn save(path: &Path, preferences: &Preferences) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let body = serde_json::to_string_pretty(preferences).map_err(|error| error.to_string())?;
    std::fs::write(path, format!("{body}\n")).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn defaults_to_command_code() {
        assert_eq!(Preferences::default().favorite, Some(Favorite::CommandCode));
    }

    #[test]
    fn missing_or_broken_file_falls_back_to_defaults() {
        let dir = TempDir::new().expect("temp dir");
        let missing = dir.path().join("preferences.json");

        assert_eq!(load(&missing), Preferences::default());

        fs::write(&missing, "{ not json").expect("writes broken file");
        assert_eq!(load(&missing), Preferences::default());
    }

    #[test]
    fn round_trips_through_the_config_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("nested").join("preferences.json");
        let preferences = Preferences::with_favorite(Some(Favorite::Grok));

        save(&path, &preferences).expect("saves preferences");

        assert_eq!(load(&path).favorite, Some(Favorite::Grok));
    }

    #[test]
    fn accepts_an_explicit_null_favorite() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorite":null}"#).expect("writes config");

        assert_eq!(load(&path).favorite, None);
        assert_eq!(Preferences::with_favorite(None).favorite, None);
    }

    #[test]
    fn ignores_unknown_ids_from_the_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorite":"openCodeTodayCost"}"#).expect("writes config");

        assert_eq!(load(&path), Preferences::default());
    }

    #[test]
    fn round_trips_hidden_harnesses() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        let preferences = Preferences {
            favorite: Some(Favorite::Grok),
            hidden: vec![Favorite::Codex, Favorite::OpenCode],
            theme: Theme::Light,
            order: DEFAULT_ORDER.to_vec(),
        };

        save(&path, &preferences).expect("saves preferences");

        assert_eq!(load(&path), preferences);
    }

    #[test]
    fn round_trips_harness_order() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        let preferences = Preferences {
            favorite: Some(Favorite::Grok),
            hidden: Vec::new(),
            theme: Theme::Dark,
            order: vec![
                Favorite::Grok,
                Favorite::Codex,
                Favorite::CommandCode,
                Favorite::OpenCode,
            ],
        };

        save(&path, &preferences).expect("saves preferences");

        assert_eq!(load(&path).order, preferences.order);
    }

    #[test]
    fn missing_order_uses_the_default() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorite":"grok"}"#).expect("writes config");

        assert_eq!(load(&path).order, DEFAULT_ORDER.to_vec());
    }

    #[test]
    fn missing_theme_defaults_to_dark() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorite":"grok"}"#).expect("writes config");

        assert_eq!(load(&path).theme, Theme::Dark);
    }

    #[test]
    fn missing_hidden_stays_empty() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorite":"grok"}"#).expect("writes config");

        assert_eq!(load(&path).hidden, Vec::<Favorite>::new());
        assert_eq!(load(&path).favorite, Some(Favorite::Grok));
    }

    #[test]
    fn migrates_the_legacy_favorites_array() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorites":["openCode","grok"]}"#).expect("writes config");

        assert_eq!(load(&path).favorite, Some(Favorite::OpenCode));
    }

    #[test]
    fn legacy_files_without_known_entries_fall_back_to_defaults() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(&path, r#"{"favorites":["grokWeekly"]}"#).expect("writes config");

        assert_eq!(load(&path), Preferences::default());
    }
}
