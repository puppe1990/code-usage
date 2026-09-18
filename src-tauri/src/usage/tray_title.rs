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

fn provider(snapshot: &UsageSnapshot, provider: Provider) -> Option<&super::ProviderUsage> {
    snapshot
        .providers
        .iter()
        .find(|usage| usage.provider == provider)
}

fn value_for(snapshot: &UsageSnapshot, favorite: Favorite) -> Vec<String> {
    match favorite {
        Favorite::Grok => vec![provider(snapshot, Provider::Grok)
            .filter(|usage| matches!(usage.status, ProviderStatus::Ok))
            .and_then(|usage| usage.grok.as_ref())
            .map(|limits| percent(limits.credit_usage_percent))
            .unwrap_or_else(placeholder)],
        Favorite::CommandCode => {
            let windows = provider(snapshot, Provider::CommandCode)
                .and_then(|usage| usage.command_code.as_ref())
                .map(|limits| {
                    let mut values = Vec::new();
                    if let Some(window) = limits.five_hour.as_ref() {
                        values.push(percent(window.percent_used));
                    }
                    if let Some(window) = limits.weekly.as_ref() {
                        values.push(percent(window.percent_used));
                    }
                    values
                })
                .unwrap_or_default();

            if windows.is_empty() {
                vec![today_cost(snapshot, Provider::CommandCode)]
            } else {
                windows
            }
        }
        Favorite::OpenCode => {
            let windows = provider(snapshot, Provider::OpenCode)
                .and_then(|usage| usage.open_code_go.as_ref())
                .map(|limits| {
                    [&limits.rolling, &limits.weekly, &limits.monthly]
                        .into_iter()
                        .flatten()
                        .map(|window| percent(window.percent))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if windows.is_empty() {
                vec![today_cost(snapshot, Provider::OpenCode)]
            } else {
                windows
            }
        }
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

pub fn format_title(snapshot: &UsageSnapshot, favorites: &[Favorite]) -> String {
    Favorite::ALL
        .iter()
        .filter(|favorite| favorites.contains(favorite))
        .flat_map(|favorite| value_for(snapshot, *favorite))
        .collect::<Vec<_>>()
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
        CommandCodeLimits, GrokLimits, OpenCodeGoLimits, OpenCodeGoWindow, ProviderUsage,
        UsageWindow, WindowLimit,
    };
    use chrono::{TimeZone, Utc};

    fn grok_limits(percent: f64) -> GrokLimits {
        let start = Utc.with_ymd_and_hms(2026, 9, 17, 13, 0, 0).unwrap();
        GrokLimits {
            credit_usage_percent: percent,
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

    fn provider(
        provider: Provider,
        status: ProviderStatus,
        today_cost: f64,
        grok: Option<GrokLimits>,
    ) -> ProviderUsage {
        ProviderUsage {
            provider,
            status,
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

    fn full_snapshot() -> UsageSnapshot {
        snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None),
            provider(
                Provider::Grok,
                ProviderStatus::Ok,
                0.0,
                Some(grok_limits(92.3)),
            ),
            provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None),
        ])
    }

    fn default_favorites() -> Vec<Favorite> {
        Preferences::default().favorites
    }

    #[test]
    fn formats_costs_compactly() {
        assert_eq!(format_cost(0.0), "$0");
        assert_eq!(format_cost(0.4242), "$0.42");
        assert_eq!(format_cost(1.034), "$1.03");
        assert_eq!(format_cost(12.4), "$12.40");
    }

    #[test]
    fn renders_grok_percent_and_todays_costs_by_default() {
        assert_eq!(
            format_title(&healthy_snapshot(), &default_favorites()),
            "46% · $0.42 · $1.03"
        );
    }

    #[test]
    fn renders_only_the_favorited_harnesses() {
        assert_eq!(format_title(&healthy_snapshot(), &[Favorite::Grok]), "46%");
        assert_eq!(
            format_title(
                &healthy_snapshot(),
                &[Favorite::CommandCode, Favorite::OpenCode]
            ),
            "$0.42 · $1.03"
        );
    }

    #[test]
    fn keeps_the_canonical_order_regardless_of_input_order() {
        assert_eq!(
            format_title(&healthy_snapshot(), &[Favorite::OpenCode, Favorite::Grok]),
            "46% · $1.03"
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
            format_title(&snapshot, &Favorite::ALL),
            "92% · 10% · 5% · 11% · 21% · 10%"
        );
    }

    #[test]
    fn renders_only_the_windows_a_harness_actually_has() {
        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = Some(open_code_go_limits(None, None, Some(9.6)));

        let snapshot = snapshot(vec![open_code]);

        assert_eq!(format_title(&snapshot, &[Favorite::OpenCode]), "10%");
    }

    #[test]
    fn falls_back_to_today_cost_when_a_harness_has_no_windows() {
        let mut command_code = provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None);
        command_code.command_code = Some(command_code_limits(None, None));

        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = None;

        let snapshot = snapshot(vec![command_code, open_code]);

        assert_eq!(
            format_title(&snapshot, &[Favorite::CommandCode, Favorite::OpenCode]),
            "$1.09 · $3.43"
        );
    }

    #[test]
    fn every_harness_can_be_favorited_at_once() {
        assert_eq!(
            format_title(&full_snapshot(), &Favorite::ALL),
            "92% · $1.09 · $3.43"
        );
    }

    #[test]
    fn empty_favorites_produce_an_empty_title() {
        assert_eq!(format_title(&healthy_snapshot(), &[]), "");
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

        assert_eq!(
            format_title(&snapshot, &default_favorites()),
            "– · $0.42 · –"
        );
    }

    #[test]
    fn grok_without_limits_renders_placeholder() {
        let snapshot = snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 0.0, None),
            provider(Provider::Grok, ProviderStatus::Ok, 0.0, None),
            provider(Provider::OpenCode, ProviderStatus::Ok, 0.0, None),
        ]);

        assert_eq!(format_title(&snapshot, &default_favorites()), "– · $0 · $0");
    }

    #[test]
    fn metrics_without_data_render_placeholders() {
        let snapshot = snapshot(vec![provider(
            Provider::Grok,
            ProviderStatus::Ok,
            0.0,
            None,
        )]);

        assert_eq!(
            format_title(&snapshot, &[Favorite::Grok, Favorite::OpenCode]),
            "– · –"
        );
    }
}
