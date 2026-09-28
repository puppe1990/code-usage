//! One `ProviderUsage` per CLI, built from the local transcripts/logs/database plus the
//! plan-limit caches. `snapshot` is the single entry point the rest of the app calls.

mod cache;
pub mod codex;
pub mod commandcode;
pub mod commandcode_api;
mod commandcode_payload;
pub mod grok;
mod limits_http;
pub mod opencode;
pub mod opencode_go;
#[cfg(test)]
mod test_server;
pub mod tray_title;
pub mod window;

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    CommandCode,
    Grok,
    OpenCode,
    Codex,
}

impl Provider {
    pub fn display_name(self) -> &'static str {
        match self {
            Provider::CommandCode => "Command Code",
            Provider::Grok => "Grok",
            Provider::OpenCode => "OpenCode",
            Provider::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: u64,
}

impl TokenTotals {
    pub fn add(&mut self, other: &TokenTotals) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.reasoning += other.reasoning;
    }

    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write + self.reasoning
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub tokens: TokenTotals,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub cost_usd: f64,
    pub tokens: TokenTotals,
    pub records: usize,
}

impl UsageWindow {
    pub fn add_record(&mut self, record: &UsageRecord) {
        self.cost_usd += record.cost_usd;
        self.tokens.add(&record.tokens);
        self.records += 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollectError {
    NotFound(String),
    Failed(String),
}

impl From<CollectError> for ProviderStatus {
    fn from(error: CollectError) -> Self {
        match error {
            CollectError::NotFound(path) => ProviderStatus::NotFound { path },
            CollectError::Failed(message) => ProviderStatus::Error { message },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ProviderStatus {
    Ok,
    NotFound { path: String },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokLimits {
    pub credit_usage_percent: Option<f64>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub tier: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

/// One plan window of the Codex CLI: its length decides the label (`5h`, `W`, `M`), because the
/// free plan reports a single monthly window while the paid ones report a 5-hour and a weekly one.
/// Workspace accounts also send `individual_limit`, the monthly credit cap.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexWindow {
    pub percent_used: f64,
    pub window_minutes: Option<i64>,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly: Option<CodexWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowLimit {
    pub percent_used: f64,
    pub used: f64,
    pub cap: f64,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandCodeLimits {
    pub plan: Option<String>,
    pub plan_id: Option<String>,
    pub status: Option<String>,
    pub usage_percent: f64,
    pub credits_total: f64,
    pub credits_remaining: f64,
    pub requests_this_period: u64,
    pub period_basis: Option<String>,
    pub renews_at: Option<DateTime<Utc>>,
    pub days_to_renew: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<WindowLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<WindowLimit>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoWindow {
    pub percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolling: Option<OpenCodeGoWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<OpenCodeGoWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly: Option<OpenCodeGoWindow>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderLimits {
    pub grok: Option<GrokLimits>,
    pub command_code: Option<CommandCodeLimits>,
    pub open_code_go: Option<OpenCodeGoLimits>,
    pub codex: Option<CodexLimits>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider: Provider,
    pub status: ProviderStatus,
    /// Account the harness is logged in with, when the CLI records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    pub today: UsageWindow,
    pub last_7d: UsageWindow,
    pub last_30d: UsageWindow,
    pub last_record_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grok: Option<GrokLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_code: Option<CommandCodeLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_code_go: Option<OpenCodeGoLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex: Option<CodexLimits>,
}

impl ProviderUsage {
    /// Card with the local windows plus whatever plan limits were cached for this harness.
    pub fn from_records(
        provider: Provider,
        account: Option<String>,
        records: &[UsageRecord],
        now: DateTime<Local>,
        limits: ProviderLimits,
    ) -> Self {
        let windows = window::summarize(records, now);
        Self {
            provider,
            status: ProviderStatus::Ok,
            account,
            today: windows.today,
            last_7d: windows.last_7d,
            last_30d: windows.last_30d,
            last_record_at: records.iter().map(|record| record.timestamp).max(),
            grok: limits.grok,
            command_code: limits.command_code,
            open_code_go: limits.open_code_go,
            codex: limits.codex,
        }
    }

    /// Card for a CLI that failed or is not installed; the error becomes the status notice.
    pub fn unavailable(provider: Provider, error: CollectError, account: Option<String>) -> Self {
        Self {
            provider,
            status: error.into(),
            account,
            today: UsageWindow::default(),
            last_7d: UsageWindow::default(),
            last_30d: UsageWindow::default(),
            last_record_at: None,
            grok: None,
            command_code: None,
            open_code_go: None,
            codex: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub generated_at: DateTime<Utc>,
    pub providers: Vec<ProviderUsage>,
}

/// Collects every harness into the snapshot the UI renders, one failure isolated per provider.
pub fn snapshot(now: DateTime<Local>) -> UsageSnapshot {
    UsageSnapshot {
        generated_at: now.with_timezone(&Utc),
        providers: vec![
            command_code_usage(now),
            grok_usage(now),
            open_code_usage(now),
            codex_usage(now),
        ],
    }
}

/// Command Code card: transcripts on disk plus the cached plan limits.
fn command_code_usage(now: DateTime<Local>) -> ProviderUsage {
    let account = commandcode_api::read_account(&commandcode_api::auth_path());

    match commandcode::collect(&commandcode::root_path()) {
        Ok(records) => ProviderUsage::from_records(
            Provider::CommandCode,
            account,
            &records,
            now,
            ProviderLimits {
                command_code: commandcode_api::cached_limits(),
                ..ProviderLimits::default()
            },
        ),
        Err(error) => ProviderUsage::unavailable(Provider::CommandCode, error, account),
    }
}

/// Grok card: the CLI log (records and the weekly limits live in the same file, the limits
/// attributed to the account that is live now).
fn grok_usage(now: DateTime<Local>) -> ProviderUsage {
    let since = window::start_of_day(now) - Duration::days(31);
    let auth = grok::auth_path();
    let account = grok::read_account(&auth);
    let user_id = grok::live_user_id(&auth);

    match grok::collect(&grok::log_path(), since, user_id.as_deref()) {
        Ok(data) => ProviderUsage::from_records(
            Provider::Grok,
            account,
            &data.records,
            now,
            ProviderLimits {
                grok: data.limits,
                ..ProviderLimits::default()
            },
        ),
        Err(error) => ProviderUsage::unavailable(Provider::Grok, error, account),
    }
}

/// OpenCode card: the local database plus the cached Go plan limits.
fn open_code_usage(now: DateTime<Local>) -> ProviderUsage {
    let since = window::start_of_day(now) - Duration::days(31);
    let account = opencode_go::read_account(&opencode_go::auth_path());

    match opencode::collect(&opencode::db_path(), since) {
        Ok(records) => ProviderUsage::from_records(
            Provider::OpenCode,
            account,
            &records,
            now,
            ProviderLimits {
                open_code_go: opencode_go::cached_limits(),
                ..ProviderLimits::default()
            },
        ),
        Err(error) => ProviderUsage::unavailable(Provider::OpenCode, error, account),
    }
}

/// Codex card: the rollout files carry both the turn tokens and the plan windows the CLI last saw.
fn codex_usage(now: DateTime<Local>) -> ProviderUsage {
    let since = window::start_of_day(now) - Duration::days(31);
    let account = codex::read_account(&codex::auth_path());

    match codex::collect(&codex::home_path(), since) {
        Ok(data) => ProviderUsage::from_records(
            Provider::Codex,
            account,
            &data.records,
            now,
            ProviderLimits {
                codex: data.limits,
                ..ProviderLimits::default()
            },
        ),
        Err(error) => ProviderUsage::unavailable(Provider::Codex, error, account),
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use chrono::Local;

    #[test]
    #[ignore = "reads the real CLI data from this machine"]
    fn prints_snapshot_from_real_data() {
        commandcode_api::refresh_cache();
        opencode_go::refresh_cache();
        let snapshot = snapshot(Local::now());

        for usage in &snapshot.providers {
            println!(
                "{}: status={:?} account={:?} today=${:.4} 7d=${:.4} 30d=${:.4} tokens30d={} last={:?}",
                usage.provider.display_name(),
                usage.status,
                usage.account,
                usage.today.cost_usd,
                usage.last_7d.cost_usd,
                usage.last_30d.cost_usd,
                usage.last_30d.tokens.total(),
                usage.last_record_at.map(|value| value.to_rfc3339()),
            );
            if let Some(limits) = &usage.grok {
                println!(
                    "  grok limits: {:?}% ({} -> {}), tier={:?}",
                    limits.credit_usage_percent,
                    limits.period_start.to_rfc3339(),
                    limits.period_end.to_rfc3339(),
                    limits.tier,
                );
            }
            if let Some(limits) = &usage.open_code_go {
                println!(
                    "  opencode go: rolling={:?}% weekly={:?}% monthly={:?}%",
                    limits.rolling.as_ref().map(|window| window.percent),
                    limits.weekly.as_ref().map(|window| window.percent),
                    limits.monthly.as_ref().map(|window| window.percent),
                );
            }
            if let Some(limits) = &usage.command_code {
                println!(
                    "  command code: plan={:?} status={:?} {:.2}% used, {} requests, {} days to renew, 5h={:?} weekly={:?}",
                    limits.plan,
                    limits.status,
                    limits.usage_percent,
                    limits.requests_this_period,
                    limits.days_to_renew.unwrap_or(-1),
                    limits.five_hour.as_ref().map(|window| window.percent_used),
                    limits.weekly.as_ref().map(|window| window.percent_used),
                );
            }
            if let Some(limits) = &usage.codex {
                println!(
                    "  codex: plan={:?} primary={:?}% ({} min) secondary={:?}% ({} min) monthly={:?}%",
                    limits.plan,
                    limits.primary.as_ref().map(|window| window.percent_used),
                    limits
                        .primary
                        .as_ref()
                        .and_then(|window| window.window_minutes)
                        .unwrap_or(-1),
                    limits.secondary.as_ref().map(|window| window.percent_used),
                    limits
                        .secondary
                        .as_ref()
                        .and_then(|window| window.window_minutes)
                        .unwrap_or(-1),
                    limits.monthly.as_ref().map(|window| window.percent_used),
                );
            }
        }

        println!(
            "tray title: {}",
            tray_title::format_title(
                &snapshot,
                crate::preferences::Preferences::default().favorite
            )
        );
    }
}
