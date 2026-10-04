//! Grok accounts: the saved profiles under `<home>/accounts` (the format
//! [grok-accounts](https://github.com/puppe1990/grok-accounts) writes) plus the live
//! `<home>/auth.json`. The profile in use is the one whose e-mail matches the live file, and
//! switching copies the profile's credentials over it.

use super::{read_json, write_json, Account};
use crate::usage::grok::auth_path;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static SESSION_ACCESS: Mutex<()> = Mutex::new(());

pub fn list() -> Result<Vec<Account>, String> {
    let _guard = session_access()?;
    list_at(&auth_path())
}

pub fn switch(name: &str) -> Result<(), String> {
    let _guard = session_access()?;
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

pub fn sync() -> Result<(), String> {
    let _guard = session_access()?;
    let auth = auth_path();
    let live = read_json(&auth)?;
    snapshot_live(&auth, &profiles(&auth)?, live.as_ref())
}

fn session_access() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    SESSION_ACCESS
        .lock()
        .map_err(|_| "A sincronização do Grok falhou. Reinicie o Code Usage.".to_string())
}

fn switch_at(auth: &Path, name: &str) -> Result<(), String> {
    let profiles = profiles(auth)?;
    let target = profiles
        .iter()
        .find(|p| p.matches(name))
        .ok_or_else(|| format!("conta {name:?} não existe"))?;
    let live = read_json(auth)?;
    if live
        .as_ref()
        .and_then(credential_email)
        .is_some_and(|email| Some(email) == target.email())
    {
        return snapshot_live(auth, &profiles, live.as_ref());
    }
    let credentials = validated_credentials(target)?;
    snapshot_live(auth, &profiles, live.as_ref())?;
    write_json(auth, credentials)
}

fn validated_credentials(profile: &Profile) -> Result<&Value, String> {
    let credentials = profile
        .credentials
        .as_ref()
        .ok_or_else(|| format!("perfil {:?} não tem credencial salva", profile.name()))?;
    if credential_email(credentials).is_none() || credential_email(credentials) != profile.email() {
        return Err(format!("A credencial de {:?} não corresponde à conta salva. Rode grok-accounts login para atualizar o perfil.", profile.name()));
    }
    validate_expiry(credentials, &profile.name())?;
    Ok(credentials)
}

fn validate_expiry(credentials: &Value, name: &str) -> Result<(), String> {
    for entry in credentials
        .as_object()
        .into_iter()
        .flat_map(|entries| entries.values())
    {
        let expiry = entry
            .get("expires_at")
            .and_then(Value::as_str)
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok());
        let can_refresh = entry
            .get("refresh_token")
            .and_then(Value::as_str)
            .is_some_and(|token| !token.trim().is_empty());
        if entry.get("email").and_then(Value::as_str).is_some()
            && expiry.is_some_and(|expires| expires < Utc::now())
            && !can_refresh
        {
            return Err(format!("A sessão de {name:?} expirou e não pode ser renovada. Rode grok-accounts login para atualizar o perfil."));
        }
    }
    Ok(())
}

fn snapshot_live(auth: &Path, profiles: &[Profile], live: Option<&Value>) -> Result<(), String> {
    let Some(live) = live else { return Ok(()) };
    let Some(email) = credential_email(live) else {
        return Err("A sessão atual do Grok não contém uma conta válida. Rode grok-accounts login; nenhuma credencial foi substituída.".to_string());
    };
    let matching = profiles
        .iter()
        .filter(|profile| profile.email().as_ref() == Some(&email))
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return save_unsaved(auth, &email, live);
    }
    for profile in matching {
        update_snapshot(&profile.path, live)?;
    }
    Ok(())
}

fn update_snapshot(path: &Path, live: &Value) -> Result<(), String> {
    let mut saved = read_json(path)?.ok_or_else(|| {
        format!(
            "O perfil {} desapareceu; a troca foi cancelada.",
            path.display()
        )
    })?;
    if saved.get("auth") == Some(live) {
        return Ok(());
    }
    saved["auth"] = live.clone();
    saved["saved_at"] = json!(Utc::now().to_rfc3339());
    write_json(path, &saved)
}

fn save_unsaved(auth: &Path, email: &str, live: &Value) -> Result<(), String> {
    let alias = email
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let path = profiles_dir_for(auth).join(format!("{alias}.json"));
    if path.exists() {
        return Err("Não foi possível guardar a conta atual sem sobrescrever outro perfil. Rode grok-accounts add --alias NOME antes de trocar.".to_string());
    }
    write_json(
        &path,
        &json!({"alias": email, "email": email, "saved_at": Utc::now().to_rfc3339(), "auth": live}),
    )
}

#[derive(Debug, Clone, PartialEq)]
struct Profile {
    path: PathBuf,
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
            path: entry.path(),
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
    credential_email(&value)
}

fn credential_email(value: &Value) -> Option<String> {
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
#[path = "grok_tests.rs"]
mod tests;
