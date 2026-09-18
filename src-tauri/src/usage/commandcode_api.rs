use super::{CollectError, CommandCodeLimits, WindowLimit};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DEFAULT_BASE_URL: &str = "https://api.commandcode.ai";
pub const CACHE_TTL: Duration = Duration::from_secs(300);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

const PLAN_TABLE: &[(&str, &str, f64)] = &[
    ("individual-ultra", "Ultra", 300.0),
    ("individual-max", "Max", 150.0),
    ("individual-pro-v1", "Pro", 80.0),
    ("individual-goat", "GOAT", 70.0),
    ("teams-pro", "Teams Pro", 40.0),
    ("individual-pro", "Pro", 30.0),
    ("individual-provider", "Provider", 15.0),
    ("individual-go", "Go", 10.0),
];

#[derive(Debug, Clone)]
struct CachedLimits {
    fetched_at: Instant,
    limits: CommandCodeLimits,
}

static CACHE: Mutex<Option<CachedLimits>> = Mutex::new(None);

pub fn plan_name(plan_id: &str) -> Option<&'static str> {
    PLAN_TABLE
        .iter()
        .find(|(id, _, _)| *id == plan_id)
        .map(|(_, name, _)| *name)
}

pub fn plan_monthly_credits(plan_id: &str) -> Option<f64> {
    PLAN_TABLE
        .iter()
        .find(|(id, _, _)| *id == plan_id)
        .map(|(_, _, credits)| *credits)
}

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

pub fn usage_percent(
    plan_credits: Option<f64>,
    status: Option<&str>,
    monthly_remaining: f64,
    purchased: f64,
    free: f64,
    total_spent: f64,
) -> f64 {
    let monthly = monthly_remaining.max(0.0);
    let purchased = purchased.max(0.0);
    let free = free.max(0.0);
    let remaining = monthly + purchased + free;

    let active_plan_credits = if status == Some("active") {
        plan_credits
    } else {
        None
    };

    let total_pool = match active_plan_credits {
        Some(credits) => credits.max(monthly) + purchased + free,
        None => total_spent.max(0.0) + remaining,
    };

    if total_pool <= 0.0 {
        return 0.0;
    }

    (((total_pool - remaining) / total_pool) * 100.0).clamp(0.0, 100.0)
}

