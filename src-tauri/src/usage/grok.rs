//! Grok CLI log (`~/.grok/logs/unified.jsonl`): billing events carry the weekly percentage,
//! `shell.turn.inference_done` carries tokens. Billing lines do not say which account produced
//! them, so they are attributed through the process (`pid`) that logged them.

use super::{CollectError, GrokLimits, TokenTotals, UsageRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// One log line that this app cares about, with the process that wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    pub pid: Option<u32>,
    pub event: GrokEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GrokEvent {
    Billing(GrokLimits),
    Inference(UsageRecord),
    /// A line carrying a `user_id`: which account the process at this `pid` is running as.
    Session(String),
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

/// Credential the CLI is signed in with: the newest login in `auth.json` that carries an e-mail.
fn newest_credential(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;

    value
        .as_object()?
        .values()
        .filter(|credential| {
            credential
                .get("email")
                .and_then(|email| email.as_str())
                .is_some_and(|email| !email.is_empty())
        })
        .max_by_key(|credential| created_at(credential))
        .cloned()
}

fn created_at(credential: &serde_json::Value) -> Option<DateTime<Utc>> {
    credential
        .get("create_time")
        .and_then(|time| time.as_str())
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.with_timezone(&Utc))
}

/// E-mail of the logged in account, shown on the panel avatar.
pub fn read_account(path: &Path) -> Option<String> {
    newest_credential(path)?
        .get("email")
        .and_then(|email| email.as_str())
        .filter(|email| !email.is_empty())
        .map(str::to_string)
}

/// User id of that same credential: the log lines carry it, so it ties the CLI sessions to the
/// account that is live right now.
pub fn live_user_id(path: &Path) -> Option<String> {
    newest_credential(path)?
        .get("user_id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

pub fn parse_line(line: &str) -> Option<LogEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let message = value.get("msg")?.as_str()?;
    let timestamp = DateTime::parse_from_rfc3339(value.get("ts")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let pid = value
        .get("pid")
        .and_then(|pid| pid.as_u64())
        .map(|pid| pid as u32);
    let context = value.get("ctx")?;

    let event = match message {
        "billing: fetched credits config" => billing_event(context, timestamp),
        "shell.turn.inference_done" => inference_event(context, timestamp),
        _ => session_event(context),
    }?;

    Some(LogEvent { pid, event })
}

/// Any line with a `user_id` says which account the process at that `pid` is signed in with —
/// `auth init user_info check` and the paywall events carry it.
fn session_event(context: &serde_json::Value) -> Option<GrokEvent> {
    context
        .get("user_id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(|id| GrokEvent::Session(id.to_string()))
}

/// `billing: fetched credits config` carries the weekly percentage and its period. Some accounts
/// (e.g. `SuperGrok Plus`) omit the percentage, so it stays `None`; the account that logged it is
/// recovered from the process id by `pick_limits`.
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

/// Reads the CLI log from `since` on, into records plus the limits of `account` (the live login,
/// as its `user_id`).
pub fn collect(
    path: &Path,
    since: DateTime<Utc>,
    account: Option<&str>,
) -> Result<GrokData, CollectError> {
    if !path.exists() {
        return Err(CollectError::NotFound(path.display().to_string()));
    }

    let file = File::open(path)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))?;

    let mut records = Vec::new();
    let mut billings: Vec<(Option<u32>, GrokLimits)> = Vec::new();
    let mut sessions: HashMap<u32, String> = HashMap::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Some(entry) = parse_line(&line) else {
            continue;
        };

        match entry.event {
            GrokEvent::Billing(limits) => billings.push((entry.pid, limits)),
            GrokEvent::Inference(record) if record.timestamp >= since => records.push(record),
            GrokEvent::Inference(_) => {}
            GrokEvent::Session(user_id) => {
                if let Some(pid) = entry.pid {
                    sessions.insert(pid, user_id);
                }
            }
        }
    }

    Ok(GrokData {
        records,
        limits: pick_limits(&billings, &sessions, account),
    })
}

/// Newest of `limits` by fetch time.
fn newest<'a>(limits: impl Iterator<Item = &'a GrokLimits>) -> Option<GrokLimits> {
    limits.max_by_key(|limits| limits.fetched_at).cloned()
}

