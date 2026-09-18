//! Turns the Command Code API payloads into the plan limits the panel shows: the plan table, the
//! usage percentage and credit math, and the field extraction from each endpoint.

use super::{CollectError, CommandCodeLimits, WindowLimit};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

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

/// Credit balance fields shared by the usage percentage and the total credit calculations.
#[derive(Debug, Clone, Copy)]
struct CreditTotals {
    monthly_remaining: f64,
    purchased: f64,
    free: f64,
}

impl CreditTotals {
    /// Clamps every field at zero so negative API values cannot skew the pool.
    fn positive(self) -> Self {
        Self {
            monthly_remaining: self.monthly_remaining.max(0.0),
            purchased: self.purchased.max(0.0),
            free: self.free.max(0.0),
        }
    }

    fn remaining(self) -> f64 {
        self.monthly_remaining + self.purchased + self.free
    }

    /// Credits available this period: the plan allowance when active, else what was spent.
    fn total_pool(self, status: Option<&str>, plan_credits: Option<f64>, total_spent: f64) -> f64 {
        match (status, plan_credits) {
            (Some("active"), Some(credits)) => {
                credits.max(self.monthly_remaining) + self.purchased + self.free
            }
            _ => total_spent.max(0.0) + self.remaining(),
        }
    }
}

fn usage_percent(
    totals: CreditTotals,
    status: Option<&str>,
    plan_credits: Option<f64>,
    total_spent: f64,
) -> f64 {
    let totals = totals.positive();
    let total_pool = totals.total_pool(status, plan_credits, total_spent);

    if total_pool <= 0.0 {
        return 0.0;
    }

    (((total_pool - totals.remaining()) / total_pool) * 100.0).clamp(0.0, 100.0)
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

/// Plan fields the subscription payload carries.
struct SubscriptionInfo {
    plan_id: Option<String>,
    status: Option<String>,
    renews_at: Option<DateTime<Utc>>,
}

/// Usage summary fields shown in the panel.
struct SummaryInfo {
    total_spent: f64,
    requests_this_period: u64,
    period_basis: Option<String>,
}

/// Parses one API payload, keeping the endpoint name in the error.
fn parse_payload(label: &str, body: &str) -> Result<Value, CollectError> {
    serde_json::from_str(body)
        .map_err(|error| CollectError::Failed(format!("{label} inválido: {error}")))
}

fn credit_totals(credits: &Value) -> Result<CreditTotals, CollectError> {
    let info = credits
        .get("credits")
        .filter(|value| !value.is_null())
        .ok_or_else(|| CollectError::Failed("resposta de credits sem saldo".to_string()))?;

    let number = |key: &str| {
        info.get(key)
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0)
    };
    Ok(CreditTotals {
        monthly_remaining: number("monthlyCredits"),
        purchased: number("purchasedCredits"),
        free: number("freeCredits"),
    })
}

fn subscription_info(subscription: &Value) -> SubscriptionInfo {
    let data = subscription.get("data").filter(|value| !value.is_null());
    let text = |key: &str| {
        data.and_then(|value| value.get(key))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    };

    SubscriptionInfo {
        plan_id: text("planId"),
        status: text("status"),
        renews_at: text("currentPeriodEnd")
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc)),
    }
}

fn summary_info(summary: &Value) -> SummaryInfo {
    SummaryInfo {
        total_spent: summary
            .get("totalCost")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0),
        requests_this_period: summary
            .get("totalCount")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        period_basis: summary
            .get("periodBasis")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    }
}

fn days_until(renews_at: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    (((renews_at - now).num_milliseconds() as f64) / 86_400_000.0)
        .ceil()
        .max(0.0) as i64
}

fn credit_windows(credits_json: &Value) -> (Option<WindowLimit>, Option<WindowLimit>) {
    let window_limits = credits_json
        .get("windowLimits")
        .filter(|value| !value.is_null());
    let read = |key: &str| {
        window_limits
            .and_then(|value| value.get(key))
            .and_then(window_limit)
    };

    (read("fiveHour"), read("weekly"))
}

/// Turns the three Command Code API payloads into the plan limits the panel shows.
pub(crate) fn parse_limits(
    summary: &str,
    credits: &str,
    subscription: &str,
    now: DateTime<Utc>,
) -> Result<CommandCodeLimits, CollectError> {
    let summary_json = parse_payload("usage/summary", summary)?;
    let credits_json = parse_payload("billing/credits", credits)?;
    let subscription_json = parse_payload("billing/subscriptions", subscription)?;

    let totals = credit_totals(&credits_json)?.positive();
    let subscription = subscription_info(&subscription_json);
    let summary = summary_info(&summary_json);

    let plan_credits = subscription
        .plan_id
        .as_deref()
        .and_then(plan_monthly_credits);
    let status = subscription.status.as_deref();
    let (five_hour, weekly) = credit_windows(&credits_json);

    Ok(CommandCodeLimits {
        plan: subscription
            .plan_id
            .as_deref()
            .and_then(plan_name)
            .map(str::to_string),
        plan_id: subscription.plan_id,
        status: subscription.status.clone(),
        usage_percent: usage_percent(totals, status, plan_credits, summary.total_spent),
        credits_total: totals.total_pool(status, plan_credits, summary.total_spent),
        credits_remaining: totals.remaining(),
        requests_this_period: summary.requests_this_period,
        period_basis: summary.period_basis,
        renews_at: subscription.renews_at,
        days_to_renew: subscription.renews_at.map(|end| days_until(end, now)),
        five_hour,
        weekly,
        fetched_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

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
}
