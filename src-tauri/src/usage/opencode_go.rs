use super::{CollectError, OpenCodeGoLimits, OpenCodeGoWindow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DEFAULT_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
pub const CACHE_TTL: Duration = Duration::from_secs(300);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct CachedLimits {
    fetched_at: Instant,
    limits: OpenCodeGoLimits,
}

static CACHE: Mutex<Option<CachedLimits>> = Mutex::new(None);

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

fn window(value: Option<&Value>) -> Option<OpenCodeGoWindow> {
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

pub fn parse_usage(body: &str, now: DateTime<Utc>) -> Result<OpenCodeGoLimits, CollectError> {
    let payload: Value = serde_json::from_str(body)
        .map_err(|error| CollectError::Failed(format!("zen/go/usage inválido: {error}")))?;
    let usage = payload
        .get("usage")
        .filter(|usage| !usage.is_null())
        .ok_or_else(|| CollectError::Failed("resposta sem usage".to_string()))?;

    Ok(OpenCodeGoLimits {
        rolling: window(usage.get("rolling")),
        weekly: window(usage.get("weekly")),
        monthly: window(usage.get("monthly")),
        fetched_at: now,
    })
}

pub fn fetch_limits(
    url: &str,
    api_key: &str,
    now: DateTime<Utc>,
) -> Result<OpenCodeGoLimits, CollectError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("code-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| CollectError::Failed(error.to_string()))?;

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|error| CollectError::Failed(format!("{url}: {error}")))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|error| CollectError::Failed(format!("{url}: {error}")))?;

    if !status.is_success() {
        return Err(CollectError::Failed(format!("{url}: HTTP {status}")));
    }

    parse_usage(&body, now)
}

fn fresh_cache() -> Option<OpenCodeGoLimits> {
    CACHE.lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|cached| cached.fetched_at.elapsed() < CACHE_TTL)
            .map(|cached| cached.limits.clone())
    })
}

pub fn cached_limits() -> Option<OpenCodeGoLimits> {
    CACHE
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|cached| cached.limits.clone()))
}

pub fn refresh_cache() -> Option<OpenCodeGoLimits> {
    if let Some(limits) = fresh_cache() {
        return Some(limits);
    }

    let api_key = read_api_key(&auth_path()).ok()?;

    match fetch_limits(&usage_url(), &api_key, Utc::now()) {
        Ok(limits) => {
            if let Ok(mut guard) = CACHE.lock() {
                *guard = Some(CachedLimits {
                    fetched_at: Instant::now(),
                    limits: limits.clone(),
                });
            }
            Some(limits)
        }
        Err(_) => cached_limits(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
}
