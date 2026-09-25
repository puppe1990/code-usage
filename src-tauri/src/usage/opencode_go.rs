//! OpenCode Go plan limits from `opencode.ai/zen/go/v1/usage`, authenticated with the
//! `opencode-go` key from `~/.local/share/opencode/auth.json` and cached for 5 minutes.

use super::cache::LimitsCache;
use super::limits_http::{self, get_bearer_json};
use super::{CollectError, OpenCodeGoLimits, OpenCodeGoWindow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
pub const CACHE_TTL: Duration = Duration::from_secs(300);

static CACHE: LimitsCache<OpenCodeGoLimits> = LimitsCache::new();

pub fn default_auth_path() -> PathBuf {
    super::opencode::default_db_path()
        .parent()
        .unwrap_or(Path::new("."))
        .join("auth.json")
}

pub fn auth_path() -> PathBuf {
    std::env::var("CODE_USAGE_OPENCODE_AUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_auth_path())
}

pub fn usage_url() -> String {
    std::env::var("CODE_USAGE_OPENCODE_GO_URL").unwrap_or_else(|_| DEFAULT_USAGE_URL.to_string())
}

pub fn read_api_key(path: &Path) -> Result<String, CollectError> {
    if !path.exists() {
        return Err(CollectError::NotFound(path.display().to_string()));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;

    let account = value.get("opencode-go").ok_or_else(|| {
        CollectError::Failed(format!("{}: sem conta opencode-go", path.display()))
    })?;

    account
        .get("key")
        .or_else(|| account.get("token"))
        .and_then(|key| key.as_str())
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| CollectError::Failed(format!("{}: sem api key", path.display())))
}

/// OpenCode credential the app bills against, shown on the panel avatar. The CLI stores no
/// email, so the key pair in `auth.json` is the closest thing to an account.
pub fn read_account(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    let credentials = value.as_object()?;

    if credentials.contains_key("opencode-go") {
        return Some("OpenCode Go".to_string());
    }
    credentials
        .contains_key("opencode")
        .then(|| "OpenCode Zen".to_string())
}

fn parse_window(value: Option<&Value>) -> Option<OpenCodeGoWindow> {
    let value = value?;
    let percent = value.get("percent").and_then(|percent| percent.as_f64())?;
    let resets_at = value
        .get("resetsAt")
        .and_then(|reset| reset.as_str())
        .and_then(|reset| DateTime::parse_from_rfc3339(reset).ok())
        .map(|reset| reset.with_timezone(&Utc))?;

    Some(OpenCodeGoWindow {
        percent: percent.clamp(0.0, 100.0),
        status: value
            .get("status")
            .and_then(|status| status.as_str())
            .map(str::to_string),
        resets_at,
    })
}

/// Reads the rolling / weekly / monthly windows out of one usage payload.
pub fn parse_usage(body: &str, now: DateTime<Utc>) -> Result<OpenCodeGoLimits, CollectError> {
    let payload: Value = serde_json::from_str(body)
        .map_err(|error| CollectError::Failed(format!("zen/go/usage inválido: {error}")))?;
    let usage = payload
        .get("usage")
        .filter(|usage| !usage.is_null())
        .ok_or_else(|| {
            let keys = payload
                .as_object()
                .map(|object| object.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            CollectError::Failed(format!("zen/go/usage sem \"usage\" (chaves: {keys:?})"))
        })?;

    Ok(OpenCodeGoLimits {
        rolling: parse_window(usage.get("rolling")),
        weekly: parse_window(usage.get("weekly")),
        monthly: parse_window(usage.get("monthly")),
        fetched_at: now,
    })
}

/// Fetches the usage endpoint and parses it (see `parse_usage`).
pub fn fetch_limits(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    now: DateTime<Utc>,
) -> Result<OpenCodeGoLimits, CollectError> {
    let body = get_bearer_json(client, url, api_key)?;

    parse_usage(&body, now)
}

/// Last fetched limits, however old; `None` before the first successful fetch.
pub fn cached_limits() -> Option<OpenCodeGoLimits> {
    CACHE.last()
}

/// Refreshes when the 5-minute cache is stale, keeping the last value on failure.
pub fn refresh_cache() -> Option<OpenCodeGoLimits> {
    CACHE.refresh(CACHE_TTL, || {
        let api_key = read_api_key(&auth_path()).ok()?;
        let client = limits_http::client().ok()?;
        fetch_limits(&client, &usage_url(), &api_key, Utc::now()).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::test_server;
    use std::fs;
    use tempfile::TempDir;

    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .expect("test client")
    }

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("opencode-go")
            .join(name);
        fs::read_to_string(path).expect("fixture is readable")
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn parses_the_three_usage_windows() {
        let limits = parse_usage(&fixture("usage.json"), fixed_now()).expect("payload is parsed");

        let rolling = limits.rolling.expect("rolling window");
        assert_eq!(rolling.percent, 11.0);
        assert_eq!(rolling.status.as_deref(), Some("ok"));
        assert_eq!(
            rolling.resets_at.to_rfc3339(),
            "2026-09-18T02:40:19.375+00:00"
        );

        let weekly = limits.weekly.expect("weekly window");
        assert_eq!(weekly.percent, 21.0);
        assert_eq!(weekly.resets_at.to_rfc3339(), "2026-09-21T00:00:00+00:00");

        let monthly = limits.monthly.expect("monthly window");
        assert_eq!(monthly.percent, 10.0);
        assert_eq!(monthly.resets_at.to_rfc3339(), "2026-10-16T12:54:30+00:00");
    }

    #[test]
    fn drops_windows_missing_percent_or_reset() {
        let limits = parse_usage(
            r#"{"usage":{"rolling":{"percent":5},"weekly":{"resetsAt":"2026-09-21T00:00:00Z"}}}"#,
            fixed_now(),
        )
        .expect("payload is parsed");

        assert!(limits.rolling.is_none());
        assert!(limits.weekly.is_none());
        assert!(limits.monthly.is_none());
    }

    #[test]
    fn clamps_percent_to_the_zero_to_hundred_range() {
        let limits = parse_usage(
            r#"{"usage":{"rolling":{"percent":140,"resetsAt":"2026-09-21T00:00:00Z"}}}"#,
            fixed_now(),
        )
        .expect("payload is parsed");

        assert_eq!(limits.rolling.expect("rolling window").percent, 100.0);
    }

    #[test]
    fn rejects_payloads_without_usage() {
        let error = parse_usage("{\"usage\":null}", fixed_now()).expect_err("usage is required");

        assert!(matches!(error, CollectError::Failed(_)));
    }

    #[test]
    fn reads_the_opencode_go_key_from_auth_json() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        fs::write(&path, fixture("auth.json")).expect("writes fixture");

        assert_eq!(read_api_key(&path).expect("api key"), "go-fixture-key");
        assert!(matches!(
            read_api_key(&dir.path().join("missing.json")),
            Err(CollectError::NotFound(_))
        ));
    }

    #[test]
    fn reads_the_account_label_from_the_auth_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        fs::write(&path, fixture("auth.json")).expect("writes fixture");

        assert_eq!(read_account(&path).as_deref(), Some("OpenCode Go"));

        fs::write(
            &path,
            r#"{"opencode":{"type":"api","key":"zen-fixture-key"}}"#,
        )
        .expect("writes auth file");
        assert_eq!(read_account(&path).as_deref(), Some("OpenCode Zen"));

        assert_eq!(read_account(&dir.path().join("missing.json")), None);
    }

    #[test]
    fn fetches_and_parses_the_usage_endpoint() {
        let base = test_server::spawn(vec![(200, fixture("usage.json"))]);

        let limits = fetch_limits(&test_client(), &base, "go-key", fixed_now()).expect("limits");

        assert_eq!(limits.rolling.expect("rolling window").percent, 11.0);
    }

    #[test]
    fn reports_an_endpoint_failure_with_its_url() {
        let base = test_server::spawn(vec![(401, "{}".to_string())]);

        let error =
            fetch_limits(&test_client(), &base, "go-key", fixed_now()).expect_err("http error");

        assert!(matches!(error, CollectError::Failed(message) if message.contains("HTTP 401")));
    }
}
