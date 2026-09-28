//! Codex accounts: the ledger `codex-auth` keeps in `<home>/accounts/registry.json` plus the live
//! `<home>/auth.json`. Which login is in use is whatever the live file carries (its `id_token`
//! names the account), and switching copies the saved snapshot over it and marks the account as
//! active in the registry, the same way `codex-auth switch` does.

use super::{read_json, write_json, Account};
use crate::usage::codex::{auth_path, read_record_key};
use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const FIELDS: [&str; 3] = ["alias", "email", "account_key"];

pub fn list() -> Result<Vec<Account>, String> {
    list_at(&auth_path())
}

pub fn switch(name: &str) -> Result<(), String> {
    switch_at(&auth_path(), name)
}

fn list_at(auth: &Path) -> Result<Vec<Account>, String> {
    let registry = registry_at(auth)?;
    let live = read_record_key(auth).or_else(|| {
        registry
            .get("active_account_key")
            .and_then(Value::as_str)
            .map(str::to_string)
    });

    let mut accounts = accounts(&registry)
        .map(|account| Account {
            name: name_of(account),
            active: live.as_deref() == Some(key(account).as_str()),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(accounts)
}

fn switch_at(auth: &Path, name: &str) -> Result<(), String> {
    let mut registry = registry_at(auth)?;
    let found = accounts(&registry)
        .filter(|account| matches(account, name))
        .map(key)
        .collect::<Vec<_>>();

    let account_key = match found.len() {
        0 => return Err(not_found(name, &registry)),
        1 => found.into_iter().next().expect("one account"),
        count => {
            return Err(format!(
                "há {count} contas Codex chamadas {name:?}. Diferencie-as com codex-auth import --alias <nome>."
            ))
        }
    };

    let snapshot = snapshot_path(auth, &account_key);
    let credentials = read_json(&snapshot)?.ok_or_else(|| {
        format!(
            "conta {name:?} não tem credencial salva em {}",
            snapshot.display()
        )
    })?;

    // the live file first: if the registry write fails afterwards, the CLI is already on the new
    // login and `codex-auth` syncs the ledger on its next run
    write_json(auth, &credentials)?;

    let now = Utc::now();
    registry["active_account_key"] = json!(account_key);
    registry["active_account_activated_at_ms"] = json!(now.timestamp_millis());
    mark_used(&mut registry, &account_key, now.timestamp());
    write_json(&registry_path(auth), &registry)?;

    Ok(())
}

fn mark_used(registry: &mut Value, account_key: &str, seconds: i64) {
    let Some(records) = registry.get_mut("accounts").and_then(Value::as_array_mut) else {
        return;
    };

    for record in records {
        if record.get("account_key").and_then(Value::as_str) == Some(account_key) {
            record["last_used_at"] = json!(seconds);
        }
    }
}

/// The ledger, refusing anything that is not the shape `codex-auth` writes.
fn registry_at(auth: &Path) -> Result<Value, String> {
    let path = registry_path(auth);
    let registry = read_json(&path)?.ok_or_else(|| missing(auth))?;

    if registry.get("accounts").and_then(Value::as_array).is_none() {
        return Err(format!(
            "{} não é um registro do codex-auth válido",
            path.display()
        ));
    }

    Ok(registry)
}

fn accounts(registry: &Value) -> impl Iterator<Item = &Value> {
    registry
        .get("accounts")
        .and_then(Value::as_array)
        .map(|accounts| accounts.iter())
        .into_iter()
        .flatten()
}

/// What the panel shows for an account: the alias, its e-mail, or the raw key.
fn name_of(account: &Value) -> String {
    FIELDS
        .iter()
        .filter_map(|field| account.get(field).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "conta".to_string())
}

fn matches(account: &Value, name: &str) -> bool {
    FIELDS.iter().any(|field| {
        account
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(name.trim()))
    })
}

fn key(account: &Value) -> String {
    account
        .get("account_key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Credentials `codex-auth` saved for `account_key`: the key itself when it works as a file name,
/// the base64url of it otherwise (the keys carry `::`, so they are encoded in practice).
fn snapshot_path(auth: &Path, account_key: &str) -> PathBuf {
    let dir = auth.with_file_name("accounts");
    let plain = dir.join(format!("{account_key}.auth.json"));

    if plain.exists() {
        return plain;
    }

    dir.join(format!(
        "{}.auth.json",
        crate::base64url::encode(account_key.as_bytes())
    ))
}

fn registry_path(auth: &Path) -> PathBuf {
    auth.with_file_name("accounts").join("registry.json")
}

fn missing(auth: &Path) -> String {
    format!(
        "não achei o registro do codex-auth em {}. Rode codex-auth login.",
        registry_path(auth).display()
    )
}

fn not_found(name: &str, registry: &Value) -> String {
    let mut names = accounts(registry).map(name_of).collect::<Vec<_>>();
    names.sort();
    names.dedup();

    if names.is_empty() {
        return format!("conta {name:?} não existe. não há contas Codex salvas");
    }
    format!(
        "conta {name:?} não existe. Contas Codex: {}.",
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fixture_home() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("codex")
    }

    /// The fixture stores in a temp dir, so the tests can switch without touching the originals.
    fn store(dir: &TempDir) -> PathBuf {
        let accounts = dir.path().join("accounts");
        fs::create_dir_all(&accounts).expect("creates accounts dir");

        for entry in fs::read_dir(fixture_home().join("accounts"))
            .expect("reads fixtures")
            .flatten()
        {
            fs::copy(entry.path(), accounts.join(entry.file_name())).expect("copies fixture");
        }

        let auth = dir.path().join("auth.json");
        fs::copy(fixture_home().join("auth.json"), &auth).expect("copies live auth");
        auth
    }

    fn live(auth: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(auth).expect("live auth")).expect("valid json")
    }

    fn registry(auth: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(registry_path(auth)).expect("registry"))
            .expect("valid json")
    }

    fn live_token(auth: &Path) -> String {
        live(auth)["tokens"]["access_token"]
            .as_str()
            .expect("access token")
            .to_string()
    }

    #[test]
    fn lists_the_ledger_with_the_live_login_active() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);

        assert_eq!(
            list_at(&auth).expect("lists"),
            vec![
                Account {
                    name: "pessoal".to_string(),
                    active: false
                },
                Account {
                    name: "puppeicaropuppe@gmail.com".to_string(),
                    active: true
                },
                Account {
                    name: "trabalho@empresa.com".to_string(),
                    active: false
                },
            ],
            "sorted by name, the login of auth.json marked"
        );
    }

    #[test]
    fn falls_back_to_the_registry_when_the_live_file_cannot_be_read() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);
        fs::remove_file(&auth).expect("removes live auth");

        let accounts = list_at(&auth).expect("lists");

        assert!(accounts
            .iter()
            .any(|account| account.active && account.name == "puppeicaropuppe@gmail.com"));
    }

    #[test]
    fn switching_copies_the_snapshot_and_marks_the_registry() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);

        switch_at(&auth, "trabalho@empresa.com").expect("switches");

        assert_eq!(
            live_token(&auth),
            "work-access-token",
            "the live login changed"
        );
        assert_eq!(
            registry(&auth)["active_account_key"],
            json!("user-work::acct-work")
        );
        assert!(registry(&auth)["active_account_activated_at_ms"].is_number());
        assert_eq!(
            registry(&auth)["accounts"][1]["last_used_at"],
            json!(Utc::now().timestamp())
        );

        assert_eq!(
            list_at(&auth).expect("lists")[2],
            Account {
                name: "trabalho@empresa.com".to_string(),
                active: true
            }
        );
    }

    #[test]
    fn switching_by_alias_works_too() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);

        switch_at(&auth, "pessoal").expect("switches by alias");

        assert_eq!(live_token(&auth), "pessoal-access-token");
        assert_eq!(
            registry(&auth)["active_account_key"],
            json!("user-pessoal::acct-pessoal")
        );
    }

    #[test]
    fn an_unknown_account_is_an_error_and_changes_nothing() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);
        let before = fs::read_to_string(&auth).expect("live auth");

        let error = switch_at(&auth, "nope").expect_err("unknown account");

        assert!(error.contains("conta \"nope\" não existe"));
        assert!(
            error.contains("trabalho@empresa.com"),
            "the names are listed"
        );
        assert_eq!(fs::read_to_string(&auth).expect("live auth"), before);
    }

    #[test]
    fn duplicate_names_are_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);
        let mut ledger = registry(&auth);
        let mut clone = ledger["accounts"][1].clone();
        clone["account_key"] = json!("user-work-2::acct-work-2");
        clone["email"] = json!("trabalho@empresa.com");
        ledger["accounts"]
            .as_array_mut()
            .expect("accounts")
            .push(clone);
        write_json(&registry_path(&auth), &ledger).expect("writes ledger");

        let error = switch_at(&auth, "trabalho@empresa.com").expect_err("duplicate");

        assert!(error.contains("há 2 contas Codex chamadas"));
        assert!(error.contains("codex-auth import --alias"));
    }

    #[test]
    fn an_account_without_a_saved_snapshot_is_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);
        fs::remove_file(
            dir.path()
                .join("accounts")
                .join("dXNlci13b3JrOjphY2N0LXdvcms.auth.json"),
        )
        .expect("removes the snapshot");

        let error = switch_at(&auth, "trabalho@empresa.com").expect_err("no snapshot");

        assert!(error.contains("não tem credencial salva"));
        assert_eq!(live_token(&auth), "fixture-access-token", "nothing changed");
    }

    #[test]
    fn a_missing_registry_says_what_to_do() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");

        let error = list_at(&auth).expect_err("missing registry");

        assert!(error.contains("não achei o registro do codex-auth"));
        assert!(error.contains("codex-auth login"));
    }

    #[test]
    fn a_ledger_that_is_not_the_codex_auth_shape_is_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = store(&dir);
        fs::write(
            dir.path().join("accounts").join("registry.json"),
            json!({"schema_version": 3}).to_string(),
        )
        .expect("writes broken ledger");

        assert!(list_at(&auth)
            .expect_err("no accounts array")
            .contains("não é um registro do codex-auth válido"));
    }
}
