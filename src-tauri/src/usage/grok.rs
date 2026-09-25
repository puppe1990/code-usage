//! Grok CLI log (`~/.grok/logs/unified.jsonl`): billing events carry the weekly percentage,
//! `shell.turn.inference_done` carries tokens.

use super::{CollectError, GrokLimits, TokenTotals, UsageRecord};
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum GrokEvent {
    Billing(GrokLimits),
    Inference(UsageRecord),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrokData {
    pub records: Vec<UsageRecord>,
    pub limits: Option<GrokLimits>,
}

pub fn default_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".grok")
        .join("logs")
        .join("unified.jsonl")
}

pub fn log_path() -> PathBuf {
    std::env::var("CODE_USAGE_GROK_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_log_path())
}

pub fn default_auth_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".grok")
        .join("auth.json")
}

pub fn auth_path() -> PathBuf {
    std::env::var("CODE_USAGE_GROK_AUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_auth_path())
}

/// Email of the newest credential in `auth.json`, shown on the panel avatar. Saved logins can
/// pile up in that file, so the most recent `create_time` wins.
pub fn read_account(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut newest: Option<(Option<DateTime<Utc>>, String)> = None;
    for credential in value.as_object()?.values() {
        let Some(email) = credential
            .get("email")
            .and_then(|email| email.as_str())
            .filter(|email| !email.is_empty())
        else {
            continue;
        };
        let created = credential
            .get("create_time")
            .and_then(|time| time.as_str())
            .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
            .map(|time| time.with_timezone(&Utc));

        if newest
            .as_ref()
            .map(|(current, _)| created > *current)
            .unwrap_or(true)
        {
            newest = Some((created, email.to_string()));
        }
    }

    newest.map(|(_, email)| email)
}

pub fn parse_line(line: &str) -> Option<GrokEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let message = value.get("msg")?.as_str()?;
    let timestamp = DateTime::parse_from_rfc3339(value.get("ts")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);

    match message {
        "billing: fetched credits config" => billing_event(value.get("ctx")?, timestamp),
        "shell.turn.inference_done" => inference_event(value.get("ctx")?, timestamp),
        _ => None,
    }
}

/// `billing: fetched credits config` carries the weekly percentage and its period. Some accounts
/// (e.g. `SuperGrok Plus`) omit the percentage, so it stays `None` instead of reusing whichever
/// account logged it last.
fn billing_event(context: &serde_json::Value, timestamp: DateTime<Utc>) -> Option<GrokEvent> {
    let config = context.get("config")?;
    let period = config.get("currentPeriod")?;

    Some(GrokEvent::Billing(GrokLimits {
        credit_usage_percent: config
            .get("creditUsagePercent")
            .and_then(|value| value.as_f64()),
        period_start: DateTime::parse_from_rfc3339(period.get("start")?.as_str()?)
            .ok()?
            .with_timezone(&Utc),
        period_end: DateTime::parse_from_rfc3339(period.get("end")?.as_str()?)
            .ok()?
            .with_timezone(&Utc),
        tier: context
            .get("subscriptionTier")
            .and_then(|tier| tier.as_str())
            .map(str::to_string),
        fetched_at: timestamp,
    }))
}

/// `shell.turn.inference_done` carries the token counts of one turn.
fn inference_event(context: &serde_json::Value, timestamp: DateTime<Utc>) -> Option<GrokEvent> {
    let count = |key: &str| {
        context
            .get(key)
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    };
    let prompt = count("prompt_tokens");
    let cached = count("cached_prompt_tokens");
    let completion = count("completion_tokens");

    if prompt == 0 && completion == 0 {
        return None;
    }

    Some(GrokEvent::Inference(UsageRecord {
        timestamp,
        tokens: TokenTotals {
            input: prompt.saturating_sub(cached),
            output: completion,
            cache_read: cached,
            ..TokenTotals::default()
        },
        cost_usd: 0.0,
    }))
}

/// Folds one log line into the running records and the latest billing limits.
fn apply_event(
    event: Option<GrokEvent>,
    since: DateTime<Utc>,
    records: &mut Vec<UsageRecord>,
    limits: &mut Option<GrokLimits>,
) {
    match event {
        Some(GrokEvent::Billing(candidate)) => {
            let is_newer = limits
                .as_ref()
                .map(|current| candidate.fetched_at >= current.fetched_at)
                .unwrap_or(true);
            if is_newer {
                *limits = Some(candidate);
            }
        }
        Some(GrokEvent::Inference(record)) if record.timestamp >= since => records.push(record),
        Some(GrokEvent::Inference(_)) | None => {}
    }
}

