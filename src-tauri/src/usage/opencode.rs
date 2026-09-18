//! OpenCode SQLite database (`~/.local/share/opencode/opencode.db`, opened read-only): the
//! `message` table holds a JSON payload with `cost` and `tokens` per message.

use super::{CollectError, TokenTotals, UsageRecord};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub fn default_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

pub fn db_path() -> PathBuf {
    std::env::var("CODE_USAGE_OPENCODE_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path())
}

pub fn parse_message_data(time_created_ms: i64, data: &str) -> Option<UsageRecord> {
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    if value.get("role")?.as_str()? != "assistant" {
        return None;
    }

    let tokens = value.get("tokens")?;
    let cache = tokens.get("cache");

    Some(UsageRecord {
        timestamp: Utc.timestamp_millis_opt(time_created_ms).single()?,
        tokens: TokenTotals {
            input: tokens
                .get("input")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            output: tokens
                .get("output")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            reasoning: tokens
                .get("reasoning")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            cache_read: cache
                .and_then(|cache| cache.get("read"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            cache_write: cache
                .and_then(|cache| cache.get("write"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
        },
        cost_usd: value
            .get("cost")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0),
    })
}

fn open_read_only(path: &Path) -> Result<Connection, CollectError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    Connection::open_with_flags(path, flags)
        .map_err(|error| CollectError::Failed(format!("{}: {error}", path.display())))
}

/// Reads messages newer than `since` from the database into one record list.
pub fn collect(db: &Path, since: DateTime<Utc>) -> Result<Vec<UsageRecord>, CollectError> {
    if !db.exists() {
        return Err(CollectError::NotFound(db.display().to_string()));
    }

    let connection = open_read_only(db)?;
    let mut statement = connection
        .prepare("SELECT `time_created`, `data` FROM `message` WHERE `time_created` >= ?1")
        .map_err(|error| CollectError::Failed(format!("{}: {error}", db.display())))?;

    let rows = statement
        .query_map([since.timestamp_millis()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| CollectError::Failed(format!("{}: {error}", db.display())))?;

    let mut records = Vec::new();
    for row in rows {
        let Ok((time_created, data)) = row else {
            continue;
        };
        if let Some(record) = parse_message_data(time_created, &data) {
            records.push(record);
        }
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::window;
    use chrono::Local;
    use rusqlite::params;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn ts_ms(iso: &str) -> i64 {
        DateTime::parse_from_rfc3339(iso)
            .expect("valid iso timestamp")
            .timestamp_millis()
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

    fn insert_message(conn: &Connection, id: &str, iso: &str, data: &str) {
        conn.execute(
            "INSERT INTO `message` (`id`,`session_id`,`time_created`,`time_updated`,`data`) VALUES (?1, ?2, ?3, ?3, ?4)",
            params![id, "ses_test", ts_ms(iso), data],
        )
        .expect("insert message row");
    }

    fn fixture_db(dir: &Path) -> PathBuf {
        let path = dir.join("opencode.db");
        let conn = Connection::open(&path).expect("create fixture db");
        conn.execute_batch(
            "CREATE TABLE `message` (
                `id` text PRIMARY KEY,
                `session_id` text NOT NULL,
                `time_created` integer NOT NULL,
                `time_updated` integer NOT NULL,
                `data` text NOT NULL
            );
            CREATE INDEX `message_session_time_created_id_idx` ON `message` (`session_id`,`time_created`,`id`);",
        )
        .expect("create schema");

        insert_message(
            &conn,
            "msg_today",
            "2026-09-17T12:00:00Z",
            r#"{"role":"assistant","cost":0.12,"tokens":{"input":1000,"output":100,"reasoning":20,"cache":{"read":500,"write":0}}}"#,
        );
        insert_message(
            &conn,
            "msg_week",
            "2026-09-07T12:00:00Z",
            r#"{"role":"assistant","cost":0.25,"tokens":{"input":2000,"output":200,"reasoning":0,"cache":{"read":0,"write":0}}}"#,
        );
        insert_message(
            &conn,
            "msg_user",
            "2026-09-17T12:30:00Z",
            r#"{"role":"user","tokens":{"input":10,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}}"#,
        );
        insert_message(
            &conn,
            "msg_no_cost",
            "2026-09-17T13:00:00Z",
            r#"{"role":"assistant","tokens":{"input":300,"output":30,"reasoning":0,"cache":{"read":0,"write":0}}}"#,
        );
        insert_message(
            &conn,
            "msg_old",
            "2026-08-08T12:00:00Z",
            r#"{"role":"assistant","cost":9.99,"tokens":{"input":9000,"output":900,"reasoning":0,"cache":{"read":0,"write":0}}}"#,
        );
        insert_message(&conn, "msg_broken", "2026-09-17T14:00:00Z", "not json");

        drop(conn);
        path
    }

    #[test]
    fn parses_assistant_message_payload() {
        let record = parse_message_data(
            ts_ms("2026-09-17T12:00:00Z"),
            r#"{"role":"assistant","cost":0.12,"tokens":{"input":1000,"output":100,"reasoning":20,"cache":{"read":500,"write":5}}}"#,
        )
        .expect("assistant usage");

        assert_eq!(
            record.tokens,
            TokenTotals {
                input: 1000,
                output: 100,
                cache_read: 500,
                cache_write: 5,
                reasoning: 20,
            }
        );
        assert!((record.cost_usd - 0.12).abs() < 1e-9);
        assert_eq!(record.timestamp.to_rfc3339(), "2026-09-17T12:00:00+00:00");
    }

    #[test]
    fn ignores_user_messages_missing_tokens_and_broken_json() {
        let ts = ts_ms("2026-09-17T12:00:00Z");
        assert!(parse_message_data(ts, r#"{"role":"user"}"#).is_none());
        assert!(parse_message_data(ts, r#"{"role":"assistant"}"#).is_none());
        assert!(parse_message_data(ts, "not json").is_none());
    }

    #[test]
    fn aggregates_messages_from_fixture_db() {
        let dir = TempDir::new().unwrap();
        let db = fixture_db(dir.path());

        let records = collect(&db, since()).expect("collect from fixture db");

        assert_eq!(records.len(), 3, "old, user and broken rows are skipped");

        let windows = window::summarize(&records, fixed_now());
        assert!((windows.today.cost_usd - 0.12).abs() < 1e-9);
        assert_eq!(windows.today.records, 2);
        assert_eq!(windows.today.tokens.input, 1300);
        assert_eq!(windows.today.tokens.output, 130);
        assert_eq!(windows.today.tokens.reasoning, 20);
        assert_eq!(windows.today.tokens.cache_read, 500);

        assert_eq!(windows.last_7d.records, 2);
        assert!((windows.last_30d.cost_usd - 0.37).abs() < 1e-9);
        assert_eq!(windows.last_30d.records, 3);
    }

    #[test]
    fn opens_the_database_read_only() {
        let dir = TempDir::new().unwrap();
        let db = fixture_db(dir.path());
        let mut perms = fs::metadata(&db).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&db, perms).unwrap();

        let records = collect(&db, since()).expect("read-only database is readable");
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn missing_database_reports_not_found() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.db");
        let error = collect(&missing, since()).expect_err("missing db");
        assert_eq!(error, CollectError::NotFound(missing.display().to_string()));
    }
}
