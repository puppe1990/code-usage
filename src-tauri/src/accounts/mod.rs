//! Account switching for the three harnesses. The stores are the same ones the
//! `agent-account-switchers` CLIs (`ccs`, `ocgs`, `switcher-ui`) keep, so this app is just another
//! client of those files: each harness has a ledger of saved logins plus the live credential file
//! the CLI itself reads. Everything is written atomically with mode `0600`, and a failure leaves
//! the previous files untouched.

mod commandcode;
mod grok;
mod opencode;

use crate::usage::Provider;
use serde::Serialize;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub active: bool,
}

/// Logins saved for `provider`, alphabetically, with the one in use marked.
pub fn list(provider: Provider) -> Result<Vec<Account>, String> {
    match provider {
        Provider::CommandCode => commandcode::list(),
        Provider::Grok => grok::list(),
        Provider::OpenCode => opencode::list(),
    }
}

/// Points `provider` at `name`, rewriting whichever files that store keeps live.
pub fn switch(provider: Provider, name: &str) -> Result<(), String> {
    match provider {
        Provider::CommandCode => commandcode::switch(name),
        Provider::Grok => grok::switch(name),
        Provider::OpenCode => opencode::switch(name),
    }
}

/// Reads a JSON file; `Ok(None)` when it does not exist and `Err` when it is not valid JSON.
pub(crate) fn read_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("não consegui ler {}: {error}", path.display())),
    };

    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| format!("{} não é um JSON válido: {error}", path.display()))
}

/// Writes `value` as pretty JSON through a temp file, so a crash never truncates the original.
pub(crate) fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;

    write_atomic(path, &body)
}

fn write_atomic(path: &Path, body: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("não consegui criar {}: {error}", parent.display()))?;
    }

    let temp = path.with_extension("tmp");
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(body)?;
        file.sync_all()
    };

    if let Err(error) = write() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!(
            "não consegui gravar {}: {error}. Nada foi alterado.",
            path.display()
        ));
    }

    std::fs::rename(&temp, path).map_err(|error| {
        format!(
            "não consegui gravar {}: {error}. Nada foi alterado.",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn writes_with_owner_only_permissions() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("nested").join("auth.json");

        write_json(&path, &serde_json::json!({"apiKey": "secret"})).expect("writes");

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            read_json(&path).expect("reads").expect("file"),
            serde_json::json!({"apiKey": "secret"})
        );
    }

    #[test]
    fn missing_files_read_as_none_and_broken_ones_as_errors() {
        let dir = TempDir::new().expect("temp dir");
        let missing = dir.path().join("auth.json");
        assert_eq!(read_json(&missing).expect("missing file"), None);

        std::fs::write(&missing, "{ not json").expect("writes broken file");
        assert!(read_json(&missing)
            .expect_err("broken file")
            .contains("não é um JSON válido"));
    }

    #[test]
    fn a_failed_write_leaves_the_original_file_alone() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        std::fs::write(&path, "{\"apiKey\":\"original\"}").expect("writes original");

        // a directory in the temp path position makes the write fail
        std::fs::create_dir(path.with_extension("tmp")).expect("blocks the temp file");

        assert!(write_json(&path, &serde_json::json!({"apiKey": "new"})).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("original survives"),
            "{\"apiKey\":\"original\"}"
        );
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use std::path::PathBuf;

    /// Copies the live stores to a temp dir and switches there, so the real logins are untouched.
    #[test]
    #[ignore = "copies the real switcher stores and switches in the copy"]
    fn switches_a_copy_of_the_real_stores() {
        let home = dirs::home_dir().expect("home");
        let dir = tempfile::TempDir::new().expect("temp dir");
        let data = dir.path().to_path_buf();

        std::fs::copy(home.join(".commandcode/auth.json"), data.join("auth.json"))
            .expect("copies command code auth");
        std::fs::copy(
            home.join(".commandcode/ccs-accounts.json"),
            data.join("ccs-accounts.json"),
        )
        .expect("copies the ledger");
        copy(home.join(".grok/auth.json"), data.join("grok-auth.json"));
        copy_profiles(home.join(".grok/accounts"), data.join("accounts"));
        copy(
            home.join(".local/share/opencode/auth.json"),
            data.join("opencode-auth.json"),
        );
        copy(
            home.join(".local/share/opencode/account.json"),
            data.join("account.json"),
        );

        std::env::set_var("CODE_USAGE_CC_AUTH", data.join("auth.json"));
        std::env::set_var("CODE_USAGE_GROK_AUTH", data.join("grok-auth.json"));
        std::env::set_var("CODE_USAGE_OPENCODE_AUTH", data.join("opencode-auth.json"));

        for provider in [Provider::CommandCode, Provider::Grok, Provider::OpenCode] {
            let before = list(provider).expect("lists");
            println!("{provider:?} antes: {before:?}");

            let Some(target) = before.iter().find(|account| !account.active) else {
                println!("{provider:?}: nenhuma outra conta para trocar");
                continue;
            };

            switch(provider, &target.name).expect("switches");
            let after = list(provider).expect("lists");

            println!("{provider:?} depois: {after:?}");
            assert!(after
                .iter()
                .any(|account| account.active && account.name == target.name));
            assert_eq!(after.len(), before.len(), "nenhuma conta se perde");
        }

        std::env::remove_var("CODE_USAGE_CC_AUTH");
        std::env::remove_var("CODE_USAGE_GROK_AUTH");
        std::env::remove_var("CODE_USAGE_OPENCODE_AUTH");
    }

    fn copy(from: PathBuf, to: PathBuf) {
        if from.exists() {
            std::fs::copy(&from, &to).expect("copies");
        }
    }

    fn copy_profiles(from: PathBuf, to: PathBuf) {
        std::fs::create_dir_all(&to).expect("creates profiles dir");
        for entry in std::fs::read_dir(from).expect("reads profiles").flatten() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copies profile");
        }
    }

    /// Same as `ccs list` / `ocgs list`: only reads, never switches.
    #[test]
    #[ignore = "reads the real switcher stores from this machine"]
    fn prints_the_accounts_from_real_data() {
        for provider in [Provider::CommandCode, Provider::Grok, Provider::OpenCode] {
            match list(provider) {
                Ok(accounts) => {
                    for account in accounts {
                        let marker = if account.active { "→" } else { " " };
                        println!("{}: {marker} {}", provider.display_name(), account.name);
                    }
                }
                Err(error) => println!("{}: erro: {error}", provider.display_name()),
            }
        }
    }
}
