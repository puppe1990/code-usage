//! Codex CLI rollouts (`<home>/sessions/<yyyy>/<mm>/<dd>/rollout-*.jsonl`): every `token_count`
//! event carries the tokens of the turn it closes plus the plan windows the CLI last saw, so one
//! file feeds both the panel totals and the 5-hour/weekly bars. No price is ever written — a
//! ChatGPT plan has no per-turn cost — so the card shows tokens only.

use super::{CodexLimits, CodexWindow, CollectError, TokenTotals, UsageRecord};
use chrono::{DateTime, Utc};
use std::cmp::Reverse;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 4;

pub fn default_home() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".codex")
}

pub fn home_path() -> PathBuf {
    std::env::var("CODE_USAGE_CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_home())
}

pub fn sessions_path() -> PathBuf {
    home_path().join("sessions")
}

pub fn auth_path() -> PathBuf {
    home_path().join("auth.json")
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexData {
    pub records: Vec<UsageRecord>,
    pub limits: Option<CodexLimits>,
}

/// One `token_count` event: the tokens of the last turn and the limits snapshot around it.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenCount {
    pub timestamp: DateTime<Utc>,
    pub tokens: TokenTotals,
    pub limits: Option<CodexLimits>,
}

pub fn parse_line(line: &str) -> Option<TokenCount> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "event_msg" {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }

    let timestamp = DateTime::parse_from_rfc3339(value.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);

    Some(TokenCount {
        timestamp,
        tokens: payload.get("info").map(turn_tokens).unwrap_or_default(),
        limits: payload
            .get("rate_limits")
            .and_then(|limits| limits_snapshot(limits, timestamp)),
    })
}

/// Tokens of the turn the event closes. `last_token_usage` is the delta of that turn, while
/// `total_token_usage` accumulates the whole session and would be counted again on every event.
fn turn_tokens(info: &serde_json::Value) -> TokenTotals {
    let Some(usage) = info.get("last_token_usage") else {
        return TokenTotals::default();
    };

    let count = |key: &str| usage.get(key).and_then(|value| value.as_u64()).unwrap_or(0);
    let cached = count("cached_input_tokens");
    let written = count("cache_write_input_tokens");
    let reasoning = count("reasoning_output_tokens");

    // the CLI counts the cached and the reasoning tokens inside its input/output totals, so they
    // are split out here: the fields stay additive and reach the `total_tokens` of the CLI
    TokenTotals {
        input: count("input_tokens")
            .saturating_sub(cached)
            .saturating_sub(written),
        output: count("output_tokens").saturating_sub(reasoning),
        cache_read: cached,
        cache_write: written,
        reasoning,
    }
}

/// Limits of the event, or `None` when it carries neither window: the CLI also emits snapshots
/// with the plan and the credits alone, and those must not erase the last usable numbers.
fn limits_snapshot(value: &serde_json::Value, fetched_at: DateTime<Utc>) -> Option<CodexLimits> {
    let limits = CodexLimits {
        primary: value.get("primary").and_then(window),
        secondary: value.get("secondary").and_then(window),
        monthly: monthly_window(value),
        plan: value
            .get("plan_type")
            .and_then(|plan| plan.as_str())
            .map(str::to_string),
        fetched_at,
    };

    (limits.primary.is_some() || limits.secondary.is_some() || limits.monthly.is_some())
        .then_some(limits)
}

fn window(value: &serde_json::Value) -> Option<CodexWindow> {
    Some(CodexWindow {
        percent_used: value.get("used_percent")?.as_f64()?,
        window_minutes: value.get("window_minutes").and_then(|value| value.as_i64()),
        resets_at: value
            .get("resets_at")
            .and_then(|value| value.as_i64())
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
    })
}

