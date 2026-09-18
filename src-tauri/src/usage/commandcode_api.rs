use super::commandcode_limits::parse_limits;
use super::{CollectError, CommandCodeLimits};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DEFAULT_BASE_URL: &str = "https://api.commandcode.ai";
pub const CACHE_TTL: Duration = Duration::from_secs(300);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct CachedLimits {
    fetched_at: Instant,
    limits: CommandCodeLimits,
}

static CACHE: Mutex<Option<CachedLimits>> = Mutex::new(None);

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

fn get_json(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<String, CollectError> {
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
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

    Ok(body)
}

pub fn fetch_limits(
    base_url: &str,
    token: &str,
    now: DateTime<Utc>,
) -> Result<CommandCodeLimits, CollectError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("code-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| CollectError::Failed(error.to_string()))?;

    let base = base_url.trim_end_matches('/');
    let summary = get_json(&client, &format!("{base}/alpha/usage/summary"), token)?;
    let credits = get_json(&client, &format!("{base}/alpha/billing/credits"), token)?;
    let subscription = get_json(
        &client,
        &format!("{base}/alpha/billing/subscriptions"),
        token,
    )?;

    parse_limits(&summary, &credits, &subscription, now)
}

fn fresh_cache() -> Option<CommandCodeLimits> {
    CACHE.lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|cached| cached.fetched_at.elapsed() < CACHE_TTL)
            .map(|cached| cached.limits.clone())
    })
}

pub fn cached_limits() -> Option<CommandCodeLimits> {
    CACHE
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|cached| cached.limits.clone()))
}

pub fn refresh_cache() -> Option<CommandCodeLimits> {
    if let Some(limits) = fresh_cache() {
        return Some(limits);
    }

    let token = read_token(&auth_path()).ok()?;

    match fetch_limits(&base_url(), &token, Utc::now()) {
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
}
