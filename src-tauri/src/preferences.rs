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
    pub favorites: Vec<Favorite>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            favorites: vec![Favorite::Grok, Favorite::CommandCode, Favorite::OpenCode],
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

    pub fn with_favorites(favorites: Vec<Favorite>) -> Self {
        Self {
            favorites: normalize(favorites),
        }
    }
}

fn normalize(favorites: Vec<Favorite>) -> Vec<Favorite> {
    Favorite::ALL
        .into_iter()
        .filter(|favorite| favorites.contains(favorite))
        .collect()
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
    let Some(entries) = value
        .get("favorites")
        .and_then(|favorites| favorites.as_array())
    else {
        return Preferences::default();
    };

    let favorites: Vec<Favorite> = entries
        .iter()
        .filter_map(|entry| entry.as_str())
        .filter_map(|entry| {
            serde_json::from_value::<Favorite>(serde_json::Value::String(entry.to_string())).ok()
        })
        .collect();

    // a file with entries but none recognized is a legacy/foreign config: keep the defaults
    if favorites.is_empty() && !entries.is_empty() {
        return Preferences::default();
    }

    Preferences::with_favorites(favorites)
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
    fn defaults_to_every_harness() {
        let preferences = Preferences::default();

        assert_eq!(
            preferences.favorites,
            vec![Favorite::Grok, Favorite::CommandCode, Favorite::OpenCode]
        );
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
        let preferences = Preferences::with_favorites(vec![Favorite::OpenCode, Favorite::Grok]);

        save(&path, &preferences).expect("saves preferences");

        let loaded = load(&path);
        assert_eq!(loaded.favorites, vec![Favorite::Grok, Favorite::OpenCode]);
    }

    #[test]
    fn drops_unknown_favorites_and_duplicates_from_the_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(
            &path,
            r#"{"favorites":["openCode","commandCodePlan","openCode","grok"]}"#,
        )
        .expect("writes config");

        assert_eq!(
            load(&path).favorites,
            vec![Favorite::Grok, Favorite::OpenCode]
        );
    }

    #[test]
    fn files_with_only_unknown_favorites_fall_back_to_defaults() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");
        fs::write(
            &path,
            r#"{"favorites":["grokWeekly","commandCodeTodayCost","openCodeTodayCost"]}"#,
        )
        .expect("writes config");

        assert_eq!(load(&path), Preferences::default());
    }

    #[test]
    fn accepts_an_empty_favorite_list() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("preferences.json");

        save(&path, &Preferences::with_favorites(vec![])).expect("saves preferences");

        assert!(load(&path).favorites.is_empty());
    }
}
