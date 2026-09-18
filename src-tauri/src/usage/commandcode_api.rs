use super::cache::LimitsCache;
use super::commandcode_payload::parse_limits;
use super::limits_http::{self, get_bearer_json};
use super::{CollectError, CommandCodeLimits};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://api.commandcode.ai";
pub const CACHE_TTL: Duration = Duration::from_secs(300);

static CACHE: LimitsCache<CommandCodeLimits> = LimitsCache::new();

pub fn default_auth_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".commandcode")
        .join("auth.json")
}

pub fn auth_path() -> PathBuf {
    std::env::var("CODE_USAGE_CC_AUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_auth_path())
}

pub fn base_url() -> String {
    std::env::var("CODE_USAGE_CC_API_BASE").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
}

pub fn read_token(path: &Path) -> Result<String, CollectError> {
    if !path.exists() {
        return Err(CollectError::NotFound(path.display().to_string()));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;

    value
        .get("apiKey")
        .and_then(|token| token.as_str())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| CollectError::Failed(format!("{}: sem apiKey", path.display())))
}

pub fn fetch_limits(
    client: &reqwest::blocking::Client,
    base_url: &str,
    token: &str,
    now: DateTime<Utc>,
) -> Result<CommandCodeLimits, CollectError> {
    let base = base_url.trim_end_matches('/');
    let summary = get_bearer_json(client, &format!("{base}/alpha/usage/summary"), token)?;
    let credits = get_bearer_json(client, &format!("{base}/alpha/billing/credits"), token)?;
    let subscription = get_bearer_json(
        client,
        &format!("{base}/alpha/billing/subscriptions"),
        token,
    )?;

    parse_limits(&summary, &credits, &subscription, now)
}

pub fn cached_limits() -> Option<CommandCodeLimits> {
    CACHE.last()
}

pub fn refresh_cache() -> Option<CommandCodeLimits> {
    CACHE.refresh(CACHE_TTL, || {
        let token = read_token(&auth_path()).ok()?;
        let client = limits_http::client().ok()?;
        fetch_limits(&client, &base_url(), &token, Utc::now()).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::test_server;
    use chrono::TimeZone;
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
            .join("commandcode-api")
            .join(name);
        fs::read_to_string(path).expect("fixture is readable")
    }

    #[test]
    fn reads_the_token_from_the_auth_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        fs::write(&path, fixture("auth.json")).expect("writes fixture");

        assert_eq!(read_token(&path).expect("token"), "fixture-token");
        assert!(matches!(
            read_token(&dir.path().join("missing.json")),
            Err(CollectError::NotFound(_))
        ));
    }

    #[test]
    fn fetches_and_parses_the_three_endpoints() {
        let base = test_server::spawn(vec![
            (200, fixture("summary.json")),
            (200, fixture("credits.json")),
            (200, fixture("subscription.json")),
        ]);
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 18, 0, 0).unwrap();

        let limits = fetch_limits(&test_client(), &base, "token", now).expect("limits");

        assert_eq!(limits.plan.as_deref(), Some("GOAT"));
        assert_eq!(limits.plan_id.as_deref(), Some("individual-goat"));
    }

    #[test]
    fn reports_an_endpoint_failure_with_its_url() {
        let base = test_server::spawn(vec![(500, "{}".to_string())]);
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 18, 0, 0).unwrap();

        let error = fetch_limits(&test_client(), &base, "token", now).expect_err("http error");

        assert!(
            matches!(error, CollectError::Failed(message) if message.contains("/alpha/usage/summary"))
        );
    }
}