/// Which billing numbers the card may show: only the ones logged by the live account, so switching
/// logins never mixes two accounts. A fetch without the weekly percentage does not erase the last
/// number that same account reported. Logs without any `user_id` (older CLIs) keep the old rule —
/// the newest line wins.
fn pick_limits(
    billings: &[(Option<u32>, GrokLimits)],
    sessions: &HashMap<u32, String>,
    account: Option<&str>,
) -> Option<GrokLimits> {
    let every = || billings.iter().map(|(_, limits)| limits);

    let Some(account) = account.filter(|_| !sessions.is_empty()) else {
        return newest(every());
    };

    let mine: Vec<&GrokLimits> = billings
        .iter()
        .filter(|(pid, _)| {
            pid.and_then(|pid| sessions.get(&pid)).map(String::as_str) == Some(account)
        })
        .map(|(_, limits)| limits)
        .collect();

    if mine.is_empty() {
        return None;
    }

    newest(
        mine.iter()
            .copied()
            .filter(|limits| limits.credit_usage_percent.is_some()),
    )
    .or_else(|| newest(mine.into_iter()))
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
        let data = collect(&fixture_path(), since(), None).expect("fixture log is readable");

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
        let data = collect(&path, since(), None).expect("fixture log is readable");

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
    fn shows_the_limits_of_the_live_account_only() {
        let path = fixture_path().with_file_name("sessions.jsonl");

        let personal = collect(&path, since(), Some("user-personal"))
            .expect("fixture log is readable")
            .limits
            .expect("the live account logged a config");

        assert_eq!(personal.tier.as_deref(), Some("SuperGrok"));
        assert_eq!(personal.credit_usage_percent, None);
        assert_eq!(
            personal.fetched_at.to_rfc3339(),
            "2026-09-25T16:28:41.525+00:00",
            "the 100%/3% of the other account must not leak in"
        );
    }

    #[test]
    fn keeps_the_last_percentage_the_live_account_reported() {
        let path = fixture_path().with_file_name("sessions.jsonl");

        let work = collect(&path, since(), Some("user-work"))
            .expect("fixture log is readable")
            .limits
            .expect("that account logged a config");

        assert_eq!(
            work.credit_usage_percent,
            Some(3.0),
            "the newest fetch of that account omits the percentage"
        );
        assert_eq!(
            work.fetched_at.to_rfc3339(),
            "2026-09-25T13:00:45.249+00:00"
        );
    }

    #[test]
    fn an_account_without_sessions_shows_no_limits() {
        let path = fixture_path().with_file_name("sessions.jsonl");

        assert_eq!(
            collect(&path, since(), Some("user-elsewhere"))
                .expect("fixture log is readable")
                .limits,
            None,
            "nothing in the log belongs to that account"
        );
    }

    #[test]
    fn logs_without_a_user_id_keep_the_newest_line() {
        let data = collect(&fixture_path(), since(), Some("user-anything")).expect("reads");

        assert_eq!(
            data.limits.expect("billing limits").fetched_at.to_rfc3339(),
            "2026-09-17T14:22:36.406+00:00"
        );
    }

    #[test]
    fn parses_inference_records_with_cache_split() {
        let data = collect(&fixture_path(), since(), None).unwrap();

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
        let data = collect(&fixture_path(), recent_since, None).unwrap();
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
    fn reads_the_live_user_id_from_the_same_credential() {
        let path = fixture_path().with_file_name("auth.json");

        assert_eq!(live_user_id(&path).as_deref(), Some("user-new-fixture"));
        assert_eq!(
            live_user_id(&fixture_path().with_file_name("missing.json")),
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
        let data = collect(&fixture_path(), since(), None).unwrap();
        let windows = window::summarize(&data.records, fixed_now());

        assert_eq!(windows.today.records, 2);
        assert!((windows.last_30d.cost_usd - 0.0).abs() < 1e-9);
        assert_eq!(windows.last_30d.tokens.total(), 3700);
    }

    #[test]
    fn missing_log_reports_not_found() {
        let missing = fixture_path().with_file_name("missing.jsonl");
        let error = collect(&missing, since(), None).expect_err("missing log");
        assert_eq!(error, CollectError::NotFound(missing.display().to_string()));
    }
}
