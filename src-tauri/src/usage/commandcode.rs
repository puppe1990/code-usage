use super::{CollectError, TokenTotals, UsageRecord};
use chrono::{DateTime, Utc};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 3;

pub fn default_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".commandcode")
        .join("projects")
}

pub fn root_path() -> PathBuf {
    std::env::var("CODE_USAGE_CC_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_root())
}

pub fn parse_line(line: &str) -> Option<UsageRecord> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "message" {
        return None;
    }
    if value.get("message")?.get("role")?.as_str()? != "assistant" {
        return None;
    }

    let usage = value.get("usage")?;
    let timestamp = DateTime::parse_from_rfc3339(value.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);

    Some(UsageRecord {
        timestamp,
        tokens: TokenTotals {
            input: usage
                .get("inputTokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            output: usage
                .get("outputTokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            cache_read: usage
                .get("cacheReadTokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            cache_write: usage
                .get("cacheWriteTokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            reasoning: 0,
        },
        cost_usd: usage
            .get("costUsd")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0),
    })
}

pub fn collect(root: &Path) -> Result<Vec<UsageRecord>, CollectError> {
    if !root.exists() {
        return Err(CollectError::NotFound(root.display().to_string()));
    }

    let mut records = Vec::new();
    visit(root, 0, &mut records);
    Ok(records)
}

fn visit(dir: &Path, depth: usize, records: &mut Vec<UsageRecord>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, depth + 1, records);
        } else if is_transcript(&path) {
            read_transcript(&path, records);
        }
    }
}

fn is_transcript(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with(".jsonl") && !name.contains(".checkpoints.")
}

fn read_transcript(path: &Path, records: &mut Vec<UsageRecord>) {
    let Ok(file) = File::open(path) else {
        return;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Some(record) = parse_line(&line) {
            records.push(record);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::window;
    use chrono::Local;
    use chrono::TimeZone;

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("commandcode")
    }

    fn fixed_now() -> DateTime<Local> {
        Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Local)
    }

    #[test]
    fn parses_assistant_usage_line() {
        let line = r#"{"type":"message","id":"m2","timestamp":"2026-09-17T12:00:30Z","message":{"role":"assistant","content":[]},"usage":{"inputTokens":1000,"outputTokens":100,"cacheReadTokens":50,"cacheWriteTokens":10,"costUsd":0.1}}"#;

        let record = parse_line(line).expect("a usage record");

        assert_eq!(record.timestamp.to_rfc3339(), "2026-09-17T12:00:30+00:00");
        assert!((record.cost_usd - 0.1).abs() < 1e-9);
        assert_eq!(
            record.tokens,
            TokenTotals {
                input: 1000,
                output: 100,
                cache_read: 50,
                cache_write: 10,
                reasoning: 0,
            }
        );
    }

    #[test]
    fn ignores_non_assistant_and_malformed_lines() {
        assert!(
            parse_line(r#"{"type":"session","id":"s","timestamp":"2026-09-17T12:00:00Z"}"#)
                .is_none()
        );
        assert!(parse_line(r#"{"type":"message","timestamp":"2026-09-17T12:00:00Z","message":{"role":"user"},"usage":{"costUsd":1.0}}"#).is_none());
        assert!(parse_line(r#"{"type":"message","timestamp":"2026-09-17T12:00:00Z","message":{"role":"assistant"}}"#).is_none());
        assert!(parse_line("not json at all").is_none());
        assert!(parse_line(r#"{"type":"message","timestamp":"not-a-date","message":{"role":"assistant"},"usage":{"costUsd":9.99}}"#).is_none());
    }

    #[test]
    fn tolerates_missing_usage_fields() {
        let line = r#"{"type":"message","timestamp":"2026-09-17T12:00:00Z","message":{"role":"assistant"},"usage":{"costUsd":0.5}}"#;
        let record = parse_line(line).expect("record with partial usage");
        assert_eq!(record.tokens, TokenTotals::default());
        assert!((record.cost_usd - 0.5).abs() < 1e-9);
    }

    #[test]
    fn collects_every_transcript_and_skips_checkpoints() {
        let records = collect(&fixture_root()).expect("fixture root is readable");

        assert_eq!(
            records.len(),
            5,
            "2 today + 3 older, checkpoint file ignored"
        );
        assert!(
            records.iter().all(|record| record.cost_usd < 100.0),
            "checkpoint usage must not be collected"
        );
    }

    #[test]
    fn summarizes_fixture_windows() {
        let records = collect(&fixture_root()).unwrap();
        let windows = window::summarize(&records, fixed_now());

        assert!((windows.today.cost_usd - 0.30).abs() < 1e-9);
        assert_eq!(windows.today.records, 2);
        assert_eq!(windows.today.tokens.input, 3000);
        assert_eq!(windows.today.tokens.output, 300);
        assert_eq!(windows.today.tokens.cache_read, 150);
        assert_eq!(windows.today.tokens.cache_write, 30);

        assert!((windows.last_7d.cost_usd - 0.35).abs() < 1e-9);
        assert_eq!(windows.last_7d.records, 3);

        assert!((windows.last_30d.cost_usd - 1.35).abs() < 1e-9);
        assert_eq!(windows.last_30d.records, 4);
    }

    #[test]
    fn missing_root_reports_not_found() {
        let missing = fixture_root().join("does-not-exist");
        let error = collect(&missing).expect_err("missing root");
        assert_eq!(error, CollectError::NotFound(missing.display().to_string()));
    }
}
