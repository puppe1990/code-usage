//! Command Code accounts, the same pair `ccs` keeps: `ccs-accounts.json` is the ledger of saved
//! logins (one `Auth` per name plus the `active` name) and `auth.json` is the live login the `cmd`
//! CLI reads. The account in use is the ledger one whose `apiKey` matches the live file.

use super::{read_json, write_json, Account};
use crate::usage::commandcode_api::auth_path;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MISSING_LEDGER: &str = "não há contas Command Code";

pub fn list() -> Result<Vec<Account>, String> {
    list_at(&auth_path())
}

pub fn switch(name: &str) -> Result<(), String> {
    switch_at(&auth_path(), name)
}

fn list_at(auth: &Path) -> Result<Vec<Account>, String> {
    let ledger = ledger_at(auth)?;
    let live_key = live_key(auth);

    let mut accounts = entries(&ledger)
        .map(|(name, account)| Account {
            name: name.clone(),
            active: live_key.is_some()
                && live_key.as_deref() == account.get("apiKey").and_then(Value::as_str),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(accounts)
}

/// Rewrites the ledger's `active` field and then the live `auth.json`, in that order, exactly like
/// `ccs switch`.
fn switch_at(auth: &Path, name: &str) -> Result<(), String> {
    let mut ledger = ledger_at(auth)?;
    let account = ledger
        .get("accounts")
        .and_then(|accounts| accounts.get(name))
        .cloned()
        .ok_or_else(|| not_found(name, &ledger))?;

    ledger["active"] = json!(name);
    write_json(&ledger_path_for(auth), &ledger)?;

    write_json(auth, &account)
}

fn ledger_path_for(auth: &Path) -> PathBuf {
    auth.with_file_name("ccs-accounts.json")
}

/// The ledger, or an empty one when the file does not exist yet.
fn ledger_at(auth: &Path) -> Result<Value, String> {
    Ok(read_json(&ledger_path_for(auth))?.unwrap_or_else(|| json!({"version": 1, "accounts": {}})))
}

fn entries(ledger: &Value) -> impl Iterator<Item = (&String, &Value)> {
    ledger
        .get("accounts")
        .and_then(Value::as_object)
        .map(|accounts| accounts.iter())
        .into_iter()
        .flatten()
}

fn live_key(auth: &Path) -> Option<String> {
    read_json(auth)
        .ok()
        .flatten()?
        .get("apiKey")
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

fn not_found(name: &str, ledger: &Value) -> String {
    let mut names = entries(ledger)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    names.sort();

    if names.is_empty() {
        return format!("conta {name:?} não existe. {MISSING_LEDGER}");
    }
    format!(
        "conta {name:?} não existe. Contas Command Code: {}.",
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn auth_path_in(dir: &TempDir) -> PathBuf {
        dir.path().join("auth.json")
    }

    fn ledger_fixture(auth: &Path) {
        fs::write(
            ledger_path_for(auth),
            json!({
                "version": 1,
                "accounts": {
                    "pessoal": {"apiKey": "key-personal", "userName": "pessoal"},
                    "trabalho": {"apiKey": "key-work", "userName": "trabalho", "userId": "u2"},
                },
                "active": "trabalho",
            })
            .to_string(),
        )
        .expect("writes ledger");
    }

    #[test]
    fn lists_the_ledger_accounts_with_the_live_one_active() {
        let dir = TempDir::new().expect("temp dir");
        let auth = auth_path_in(&dir);
        ledger_fixture(&auth);
        fs::write(&auth, json!({"apiKey": "key-personal"}).to_string()).expect("writes live");

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
    fn switching_writes_the_ledger_then_the_live_auth_file() {
        let dir = TempDir::new().expect("temp dir");
        let auth = auth_path_in(&dir);
        ledger_fixture(&auth);
        fs::write(&auth, json!({"apiKey": "key-work"}).to_string()).expect("writes live");

        switch_at(&auth, "pessoal").expect("switches");

        let ledger: Value =
            serde_json::from_str(&fs::read_to_string(ledger_path_for(&auth)).expect("ledger"))
                .expect("valid ledger");
        assert_eq!(ledger["active"], json!("pessoal"));
        assert_eq!(ledger["accounts"]["trabalho"]["apiKey"], json!("key-work"));

        let live: Value = serde_json::from_str(&fs::read_to_string(&auth).expect("live")).unwrap();
        assert_eq!(live["apiKey"], json!("key-personal"));
        assert_eq!(live["userName"], json!("pessoal"));
    }

    #[test]
    fn an_unknown_account_lists_the_ones_that_exist() {
        let dir = TempDir::new().expect("temp dir");
        let auth = auth_path_in(&dir);
        ledger_fixture(&auth);

        let error = switch_at(&auth, "nope").expect_err("unknown account");

        assert!(error.contains("conta \"nope\" não existe"));
        assert!(error.contains("pessoal, trabalho"));
        assert!(
            !auth.exists(),
            "nothing is written when the account is missing"
        );
    }

    #[test]
    fn a_missing_ledger_lists_nothing_and_refuses_to_switch() {
        let dir = TempDir::new().expect("temp dir");
        let auth = auth_path_in(&dir);

        assert_eq!(list_at(&auth).expect("lists"), Vec::new());
        assert!(switch_at(&auth, "pessoal")
            .expect_err("no accounts")
            .contains(MISSING_LEDGER));
    }
}
