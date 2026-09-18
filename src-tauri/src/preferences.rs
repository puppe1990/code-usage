use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Favorite {
    Grok,
    CommandCode,
    OpenCode,
}

impl Favorite {
    pub const ALL: [Favorite; 3] = [Favorite::Grok, Favorite::CommandCode, Favorite::OpenCode];

    pub fn label(self) -> &'static str {
        match self {
            Favorite::Grok => "Grok",
            Favorite::CommandCode => "Command Code",
            Favorite::OpenCode => "OpenCode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub favorite: Option<Favorite>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            favorite: Some(Favorite::CommandCode),
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
        Self { favorite }
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

pub fn load(path: &Path) -> Preferences {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Preferences::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Preferences::default();
    };
    if let Some(entry) = value.get("favorite") {
        return match entry.as_str() {
            Some(id) => parse_favorite(id)
                .map(|favorite| Preferences::with_favorite(Some(favorite)))
                .unwrap_or_default(),
            None => Preferences::with_favorite(None),
        };
    }

    // legacy shape: {"favorites": ["commandCode", ...]} — keep the first known entry
    if let Some(entries) = value.get("favorites").and_then(|list| list.as_array()) {
        let favorite = entries
            .iter()
            .filter_map(|entry| entry.as_str())
            .find_map(parse_favorite);

        return match favorite {
            Some(favorite) => Preferences::with_favorite(Some(favorite)),
            None => Preferences::default(),
        };
    }

    Preferences::default()
}

fn parse_favorite(id: &str) -> Option<Favorite> {
    serde_json::from_value::<Favorite>(serde_json::Value::String(id.to_string())).ok()
}

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