/// Workspace spend control, the monthly credit cap the CLI labels "Monthly credit limit".
fn monthly_window(value: &serde_json::Value) -> Option<CodexWindow> {
    if let Some(window) = value.get("monthly").and_then(window) {
        return Some(window);
    }

    let limit = value.get("individual_limit")?;
    if limit.is_null() {
        return None;
    }

    let remaining = limit
        .get("remaining_percent")
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))?;
    Some(CodexWindow {
        percent_used: (100.0 - remaining).clamp(0.0, 100.0),
        window_minutes: Some(30 * 24 * 60),
        resets_at: limit
            .get("resets_at")
            .and_then(|value| value.as_i64())
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
    })
}

/// Reads the rollouts from `since` on into one record list plus the newest limits snapshot.
pub fn collect(home: &Path, since: DateTime<Utc>) -> Result<CodexData, CollectError> {
    let sessions = home.join("sessions");
    if !sessions.exists() {
        return Err(CollectError::NotFound(sessions.display().to_string()));
    }

    let mut records = Vec::new();
    let mut limits: Option<CodexLimits> = None;

    for (index, path) in rollout_files(&sessions).iter().enumerate() {
        // the newest session is read even when it started before the window, so the last windows
        // the CLI saw still reach the menu bar; the older ones only matter for the totals
        if index > 0 && !rollout_date(path).map(|date| date >= since).unwrap_or(true) {
            continue;
        }

        for event in read_rollout(path) {
            if event.timestamp >= since && event.tokens.total() > 0 {
                records.push(UsageRecord {
                    timestamp: event.timestamp,
                    tokens: event.tokens,
                    cost_usd: 0.0,
                });
            }
            if let Some(snapshot) = event.limits {
                limits = newest(limits, snapshot);
            }
        }
    }

    Ok(CodexData { records, limits })
}

/// Keeps the snapshot the CLI wrote last.
fn newest(current: Option<CodexLimits>, candidate: CodexLimits) -> Option<CodexLimits> {
    match current {
        Some(current) if current.fetched_at > candidate.fetched_at => Some(current),
        _ => Some(candidate),
    }
}

/// Rollout files, newest session first: the start instant is part of the file name.
fn rollout_files(sessions: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit(sessions, 0, &mut files);
    files.sort_by_key(|path| Reverse(rollout_date(path)));
    files
}

fn visit(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, depth + 1, files);
        } else if is_rollout(&path) {
            files.push(path);
        }
    }
}

fn is_rollout(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
}

/// Session start carried by `rollout-<timestamp>-<uuid>.jsonl`, in UTC.
fn rollout_date(path: &Path) -> Option<DateTime<Utc>> {
    let stamp = path.file_name()?.to_str()?.strip_prefix("rollout-")?;
    if stamp.len() < 19 {
        return None;
    }

    let text = format!("{}T{}Z", &stamp[..10], stamp[11..19].replace('-', ":"));
    DateTime::parse_from_rfc3339(&text)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn read_rollout(path: &Path) -> Vec<TokenCount> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };

    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_line(&line))
        .collect()
}

/// E-mail of the login the CLI is using, shown on the panel avatar.
pub fn read_account(auth: &Path) -> Option<String> {
    claims(auth)?
        .get("email")?
        .as_str()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_string)
}

/// Key the CLI files this login under: `<chatgpt_user_id>::<chatgpt_account_id>`.
pub fn read_record_key(auth: &Path) -> Option<String> {
    let claims = claims(auth)?;
    let openai = claims.get("https://api.openai.com/auth")?;
    let user_id = openai
        .get("chatgpt_user_id")
        .or_else(|| openai.get("user_id"))?
        .as_str()?;
    let account_id = openai.get("chatgpt_account_id")?.as_str()?;

    if user_id.is_empty() || account_id.is_empty() {
        return None;
    }

    Some(format!("{user_id}::{account_id}"))
}

