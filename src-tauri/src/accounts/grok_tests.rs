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
    assert!(list_at(&auth)
        .expect("lists")
        .iter()
        .any(|account| account.name == "trabalho" && account.active));
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

#[test]
fn selecting_active_profile_preserves_renewed_credentials() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "stale");
    let auth = live(&dir, "work@example.com", "renewed");
    let before = fs::read(&auth).unwrap();
    switch_at(&auth, "work").unwrap();
    assert_eq!(fs::read(&auth).unwrap(), before);
}

#[test]
fn switching_away_and_back_restores_renewed_credentials() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "stale");
    profile(
        &dir,
        "personal.json",
        "personal",
        "me@example.com",
        "personal",
    );
    let auth = live(&dir, "work@example.com", "renewed");
    switch_at(&auth, "personal").unwrap();
    switch_at(&auth, "work").unwrap();
    let restored = read_json(&auth).unwrap().unwrap();
    assert_eq!(restored["https://auth.x.ai::client"]["key"], "renewed");
}

#[test]
fn snapshots_preserve_metadata_and_skip_unchanged_credentials() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "old");
    let path = dir.path().join("accounts/work.json");
    let mut saved = read_json(&path).unwrap().unwrap();
    saved["custom_metadata"] = json!({"keep": true});
    write_json(&path, &saved).unwrap();
    let auth = live(&dir, "work@example.com", "renewed");
    let mut current = read_json(&auth).unwrap().unwrap();
    current["https://auth.x.ai::client"]["refresh_token"] = json!("renewed-refresh");
    current["unknown_entry"] = json!({"field": "preserved"});
    write_json(&auth, &current).unwrap();
    snapshot_live(&auth, &profiles(&auth).unwrap(), Some(&current)).unwrap();
    let updated = read_json(&path).unwrap().unwrap();
    assert_eq!(updated["custom_metadata"], saved["custom_metadata"]);
    assert_eq!(updated["auth"], current);
    let before = fs::read(&path).unwrap();
    snapshot_live(&auth, &profiles(&auth).unwrap(), Some(&current)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_file(&auth).unwrap();
    snapshot_live(&auth, &profiles(&auth).unwrap(), None).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn an_unsaved_account_can_be_restored_after_switching() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "work");
    let auth = live(&dir, "unsaved@example.com", "renewed");
    switch_at(&auth, "work").unwrap();
    switch_at(&auth, "unsaved@example.com").unwrap();
    let restored = read_json(&auth).unwrap().unwrap();
    assert_eq!(restored["https://auth.x.ai::client"]["key"], "renewed");
}

#[test]
fn failure_to_save_current_profile_does_not_change_live_credentials() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "old");
    profile(&dir, "other.json", "other", "other@example.com", "other");
    let auth = live(&dir, "work@example.com", "renewed");
    let before = fs::read(&auth).unwrap();
    fs::create_dir(dir.path().join("accounts/work.tmp")).unwrap();
    assert!(switch_at(&auth, "other").is_err());
    assert_eq!(fs::read(&auth).unwrap(), before);
}

#[test]
fn invalid_or_unrefreshable_profiles_are_refused_without_changing_the_session() {
    for credentials in [
        json!({}),
        json!({"k": {"email": "wrong@example.com"}}),
        json!({"k": {"email": "work@example.com", "expires_at": "2000-01-01T00:00:00Z"}}),
    ] {
        let dir = TempDir::new().unwrap();
        profile(&dir, "work.json", "work", "work@example.com", "old");
        let path = dir.path().join("accounts/work.json");
        let mut saved = read_json(&path).unwrap().unwrap();
        saved["auth"] = credentials;
        write_json(&path, &saved).unwrap();
        let auth = live(&dir, "me@example.com", "valid");
        let before = fs::read(&auth).unwrap();
        assert!(switch_at(&auth, "work").is_err());
        assert_eq!(fs::read(&auth).unwrap(), before);
    }
}

#[test]
fn an_expired_refreshable_token_can_still_be_selected() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "old");
    let path = dir.path().join("accounts/work.json");
    let mut saved = read_json(&path).unwrap().unwrap();
    saved["auth"]["https://auth.x.ai::client"]["expires_at"] = json!("2000-01-01T00:00:00Z");
    saved["auth"]["https://auth.x.ai::client"]["refresh_token"] = json!("refresh");
    write_json(&path, &saved).unwrap();
    let auth = live(&dir, "me@example.com", "valid");
    switch_at(&auth, "work").unwrap();
    assert_eq!(read_json(&auth).unwrap().unwrap(), saved["auth"]);
}

#[test]
fn a_corrupt_live_session_is_not_overwritten() {
    let dir = TempDir::new().unwrap();
    profile(&dir, "work.json", "work", "work@example.com", "work");
    let auth = dir.path().join("auth.json");
    for body in ["invalid-json", "{}"] {
        fs::write(&auth, body).unwrap();
        assert!(switch_at(&auth, "work").is_err());
        assert_eq!(fs::read_to_string(&auth).unwrap(), body);
    }
}
