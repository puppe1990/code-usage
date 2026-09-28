//! The menu bar string: one harness at a time, showing the plan windows it has (`5h`, `W`, `M`)
//! and falling back to today's cost (or `-`) when there is no window data.

use super::{Provider, ProviderStatus, UsageSnapshot};
use crate::preferences::Favorite;

const PLACEHOLDER: &str = "–";
const SEPARATOR: &str = " · ";

pub fn format_cost(value: f64) -> String {
    if value == 0.0 {
        return "$0".to_string();
    }
    format!("${value:.2}")
}

fn percent(value: f64) -> String {
    format!("{value:.0}%")
}

fn window_value(label: &str, value: f64) -> String {
    format!("{label} {}", percent(value))
}

fn provider(snapshot: &UsageSnapshot, provider: Provider) -> Option<&super::ProviderUsage> {
    snapshot
        .providers
        .iter()
        .find(|usage| usage.provider == provider)
}

/// Grok exposes a single weekly window; accounts that omit it fall back to the placeholder.
fn grok_values(snapshot: &UsageSnapshot) -> Vec<String> {
    provider(snapshot, Provider::Grok)
        .filter(|usage| matches!(usage.status, ProviderStatus::Ok))
        .and_then(|usage| usage.grok.as_ref())
        .and_then(|limits| limits.credit_usage_percent)
        .map_or_else(
            || vec![placeholder()],
            |value| vec![window_value("W", value)],
        )
}