/// Reads the CLI log from `since` on, into records plus the latest billing limits.
pub fn collect(path: &Path, since: DateTime<Utc>) -> Result<GrokData, CollectError> {
    if !path.exists() {
        return Err(CollectError::NotFound(path.display().to_string()));
    }

    let file = File::open(path)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;

    let mut records = Vec::new();
    let mut limits: Option<GrokLimits> = None;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        apply_event(parse_line(&line), since, &mut records, &mut limits);
    }

    Ok(GrokData { records, limits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::window;
    use chrono::Local;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::TempDir;

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("grok")
            .join("unified.jsonl")
    }

    fn fixed_now() -> DateTime<Local> {
        Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Local)
    }

    fn since() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 17, 0, 0, 0).single().unwrap()
    }

    #[test]
    fn parses_last_valid_billing_event() {
        let data = collect(&fixture_path(), since()).expect("fixture log is readable");

        let limits = data.limits.expect("billing limits");
        assert_eq!(limits.credit_usage_percent, Some(46.0));
        assert_eq!(
            limits.period_start.to_rfc3339(),
            "2026-09-17T13:25:23.983555+00:00"
        );
        assert_eq!(
            limits.period_end.to_rfc3339(),
            "2026-09-24T13:25:23.983555+00:00"
        );
        assert_eq!(limits.tier.as_deref(), Some("SuperGrok Plus"));
        assert_eq!(
            limits.fetched_at.to_rfc3339(),
            "2026-09-17T14:22:36.406+00:00"
        );
    }

    #[test]
    fn keeps_the_active_account_when_the_newest_config_omits_the_percentage() {
        let path = fixture_path().with_file_name("switched-account.jsonl");
        let data = collect(&path, since()).expect("fixture log is readable");

        let limits = data.limits.expect("billing limits");
        assert_eq!(limits.credit_usage_percent, None);
        assert_eq!(limits.tier.as_deref(), Some("SuperGrok Plus"));
        assert_eq!(
            limits.fetched_at.to_rfc3339(),
            "2026-09-18T12:43:06.588+00:00"
        );
        assert_eq!(
            limits.period_start.to_rfc3339(),
            "2026-09-18T12:30:51.769833+00:00"
        );
    }

    #[test]
    fn parses_inference_records_with_cache_split() {
        let data = collect(&fixture_path(), since()).unwrap();

        assert_eq!(data.records.len(), 3);
        let tokens: TokenTotals =
            data.records
                .iter()
                .fold(TokenTotals::default(), |mut acc, record| {
                    acc.add(&record.tokens);
                    acc
                });
        assert_eq!(tokens.input, 2000);
        assert_eq!(tokens.cache_read, 1500);
        assert_eq!(tokens.output, 200);
    }

    #[test]
    fn filters_records_older_than_since() {
        let recent_since = Utc.with_ymd_and_hms(2026, 9, 17, 0, 0, 0).single().unwrap();
        let data = collect(&fixture_path(), recent_since).unwrap();
        assert_eq!(data.records.len(), 2);
        assert!(data
            .records
            .iter()
            .all(|record| record.timestamp >= recent_since));
    }

    #[test]
    fn reads_the_newest_account_email() {
        let path = fixture_path().with_file_name("auth.json");

        assert_eq!(
            read_account(&path).as_deref(),
            Some("new-fixture@example.com")
        );
        assert_eq!(
            read_account(&fixture_path().with_file_name("missing.json")),
            None
        );
    }

    #[test]
    fn ignores_credentials_without_an_email() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        fs::write(
            &path,
            r#"{"https://auth.x.ai::client":{"auth_mode":"oidc","user_id":"user"}}"#,
        )
        .expect("writes auth file");

        assert_eq!(read_account(&path), None);
    }

    #[test]
    fn ignores_unrelated_and_malformed_lines() {
        assert!(parse_line(
            r#"{"ts":"2026-09-17T13:00:05Z","msg":"slash.advertise","ctx":{"count":135}}"#
        )
        .is_none());
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"ts":"2026-09-17T14:30:00Z","msg":"billing: fetched credits config","ctx":{"config":{"creditUsagePercent":46.0}}}"#).is_none());
    }

    #[test]
    fn collects_records_into_windows() {
        let data = collect(&fixture_path(), since()).unwrap();
        let windows = window::summarize(&data.records, fixed_now());

        assert_eq!(windows.today.records, 2);
        assert!((windows.last_30d.cost_usd - 0.0).abs() < 1e-9);
        assert_eq!(windows.last_30d.tokens.total(), 3700);
    }

    #[test]
    fn missing_log_reports_not_found() {
        let missing = fixture_path().with_file_name("missing.jsonl");
        let error = collect(&missing, since()).expect_err("missing log");
        assert_eq!(error, CollectError::NotFound(missing.display().to_string()));
    }
}