fn window_limit(value: &Value) -> Option<WindowLimit> {
    let used = value.get("used").and_then(|value| value.as_f64())?;
    let cap = value
        .get("cap")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let reset_at = Utc
        .timestamp_millis_opt(value.get("resetAt")?.as_i64()?)
        .single()?;

    Some(WindowLimit {
        percent_used: if cap > 0.0 {
            (used / cap * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        },
        used,
        cap,
        reset_at,
    })
}

fn parse_credits_total(
    status: Option<&str>,
    plan_credits: Option<f64>,
    monthly_remaining: f64,
    purchased: f64,
    free: f64,
    total_spent: f64,
) -> f64 {
    let monthly = monthly_remaining.max(0.0);
    match (status, plan_credits) {
        (Some("active"), Some(credits)) => {
            credits.max(monthly) + purchased.max(0.0) + free.max(0.0)
        }
        _ => total_spent.max(0.0) + monthly + purchased.max(0.0) + free.max(0.0),
    }
}

pub fn parse_limits(
    summary: &str,
    credits: &str,
    subscription: &str,
    now: DateTime<Utc>,
) -> Result<CommandCodeLimits, CollectError> {
    let summary: Value = serde_json::from_str(summary)
        .map_err(|error| CollectError::Failed(format!("usage/summary inválido: {error}")))?;
    let credits: Value = serde_json::from_str(credits)
        .map_err(|error| CollectError::Failed(format!("billing/credits inválido: {error}")))?;
    let subscription: Value = serde_json::from_str(subscription).map_err(|error| {
        CollectError::Failed(format!("billing/subscriptions inválido: {error}"))
    })?;

    let credit_info = credits
        .get("credits")
        .filter(|value| !value.is_null())
        .ok_or_else(|| CollectError::Failed("resposta de credits sem saldo".to_string()))?;

    let monthly_remaining = credit_info
        .get("monthlyCredits")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let purchased = credit_info
        .get("purchasedCredits")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let free = credit_info
        .get("freeCredits")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);

    let subscription_data = subscription.get("data").filter(|value| !value.is_null());
    let plan_id = subscription_data
        .and_then(|value| value.get("planId"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let status = subscription_data
        .and_then(|value| value.get("status"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let renews_at = subscription_data
        .and_then(|value| value.get("currentPeriodEnd"))
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));

    let total_spent = summary
        .get("totalCost")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let requests_this_period = summary
        .get("totalCount")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let period_basis = summary
        .get("periodBasis")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let plan_credits = plan_id.as_deref().and_then(plan_monthly_credits);
    let days_to_renew = renews_at.map(|end| {
        (((end - now).num_milliseconds() as f64) / 86_400_000.0)
            .ceil()
            .max(0.0) as i64
    });

    let window_limits = credits.get("windowLimits").filter(|value| !value.is_null());

    Ok(CommandCodeLimits {
        plan: plan_id.as_deref().and_then(plan_name).map(str::to_string),
        plan_id,
        status: status.clone(),
        usage_percent: usage_percent(
            plan_credits,
            status.as_deref(),
            monthly_remaining,
            purchased,
            free,
            total_spent,
        ),
        credits_total: parse_credits_total(
            status.as_deref(),
            plan_credits,
            monthly_remaining,
            purchased,
            free,
            total_spent,
        ),
        credits_remaining: monthly_remaining.max(0.0) + purchased.max(0.0) + free.max(0.0),
        requests_this_period,
        period_basis,
        renews_at,
        days_to_renew,
        five_hour: window_limits
            .and_then(|value| value.get("fiveHour"))
            .and_then(window_limit),
        weekly: window_limits
            .and_then(|value| value.get("weekly"))
            .and_then(window_limit),
        fetched_at: now,
    })
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

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 17, 18, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn maps_plan_limits_and_renewal_from_api_payloads() {
        let limits = parse_limits(
            &fixture("summary.json"),
            &fixture("credits.json"),
            &fixture("subscription.json"),
            fixed_now(),
        )
        .expect("payloads are parsed");

        assert_eq!(limits.plan.as_deref(), Some("GOAT"));
        assert_eq!(limits.plan_id.as_deref(), Some("individual-goat"));
        assert_eq!(limits.status.as_deref(), Some("active"));
        assert!((limits.usage_percent - 44.9347644314).abs() < 1e-6);
        assert!((limits.credits_total - 70.0).abs() < 1e-9);
        assert!((limits.credits_remaining - 38.5456648998).abs() < 1e-9);
        assert_eq!(limits.requests_this_period, 7776);
        assert_eq!(limits.period_basis.as_deref(), Some("billing-period"));
        assert_eq!(limits.days_to_renew, Some(23));
        assert_eq!(
            limits.renews_at.map(|value| value.to_rfc3339()),
            Some("2026-10-10T15:54:42+00:00".to_string())
        );

        let five_hour = limits.five_hour.expect("five hour window");
        assert!((five_hour.percent_used - (0.433224008 / 14.0 * 100.0)).abs() < 1e-9);
        assert_eq!(five_hour.reset_at.timestamp_millis(), 1789701545210);

        let weekly = limits.weekly.expect("weekly window");
        assert!((weekly.percent_used - (0.6025898049 / 35.0 * 100.0)).abs() < 1e-9);
        assert_eq!(weekly.reset_at.timestamp_millis(), 1790266576342);
    }

    #[test]
    fn falls_back_to_the_spent_pool_when_plan_is_inactive() {
        let subscription = fixture("subscription.json").replace("\"active\"", "\"canceled\"");

        let limits = parse_limits(
            &fixture("summary.json"),
            &fixture("credits.json"),
            &subscription,
            fixed_now(),
        )
        .expect("payloads are parsed");

        assert_eq!(limits.status.as_deref(), Some("canceled"));
        let pool = 34.1027526312 + 38.5456648998;
        assert!((limits.credits_total - pool).abs() < 1e-9);
        assert!((limits.usage_percent - (34.1027526312 / pool * 100.0)).abs() < 1e-9);
    }

    #[test]
    fn rejects_payloads_without_credits() {
        let error = parse_limits(
            &fixture("summary.json"),
            "{\"credits\":null}",
            &fixture("subscription.json"),
            fixed_now(),
        )
        .expect_err("credits are required");

        assert!(matches!(error, CollectError::Failed(_)));
    }

    #[test]
    fn maps_plan_ids_to_names_and_credits() {
        assert_eq!(plan_name("individual-goat"), Some("GOAT"));
        assert_eq!(plan_monthly_credits("individual-goat"), Some(70.0));
        assert_eq!(plan_name("individual-max"), Some("Max"));
        assert_eq!(plan_monthly_credits("teams-pro"), Some(40.0));
        assert_eq!(plan_name("individual-unknown"), None);
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
