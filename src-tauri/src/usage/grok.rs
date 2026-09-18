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

pub fn parse_line(line: &str) -> Option<GrokEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let message = value.get("msg")?.as_str()?;
    let timestamp = DateTime::parse_from_rfc3339(value.get("ts")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);

    match message {
        "billing: fetched credits config" => {
            let context = value.get("ctx")?;
            let config = context.get("config")?;
            let period = config.get("currentPeriod")?;

            Some(GrokEvent::Billing(GrokLimits {
                credit_usage_percent: config.get("creditUsagePercent")?.as_f64()?,
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
        "shell.turn.inference_done" => {
            let context = value.get("ctx")?;
            let prompt = context
                .get("prompt_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let cached = context
                .get("cached_prompt_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let completion = context
                .get("completion_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);

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
        _ => None,
    }
}

pub fn collect(path: &Path, since: DateTime<Utc>) -> Result<GrokData, CollectError> {
    if !path.exists() {
        return Err(CollectError::NotFound(path.display().to_string()));
    }

    let file = File::open(path)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;

    let mut records = Vec::new();
    let mut limits: Option<GrokLimits> = None;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        match parse_line(&line) {
            Some(GrokEvent::Billing(candidate)) => {
                let is_newer = limits
                    .as_ref()
                    .map(|current| candidate.fetched_at >= current.fetched_at)
                    .unwrap_or(true);
                if is_newer {
                    limits = Some(candidate);
                }
            }
            Some(GrokEvent::Inference(record)) => {
                if record.timestamp >= since {
                    records.push(record);
                }
            }
            None => {}
        }
    }

    Ok(GrokData { records, limits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::window;
    use chrono::Local;
    use chrono::TimeZone;

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
        assert!((limits.credit_usage_percent - 46.0).abs() < 1e-9);
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
    fn parses_inference_records_with_cache_split() {
        let data = collect(&fixture_path(), since()).unwrap();

        assert_eq!(data.records.len(), 3);
        let tokens: TokenTotals = data
            .records
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
        assert!(data.records.iter().all(|record| record.timestamp >= recent_since));
    }

    #[test]
    fn ignores_unrelated_and_malformed_lines() {
        assert!(parse_line(r#"{"ts":"2026-09-17T13:00:05Z","msg":"slash.advertise","ctx":{"count":135}}"#).is_none());
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
