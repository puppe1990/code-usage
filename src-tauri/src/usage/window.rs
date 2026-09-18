//! Buckets usage records into the today / 7 days / 30 days windows (day starts at local midnight).

use super::{UsageRecord, UsageWindow};
use chrono::{DateTime, Duration, Local, TimeZone, Utc};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Windows {
    pub today: UsageWindow,
    pub last_7d: UsageWindow,
    pub last_30d: UsageWindow,
}

/// Local midnight of `now`, expressed in UTC (records are stored in UTC).
pub fn start_of_day(now: DateTime<Local>) -> DateTime<Utc> {
    let naive = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    now.timezone()
        .from_local_datetime(&naive)
        .earliest()
        .map(|start| start.with_timezone(&Utc))
        .unwrap_or_else(|| (now - Duration::hours(24)).with_timezone(&Utc))
}

/// Sums records into the today / 7 days / 30 days windows ending at `now`.
pub fn summarize(records: &[UsageRecord], now: DateTime<Local>) -> Windows {
    let today_start = start_of_day(now);
    let week_start = today_start - Duration::days(6);
    let month_start = today_start - Duration::days(29);

    let mut windows = Windows::default();
    for record in records {
        if record.timestamp >= month_start {
            windows.last_30d.add_record(record);
        }
        if record.timestamp >= week_start {
            windows.last_7d.add_record(record);
        }
        if record.timestamp >= today_start {
            windows.today.add_record(record);
        }
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::TokenTotals;
    use chrono::Timelike;

    fn utc(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn local_now() -> DateTime<Local> {
        utc(2026, 9, 17, 15).with_timezone(&Local)
    }

    fn record(timestamp: DateTime<Utc>, cost_usd: f64, input: u64) -> UsageRecord {
        UsageRecord {
            timestamp,
            tokens: TokenTotals {
                input,
                ..TokenTotals::default()
            },
            cost_usd,
        }
    }

    #[test]
    fn today_starts_at_local_midnight() {
        let start = start_of_day(local_now());
        let local_start = start.with_timezone(&Local);
        assert_eq!(local_start.hour(), 0);
        assert_eq!(local_start.minute(), 0);
        assert_eq!(local_start.date_naive(), local_now().date_naive());
    }

    #[test]
    fn buckets_records_into_today_week_and_month() {
        let records = vec![
            record(utc(2026, 9, 17, 12), 0.10, 100),
            record(utc(2026, 9, 14, 12), 0.20, 200),
            record(utc(2026, 8, 27, 12), 0.40, 400),
            record(utc(2026, 7, 1, 12), 5.00, 5000),
        ];

        let windows = summarize(&records, local_now());

        assert!((windows.today.cost_usd - 0.10).abs() < 1e-9);
        assert_eq!(windows.today.records, 1);
        assert_eq!(windows.today.tokens.input, 100);

        assert!((windows.last_7d.cost_usd - 0.30).abs() < 1e-9);
        assert_eq!(windows.last_7d.records, 2);

        assert!((windows.last_30d.cost_usd - 0.70).abs() < 1e-9);
        assert_eq!(windows.last_30d.records, 3);
        assert_eq!(windows.last_30d.tokens.input, 700);
    }

    #[test]
    fn empty_records_produce_zeroed_windows() {
        let windows = summarize(&[], local_now());
        assert_eq!(windows, Windows::default());
    }
}