/// Claims of the `id_token`. The CLI already checked the signature; this only reads what it left.
fn claims(auth: &Path) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(auth).ok()?).ok()?;
    let token = value.get("tokens")?.get("id_token")?.as_str()?;
    let payload = token.split('.').nth(1)?;

    serde_json::from_slice(&crate::base64url::decode(payload)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::window;
    use chrono::Local;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::TempDir;

    /// One turn of a paid plan: 1000 input (800 cached, 100 written) and 100 output (40 reasoning).
    const TURN: &str = r#"{"timestamp":"2026-09-17T12:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":900,"cached_input_tokens":800,"output_tokens":60,"reasoning_output_tokens":40,"total_tokens":960},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":800,"cache_write_input_tokens":100,"output_tokens":100,"reasoning_output_tokens":40,"total_tokens":1100}},"rate_limits":{"primary":{"used_percent":12.5,"window_minutes":300,"resets_at":1789650000},"secondary":{"used_percent":3.2,"window_minutes":10080,"resets_at":1790600000},"credits":{"has_credits":false,"unlimited":false,"balance":null},"plan_type":"plus"}}}"#;

    fn fixture_home() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("codex")
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
    fn parses_the_turn_tokens_and_the_limits_snapshot() {
        let event = parse_line(TURN).expect("a token_count event");

        assert_eq!(event.timestamp.to_rfc3339(), "2026-09-17T12:05:00+00:00");
        assert_eq!(
            event.tokens,
            TokenTotals {
                input: 100,
                output: 60,
                cache_read: 800,
                cache_write: 100,
                reasoning: 40,
            }
        );
        assert_eq!(
            event.tokens.total(),
            1100,
            "the split must reach the total_tokens the CLI reports"
        );

        let limits = event.limits.expect("limits");
        assert_eq!(limits.plan.as_deref(), Some("plus"));
        assert_eq!(
            limits.primary,
            Some(CodexWindow {
                percent_used: 12.5,
                window_minutes: Some(300),
                resets_at: Some(Utc.timestamp_opt(1789650000, 0).unwrap()),
            })
        );
        assert_eq!(limits.secondary.unwrap().window_minutes, Some(10080));
        assert_eq!(limits.monthly, None);
    }

    #[test]
    fn reads_the_monthly_credit_cap_from_individual_limit() {
        let event = parse_line(
            r#"{"timestamp":"2026-09-17T12:05:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":12.5,"window_minutes":300,"resets_at":1789650000},"secondary":{"used_percent":3.2,"window_minutes":10080,"resets_at":1790600000},"individual_limit":{"limit":"100","used":"12","remaining_percent":88,"resets_at":1792000000},"plan_type":"plus"}}}"#,
        )
        .expect("a token_count event");

        let limits = event.limits.expect("limits");
        assert_eq!(
            limits.monthly,
            Some(CodexWindow {
                percent_used: 12.0,
                window_minutes: Some(43_200),
                resets_at: Some(Utc.timestamp_opt(1_792_000_000, 0).unwrap()),
            })
        );
    }

    #[test]
    fn ignores_lines_that_are_not_token_count_events() {
        assert!(parse_line(
            r#"{"timestamp":"2026-09-17T12:00:00.000Z","type":"session_meta","payload":{"id":"x"}}"#
        )
        .is_none());
        assert!(parse_line(
            r#"{"timestamp":"2026-09-17T12:00:01.000Z","type":"response_item","payload":{"type":"message","role":"user"}}"#
        )
        .is_none());
        assert!(parse_line(
            r#"{"timestamp":"2026-09-17T12:00:02.000Z","type":"event_msg","payload":{"type":"agent_message","message":"oi"}}"#
        )
        .is_none());
        assert!(parse_line(r#"{"type":"event_msg","payload":{"type":"token_count"}}"#).is_none());
        assert!(parse_line("not json at all").is_none());
    }

    #[test]
    fn drops_a_snapshot_without_windows_and_a_turn_without_usage() {
        let event = parse_line(
            r#"{"timestamp":"2026-09-17T12:50:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":null,"secondary":null,"credits":{"has_credits":true,"unlimited":false,"balance":"12.50"},"plan_type":"plus"}}}"#,
        )
        .expect("a token_count event");

        assert_eq!(event.limits, None, "no window is nothing to show");
        assert_eq!(event.tokens, TokenTotals::default());
    }

    #[test]
    fn collects_the_turns_inside_the_window() {
        let data = collect(&fixture_home(), since()).expect("fixture home is readable");
        let windows = window::summarize(&data.records, fixed_now());

        assert_eq!(
            data.records.len(),
            3,
            "the July session is out of the window"
        );
        assert_eq!(windows.today.records, 2);
        assert_eq!(windows.today.tokens.total(), 3300, "1100 + 2200");
        assert_eq!(windows.last_7d.records, 2);
        assert_eq!(windows.last_30d.records, 3);
        assert_eq!(windows.last_30d.tokens.total(), 3850, "1100 + 2200 + 550");
        assert_eq!(windows.last_30d.cost_usd, 0.0, "a rollout carries no price");
    }

    #[test]
    fn counts_the_turn_delta_and_not_the_session_total() {
        let data = collect(&fixture_home(), since()).expect("fixture home is readable");

        assert!(
            data.records
                .iter()
                .any(|record| record.tokens.total() == 2200),
            "the second turn of the day"
        );
        assert!(
            !data
                .records
                .iter()
                .any(|record| record.tokens.total() == 3300),
            "the session total would be counted again on every event"
        );
    }

    #[test]
    fn keeps_the_newest_usable_limits_snapshot() {
        let limits = collect(&fixture_home(), since())
            .expect("fixture home is readable")
            .limits
            .expect("the fixture logged limits");

        assert_eq!(
            limits.fetched_at.to_rfc3339(),
            "2026-09-17T12:45:00+00:00",
            "the windowless 12:50 snapshot must not erase it"
        );
        assert_eq!(limits.plan.as_deref(), Some("plus"));
        assert_eq!(limits.primary.unwrap().percent_used, 18.0);
        assert_eq!(limits.secondary.unwrap().percent_used, 4.6);
        assert_eq!(limits.monthly.unwrap().percent_used, 12.0);
    }

    #[test]
    fn reads_the_limits_of_the_newest_session_even_outside_the_window() {
        let future = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).single().unwrap();
        let data = collect(&fixture_home(), future).expect("fixture home is readable");

        assert!(data.records.is_empty());
        assert_eq!(
            data.limits.expect("limits").primary.unwrap().percent_used,
            18.0,
            "the last windows the CLI saw"
        );
    }

    #[test]
    fn reads_the_login_from_the_id_token() {
        let auth = fixture_home().join("auth.json");

        assert_eq!(
            read_account(&auth).as_deref(),
            Some("puppeicaropuppe@gmail.com")
        );
        assert_eq!(
            read_record_key(&auth).as_deref(),
            Some("user-fixture::acct-fixture")
        );
        assert_eq!(read_account(&fixture_home().join("missing.json")), None);
        assert_eq!(read_record_key(&fixture_home().join("missing.json")), None);
    }

    #[test]
    fn a_login_without_an_id_token_has_no_account() {
        let dir = TempDir::new().expect("temp dir");
        let auth = dir.path().join("auth.json");

        fs::write(&auth, r#"{"OPENAI_API_KEY":"sk-fixture"}"#).expect("writes auth file");
        assert_eq!(read_account(&auth), None);

        fs::write(&auth, r#"{"tokens":{"id_token":"a.b.c"}}"#).expect("writes broken token");
        assert_eq!(read_record_key(&auth), None);
    }

    #[test]
    fn missing_sessions_report_not_found() {
        let dir = TempDir::new().expect("temp dir");
        let sessions = dir.path().join("sessions");

        let error = collect(dir.path(), since()).expect_err("missing sessions dir");
        assert_eq!(
            error,
            CollectError::NotFound(sessions.display().to_string())
        );
    }
}
