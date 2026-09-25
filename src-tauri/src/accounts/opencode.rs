//! OpenCode Go accounts: `account.json` (v2) is the store of saved credentials and the live
//! `auth.json` holds the one the CLI uses per service. Only the `opencode-go` service is touched;
//! the account in use is the one `active["opencode-go"]` points at.

use super::{read_json, write_json, Account};
use crate::usage::opencode_go::auth_path;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const SERVICE: &str = "opencode-go";

pub fn list() -> Result<Vec<Account>, String> {
    list_at(&auth_path())
}

pub fn switch(name: &str) -> Result<(), String> {
    switch_at(&auth_path(), name)
}

fn list_at(auth: &Path) -> Result<Vec<Account>, String> {
    let store = store_at(auth)?;
    let active = store.get("active").and_then(|active| active.get(SERVICE));

    let mut accounts = store_accounts(&store)
        .map(|(id, account)| Account {
            name: description(account),
            active: active.and_then(Value::as_str) == Some(id.as_str()),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(accounts)
}

fn switch_at(auth: &Path, name: &str) -> Result<(), String> {
    let mut store = store_at(auth)?;
    let found = store_accounts(&store)
        .filter(|(_, account)| description(account) == name)
        .map(|(id, account)| (id.clone(), account.clone()))
        .collect::<Vec<_>>();

    let (id, account) = match found.len() {
        0 => return Err(not_found(name, &store)),
        1 => found.into_iter().next().expect("one account"),
        count => {
            return Err(format!(
            "há {count} contas Go chamadas {name:?}. Renomeie a ativa com ocgs save <nome-único>."
        ))
        }
    };

    let mut credentials = live_credentials(auth)?;
    credentials[SERVICE] = account
        .get("credential")
        .cloned()
        .ok_or_else(|| format!("conta {name:?} não tem credencial salva"))?;

    store["active"][SERVICE] = json!(id);
    write_json(&store_path(auth), &store)?;

    write_json(auth, &Value::Object(credentials))
}

/// The saved store, refusing anything that is not the v2 shape the CLIs write.
fn store_at(auth: &Path) -> Result<Value, String> {
    let path = store_path(auth);
    let store = read_json(&path)?.ok_or_else(|| missing(auth))?;

    if store.get("version").and_then(Value::as_u64) != Some(2) {
        return Err(format!(
            "{} não é um JSON v2 válido: version {:?}",
            path.display(),
            store.get("version").unwrap_or(&Value::Null)
        ));
    }

    Ok(store)
}

/// The live `auth.json`, which the CLI keeps as a map of service id to credentials.
fn live_credentials(auth: &Path) -> Result<serde_json::Map<String, Value>, String> {
    let value = read_json(auth)?.ok_or_else(|| missing(auth))?;

    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{} não é um JSON válido", auth.display()))
}

fn store_accounts(store: &Value) -> impl Iterator<Item = (&String, &Value)> {
    store
        .get("accounts")
        .and_then(Value::as_object)
        .map(|accounts| accounts.iter())
        .into_iter()
        .flatten()
        .filter(|(_, account)| account.get("serviceID").and_then(Value::as_str) == Some(SERVICE))
}

fn description(account: &Value) -> String {
    account
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn store_path(auth: &Path) -> PathBuf {
    auth.with_file_name("account.json")
}

fn missing(auth: &Path) -> String {
    let dir = auth.parent().unwrap_or(Path::new("."));
    format!(
        "não achei o store do OpenCode em {}. Abra o OpenCode uma vez ou rode opencode auth login.",
        dir.display()
    )
}

fn not_found(name: &str, store: &Value) -> String {
    let mut names = store_accounts(store)
        .map(|(_, account)| description(account))
        .collect::<Vec<_>>();
    names.sort();

    if names.is_empty() {
        return format!("conta {name:?} não existe. não há contas Go");
    }
    format!(
        "conta {name:?} não existe. Contas Go: {}.",
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn store_fixture(auth: &Path) {
        fs::write(
            store_path(auth),
            json!({
                "version": 2,
                "accounts": {
                    "id-a": {"id": "id-a", "serviceID": "opencode-go", "description": "pessoal",
                             "credential": {"type": "api", "key": "go-personal"}},
                    "id-b": {"id": "id-b", "serviceID": "opencode-go", "description": "trabalho",
                             "credential": {"type": "api", "key": "go-work"}},
                    "id-c": {"id": "id-c", "serviceID": "opencode", "description": "zen",
                             "credential": {"type": "api", "key": "zen"}},
                },
                "active": {"opencode-go": "id-a", "opencode": "id-c"},
            })
            .to_string(),
        )
        .expect("writes store");
    }

    fn auth_fixture(auth: &Path) {
        fs::write(
            auth,
            json!({
                "opencode-go": {"type": "api", "key": "go-personal"},
                "opencode": {"type": "api", "key": "zen"},
            })
            .to_string(),
        )
        .expect("writes auth");
    }

    #[test]
    fn lists_only_the_go_accounts_with_the_active_one_marked() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");
        store_fixture(&auth);
        auth_fixture(&auth);

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
    fn switching_moves_the_active_id_and_the_live_credential() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");
        store_fixture(&auth);
        auth_fixture(&auth);

        switch_at(&auth, "trabalho").expect("switches");

        let store: Value =
            serde_json::from_str(&fs::read_to_string(store_path(&auth)).expect("store")).unwrap();
        assert_eq!(store["active"]["opencode-go"], json!("id-b"));
        assert_eq!(
            store["accounts"]["id-c"]["credential"]["key"],
            json!("zen"),
            "other providers keep their credentials"
        );

        let live: Value = serde_json::from_str(&fs::read_to_string(&auth).expect("auth")).unwrap();
        assert_eq!(live["opencode-go"]["key"], json!("go-work"));
        assert_eq!(live["opencode"]["key"], json!("zen"));
    }

    #[test]
    fn a_store_that_is_not_v2_is_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");
        fs::write(
            store_path(&auth),
            json!({"version": 1, "accounts": {}}).to_string(),
        )
        .expect("writes store");
        auth_fixture(&auth);

        assert!(list_at(&auth)
            .expect_err("version 1")
            .contains("não é um JSON v2 válido"));
    }

    #[test]
    fn a_missing_store_says_what_to_do() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");

        let error = list_at(&auth).expect_err("missing store");

        assert!(error.contains("não achei o store do OpenCode"));
        assert!(error.contains("opencode auth login"));
    }

    #[test]
    fn duplicate_names_are_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");
        fs::write(
            store_path(&auth),
            json!({
                "version": 2,
                "accounts": {
                    "a": {"id": "a", "serviceID": "opencode-go", "description": "default", "credential": {"key": "1"}},
                    "b": {"id": "b", "serviceID": "opencode-go", "description": "default", "credential": {"key": "2"}},
                },
                "active": {"opencode-go": "a"},
            })
            .to_string(),
        )
        .expect("writes store");
        auth_fixture(&auth);

        assert!(switch_at(&auth, "default")
            .expect_err("duplicate")
            .contains("há 2 contas Go chamadas \"default\""));
    }

    #[test]
    fn switching_without_a_live_auth_file_is_refused() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");
        store_fixture(&auth);

        assert!(switch_at(&auth, "trabalho")
            .expect_err("no auth file")
            .contains("não achei o store do OpenCode"));
    }
}