/// The billing period is monthly: that percentage is the plan usage the dashboard shows.
fn command_code_values(snapshot: &UsageSnapshot) -> Vec<String> {
    let windows = provider(snapshot, Provider::CommandCode)
        .and_then(|usage| usage.command_code.as_ref())
        .map(|limits| {
            [
                ("5h", limits.five_hour.as_ref()),
                ("W", limits.weekly.as_ref()),
            ]
            .into_iter()
            .filter_map(|(label, window)| {
                window.map(|window| window_value(label, window.percent_used))
            })
            .chain(std::iter::once(window_value("M", limits.usage_percent)))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    fallback_to_today_cost(windows, snapshot, Provider::CommandCode)
}

/// OpenCode Go exposes rolling / weekly / monthly windows; only the ones it has are shown.
fn open_code_values(snapshot: &UsageSnapshot) -> Vec<String> {
    let windows = provider(snapshot, Provider::OpenCode)
        .and_then(|usage| usage.open_code_go.as_ref())
        .map(|limits| {
            [
                ("5h", limits.rolling.as_ref()),
                ("W", limits.weekly.as_ref()),
                ("M", limits.monthly.as_ref()),
            ]
            .into_iter()
            .filter_map(|(label, window)| window.map(|window| window_value(label, window.percent)))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    fallback_to_today_cost(windows, snapshot, Provider::OpenCode)
}

/// Codex labels every window by its own length instead of a fixed slot: the free plan reports a
/// single monthly window, the paid ones a 5-hour and a weekly one.
fn codex_values(snapshot: &UsageSnapshot) -> Vec<String> {
    let windows = provider(snapshot, Provider::Codex)
        .and_then(|usage| usage.codex.as_ref())
        .map(|limits| {
            [
                ("5h", limits.primary.as_ref()),
                ("W", limits.secondary.as_ref()),
            ]
            .into_iter()
            .filter_map(|(fallback, window)| {
                window.map(|window| {
                    window_value(
                        window_label(window.window_minutes, fallback),
                        window.percent_used,
                    )
                })
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // no price to fall back to: a rollout never records one
    if windows.is_empty() {
        vec![placeholder()]
    } else {
        windows
    }
}

/// `5h` for the five-hour window, `W` for the weekly one and `M` for the monthly one; a window
/// the CLI sent without a length keeps the label of its slot.
fn window_label(window_minutes: Option<i64>, fallback: &str) -> &str {
    match window_minutes {
        Some(minutes) if minutes <= 6 * 60 => "5h",
        Some(minutes) if minutes <= 7 * 24 * 60 => "W",
        Some(_) => "M",
        None => fallback,
    }
}

/// A harness without window data shows today's cost instead.
fn fallback_to_today_cost(
    windows: Vec<String>,
    snapshot: &UsageSnapshot,
    kind: Provider,
) -> Vec<String> {
    if windows.is_empty() {
        vec![today_cost(snapshot, kind)]
    } else {
        windows
    }
}

fn value_for(snapshot: &UsageSnapshot, favorite: Favorite) -> Vec<String> {
    match favorite {
        Favorite::Grok => grok_values(snapshot),
        Favorite::CommandCode => command_code_values(snapshot),
        Favorite::OpenCode => open_code_values(snapshot),
        Favorite::Codex => codex_values(snapshot),
    }
}

fn today_cost(snapshot: &UsageSnapshot, kind: Provider) -> String {
    match provider(snapshot, kind) {
        Some(usage) if matches!(usage.status, ProviderStatus::Ok) => {
            format_cost(usage.today.cost_usd)
        }
        _ => placeholder(),
    }
}

/// Menu bar title for the selected harness (empty when no star is selected).
pub fn format_title(snapshot: &UsageSnapshot, favorite: Option<Favorite>) -> String {
    favorite
        .map(|favorite| value_for(snapshot, favorite))
        .unwrap_or_default()
        .join(SEPARATOR)
}

fn placeholder() -> String {
    PLACEHOLDER.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::Preferences;
    use crate::usage::{
        CodexLimits, CodexWindow, CommandCodeLimits, GrokLimits, OpenCodeGoLimits,
        OpenCodeGoWindow, ProviderUsage, UsageWindow, WindowLimit,
    };
    use chrono::{TimeZone, Utc};

    fn grok_limits(percent: f64) -> GrokLimits {
        let start = Utc.with_ymd_and_hms(2026, 9, 17, 13, 0, 0).unwrap();
        GrokLimits {
            credit_usage_percent: Some(percent),
            period_start: start,
            period_end: start + chrono::Duration::days(7),
            tier: Some("SuperGrok Plus".to_string()),
            fetched_at: start,
        }
    }

    fn command_code_limits(five_hour: Option<f64>, weekly: Option<f64>) -> CommandCodeLimits {
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
        CommandCodeLimits {
            plan: Some("GOAT".to_string()),
            plan_id: Some("individual-goat".to_string()),
            status: Some("active".to_string()),
            usage_percent: 45.7,
            credits_total: 70.0,
            credits_remaining: 38.0,
            requests_this_period: 7945,
            period_basis: Some("billing-period".to_string()),
            renews_at: None,
            days_to_renew: Some(23),
            five_hour: five_hour.map(|percent_used| WindowLimit {
                percent_used,
                used: 1.0,
                cap: 14.0,
                reset_at: now,
            }),
            weekly: weekly.map(|percent_used| WindowLimit {
                percent_used,
                used: 1.0,
                cap: 35.0,
                reset_at: now,
            }),
            fetched_at: now,
        }
    }

    fn open_code_go_limits(
        rolling: Option<f64>,
        weekly: Option<f64>,
        monthly: Option<f64>,
    ) -> OpenCodeGoLimits {
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
        let window = |percent: f64| {
            Some(OpenCodeGoWindow {
                percent,
                status: Some("ok".to_string()),
                resets_at: now,
            })
        };

        OpenCodeGoLimits {
            rolling: rolling.and_then(window),
            weekly: weekly.and_then(window),
            monthly: monthly.and_then(window),
            fetched_at: now,
        }
    }

    fn codex_limits(primary: Option<(f64, i64)>, secondary: Option<(f64, i64)>) -> CodexLimits {
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
        let window = |(percent_used, window_minutes): (f64, i64)| CodexWindow {
            percent_used,
            window_minutes: Some(window_minutes),
            resets_at: Some(now),
        };

        CodexLimits {
            primary: primary.map(window),
            secondary: secondary.map(window),
            plan: Some("plus".to_string()),
            fetched_at: now,
        }
    }

    fn codex_provider(limits: Option<CodexLimits>) -> ProviderUsage {
        let mut provider = provider(Provider::Codex, ProviderStatus::Ok, 0.0, None);
        provider.codex = limits;
        provider
    }

    fn provider(
        provider: Provider,
        status: ProviderStatus,
        today_cost: f64,
        grok: Option<GrokLimits>,
    ) -> ProviderUsage {
        ProviderUsage {
            provider,
            status,
            account: None,
            today: UsageWindow {
                cost_usd: today_cost,
                ..UsageWindow::default()
            },
            last_7d: UsageWindow::default(),
            last_30d: UsageWindow::default(),
            last_record_at: None,
            grok,
            command_code: None,
            open_code_go: None,
            codex: None,
        }
    }

    fn snapshot(providers: Vec<ProviderUsage>) -> UsageSnapshot {
        UsageSnapshot {
            generated_at: Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap(),
            providers,
        }
    }

    fn healthy_snapshot() -> UsageSnapshot {
        snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 0.42, None),
            provider(
                Provider::Grok,
                ProviderStatus::Ok,
                0.0,
                Some(grok_limits(46.4)),
            ),
            provider(Provider::OpenCode, ProviderStatus::Ok, 1.034, None),
        ])
    }

    fn default_favorite() -> Option<Favorite> {
        Preferences::default().favorite
    }

    #[test]
    fn formats_costs_compactly() {
        assert_eq!(format_cost(0.0), "$0");
        assert_eq!(format_cost(0.4242), "$0.42");
        assert_eq!(format_cost(1.034), "$1.03");
        assert_eq!(format_cost(12.4), "$12.40");
    }

    #[test]
    fn renders_the_default_favorite() {
        assert_eq!(
            format_title(&healthy_snapshot(), default_favorite()),
            "$0.42"
        );
    }

    #[test]
    fn renders_only_the_favorited_harness() {
        assert_eq!(
            format_title(&healthy_snapshot(), Some(Favorite::Grok)),
            "W 46%"
        );
        assert_eq!(
            format_title(&healthy_snapshot(), Some(Favorite::OpenCode)),
            "$1.03"
        );
    }

    #[test]
    fn renders_the_plan_windows_when_they_exist() {
        let mut command_code = provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None);
        command_code.command_code = Some(command_code_limits(Some(10.2), Some(4.6)));

        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = Some(open_code_go_limits(Some(11.4), Some(21.2), Some(9.6)));

        let snapshot = snapshot(vec![
            command_code,
            provider(
                Provider::Grok,
                ProviderStatus::Ok,
                0.0,
                Some(grok_limits(92.3)),
            ),
            open_code,
        ]);

        assert_eq!(
            format_title(&snapshot, Some(Favorite::CommandCode)),
            "5h 10% · W 5% · M 46%"
        );
        assert_eq!(
            format_title(&snapshot, Some(Favorite::OpenCode)),
            "5h 11% · W 21% · M 10%"
        );
        assert_eq!(format_title(&snapshot, Some(Favorite::Grok)), "W 92%");
    }

    #[test]
    fn renders_the_command_code_billing_period_as_the_monthly_window() {
        let mut command_code = provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None);
        command_code.command_code = Some(command_code_limits(None, None));

        let snapshot = snapshot(vec![command_code]);

        assert_eq!(
            format_title(&snapshot, Some(Favorite::CommandCode)),
            "M 46%"
        );
    }

    #[test]
    fn renders_only_the_windows_a_harness_actually_has() {
        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = Some(open_code_go_limits(None, None, Some(9.6)));

        let snapshot = snapshot(vec![open_code]);

        assert_eq!(format_title(&snapshot, Some(Favorite::OpenCode)), "M 10%");
    }

    #[test]
    fn falls_back_to_today_cost_when_a_harness_has_no_windows() {
        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = Some(open_code_go_limits(None, None, None));

        let snapshot = snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None),
            open_code,
        ]);

        assert_eq!(
            format_title(&snapshot, Some(Favorite::CommandCode)),
            "$1.09"
        );
        assert_eq!(format_title(&snapshot, Some(Favorite::OpenCode)), "$3.43");
    }

    #[test]
    fn renders_the_codex_windows_labelled_by_their_own_length() {
        let snapshot = snapshot(vec![codex_provider(Some(codex_limits(
            Some((18.0, 300)),
            Some((4.6, 10080)),
        )))]);

        assert_eq!(
            format_title(&snapshot, Some(Favorite::Codex)),
            "5h 18% · W 5%"
        );
    }

    #[test]
    fn renders_the_single_monthly_window_of_the_free_plan() {
        let snapshot = snapshot(vec![codex_provider(Some(codex_limits(
            Some((100.0, 43200)),
            None,
        )))]);

        assert_eq!(format_title(&snapshot, Some(Favorite::Codex)), "M 100%");
    }

    #[test]
    fn a_codex_window_without_a_length_keeps_the_label_of_its_slot() {
        let mut limits = codex_limits(Some((12.4, 300)), Some((4.6, 10080)));
        limits.primary.as_mut().expect("primary").window_minutes = None;
        limits.secondary = None;

        let snapshot = snapshot(vec![codex_provider(Some(limits))]);

        assert_eq!(format_title(&snapshot, Some(Favorite::Codex)), "5h 12%");
    }

    #[test]
    fn codex_without_limits_renders_a_placeholder() {
        let snapshot = snapshot(vec![codex_provider(None)]);

        assert_eq!(
            format_title(&snapshot, Some(Favorite::Codex)),
            "–",
            "a rollout records no price to fall back to"
        );
    }

    #[test]
    fn no_favorite_produces_an_empty_title() {
        assert_eq!(format_title(&healthy_snapshot(), None), "");
    }

    #[test]
    fn missing_providers_render_placeholders() {
        let snapshot = snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 0.42, None),
            provider(
                Provider::Grok,
                ProviderStatus::NotFound {
                    path: "/missing".to_string(),
                },
                0.0,
                None,
            ),
            provider(
                Provider::OpenCode,
                ProviderStatus::Error {
                    message: "boom".to_string(),
                },
                0.0,
                None,
            ),
        ]);

        assert_eq!(format_title(&snapshot, Some(Favorite::Grok)), "–");
        assert_eq!(format_title(&snapshot, Some(Favorite::OpenCode)), "–");
    }

    #[test]
    fn grok_without_limits_renders_placeholder() {
        let snapshot = snapshot(vec![provider(
            Provider::Grok,
            ProviderStatus::Ok,
            0.0,
            None,
        )]);

        assert_eq!(format_title(&snapshot, Some(Favorite::Grok)), "–");
    }

    #[test]
    fn grok_without_percentage_renders_placeholder() {
        let mut limits = grok_limits(0.0);
        limits.credit_usage_percent = None;

        let snapshot = snapshot(vec![provider(
            Provider::Grok,
            ProviderStatus::Ok,
            0.0,
            Some(limits),
        )]);

        assert_eq!(format_title(&snapshot, Some(Favorite::Grok)), "–");
    }

    #[test]
    fn metrics_without_data_render_placeholders() {
        let snapshot = snapshot(vec![provider(
            Provider::Grok,
            ProviderStatus::Ok,
            0.0,
            None,
        )]);

        assert_eq!(format_title(&snapshot, Some(Favorite::OpenCode)), "–");
    }
}
