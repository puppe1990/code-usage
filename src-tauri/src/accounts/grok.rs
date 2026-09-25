//! Grok accounts: the saved profiles under `<home>/accounts` (the format
//! [grok-accounts](https://github.com/puppe1990/grok-accounts) writes) plus the live
//! `<home>/auth.json`. The profile in use is the one whose e-mail matches the live file, and
//! switching copies the profile's credentials over it.

use super::{read_json, write_json, Account};
use crate::usage::grok::auth_path;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn list() -> Result<Vec<Account>, String> {
    list_at(&auth_path())
}

pub fn switch(name: &str) -> Result<(), String> {
    switch_at(&auth_path(), name)
}

fn list_at(auth: &Path) -> Result<Vec<Account>, String> {
    let live = live_email(auth);

    let mut accounts = profiles(auth)?
        .into_iter()
        .map(|profile| Account {
            name: profile.name(),
            active: live.is_some() && live == profile.email(),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(accounts)
}

fn switch_at(auth: &Path, name: &str) -> Result<(), String> {
    let profile = profiles(auth)?
        .into_iter()
        .find(|profile| profile.matches(name))
        .ok_or_else(|| format!("conta {name:?} não existe"))?;

    let Some(credentials) = profile.credentials else {
        return Err(format!(
            "perfil {:?} não tem credencial salva",
            profile.name()
        ));
    };

    write_json(auth, &credentials)
}

#[derive(Debug, Clone, PartialEq)]
struct Profile {
    alias: String,
    email: String,
    credentials: Option<Value>,
}

impl Profile {
    fn name(&self) -> String {
        if !self.alias.trim().is_empty() {
            return self.alias.trim().to_string();
        }
        if !self.email.trim().is_empty() {
            return self.email.trim().to_string();
        }
        "perfil".to_string()
    }

    fn email(&self) -> Option<String> {
        let email = self.email.trim().to_lowercase();
        (!email.is_empty()).then_some(email)
    }

    fn matches(&self, name: &str) -> bool {
        self.alias.trim().eq_ignore_ascii_case(name.trim())
            || self.email.trim().eq_ignore_ascii_case(name.trim())
    }
}

/// One `<home>/accounts/*.json` profile per file, alphabetically.
fn profiles(auth: &Path) -> Result<Vec<Profile>, String> {
    let dir = profiles_dir_for(auth);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("não consegui ler {}: {error}", dir.display())),
    };

    let mut profiles = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() || file_name.starts_with('.') || !file_name.ends_with(".json") {
            continue;
        }

        let value = read_json(&entry.path())?
            .ok_or_else(|| format!("perfil {} desapareceu", entry.path().display()))?;
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        profiles.push(Profile {
            alias: text("alias"),
            email: text("email"),
            credentials: value.get("auth").filter(|auth| !auth.is_null()).cloned(),
        });
    }

    Ok(profiles)
}

fn profiles_dir_for(auth: &Path) -> PathBuf {
    auth.with_file_name("accounts")
}

/// E-mail of the live login: the first credential (by key) that carries one.
fn live_email(auth: &Path) -> Option<String> {
    let value = read_json(auth).ok().flatten()?;
    let mut keys = value.as_object()?.keys().collect::<Vec<_>>();
    keys.sort();

    keys.into_iter().find_map(|key| {
        value
            .get(key)?
            .get("email")?
            .as_str()
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(str::to_lowercase)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn profile(dir: &TempDir, file: &str, alias: &str, email: &str, key: &str) {
        let profiles = dir.path().join("accounts");
        fs::create_dir_all(&profiles).expect("creates profiles dir");
        fs::write(
            profiles.join(file),
            json!({
                "alias": alias,
                "email": email,
                "saved_at": "2026-09-22T02:35:23-03:00",
                "auth": {"https://auth.x.ai::client": {"key": key, "email": email}},
            })
            .to_string(),
        )
        .expect("writes profile");
    }

    fn live(dir: &TempDir, email: &str, key: &str) -> PathBuf {
        let auth = dir.path().join("auth.json");
        fs::write(
            &auth,
            json!({"https://auth.x.ai::client": {"key": key, "email": email}}).to_string(),
        )
        .expect("writes live auth");
        auth
    }

    #[test]
    fn lists_the_profiles_with_the_live_email_active() {
        let dir = TempDir::new().expect("temp dir");
        profile(
            &dir,
            "pessoal.json",
            "pessoal",
            "eu@gmail.com",
            "key-personal",
        );
        profile(
            &dir,
            "trabalho.json",
            "trabalho",
            "trabalho@empresa.com",
            "key-work",
        );
        let auth = live(&dir, "EU@gmail.com", "key-personal");

        assert_eq!(
            list_at(&auth).expect("lists"),
            vec![
                Account {
                    name: "pessoal".to_string(),
                    active: true
                },
                Account {
                    name: "trabalho".to_string(),
                    active: false
                },
            ]
        );
    }

    #[test]
    fn switching_copies_the_profile_credentials_over_the_live_file() {
        let dir = TempDir::new().expect("temp dir");
        profile(
            &dir,
            "trabalho.json",
            "trabalho",
            "trabalho@empresa.com",
            "key-work",
        );
        let auth = live(&dir, "eu@gmail.com", "key-personal");

        switch_at(&auth, "trabalho").expect("switches");

        let value: Value = serde_json::from_str(&fs::read_to_string(&auth).expect("live")).unwrap();
        assert_eq!(value["https://auth.x.ai::client"]["key"], json!("key-work"));
        assert!(list_at(&auth).expect("lists")[0].active);
    }

    #[test]
    fn switching_by_email_works_too() {
        let dir = TempDir::new().expect("temp dir");
        profile(
            &dir,
            "trabalho.json",
            "trabalho",
            "trabalho@empresa.com",
            "key-work",
        );
        let auth = live(&dir, "eu@gmail.com", "key-personal");

        switch_at(&auth, "trabalho@empresa.com").expect("switches by email");

        let value: Value = serde_json::from_str(&fs::read_to_string(&auth).expect("live")).unwrap();
        assert_eq!(value["https://auth.x.ai::client"]["key"], json!("key-work"));
    }

    #[test]
    fn an_unknown_profile_is_an_error_and_changes_nothing() {
        let dir = TempDir::new().expect("temp dir");
        profile(
            &dir,
            "trabalho.json",
            "trabalho",
            "trabalho@empresa.com",
            "key-work",
        );
        let auth = live(&dir, "eu@gmail.com", "key-personal");
        let before = fs::read_to_string(&auth).expect("live");

        assert_eq!(
            switch_at(&auth, "nope").expect_err("unknown profile"),
            "conta \"nope\" não existe"
        );
        assert_eq!(fs::read_to_string(&auth).expect("live"), before);
    }

    #[test]
    fn a_profile_without_credentials_is_refused() {
        let dir = TempDir::new().expect("temp dir");
        let profiles = dir.path().join("accounts");
        fs::create_dir_all(&profiles).expect("creates profiles dir");
        fs::write(
            profiles.join("vazio.json"),
            json!({"alias": "vazio", "email": "vazio@gmail.com"}).to_string(),
        )
        .expect("writes profile");
        let auth = live(&dir, "eu@gmail.com", "key-personal");

        assert!(switch_at(&auth, "vazio")
            .expect_err("no credentials")
            .contains("não tem credencial salva"));
    }

    #[test]
    fn a_missing_profiles_dir_lists_nothing() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");

        assert_eq!(list_at(&auth).expect("lists"), Vec::new());
    }
}
