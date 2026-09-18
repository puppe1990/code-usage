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

fn value_for(snapshot: &UsageSnapshot, favorite: Favorite) -> String {
    match favorite {
        Favorite::GrokWeekly => provider(snapshot, Provider::Grok)
            .filter(|usage| matches!(usage.status, ProviderStatus::Ok))
            .and_then(|usage| usage.grok.as_ref())
            .map(|limits| percent(limits.credit_usage_percent))
            .unwrap_or_else(placeholder),
        Favorite::CommandCodePlan => provider(snapshot, Provider::CommandCode)
            .filter(|usage| matches!(usage.status, ProviderStatus::Ok))
            .and_then(|usage| usage.command_code.as_ref())
            .map(|limits| percent(limits.usage_percent))
            .unwrap_or_else(placeholder),
        Favorite::OpenCodeGoWeekly => provider(snapshot, Provider::OpenCode)
            .filter(|usage| matches!(usage.status, ProviderStatus::Ok))
            .and_then(|usage| usage.open_code_go.as_ref())
            .and_then(|limits| limits.weekly.as_ref())
            .map(|window| percent(window.percent))
            .unwrap_or_else(placeholder),
        Favorite::CommandCodeTodayCost => today_cost(snapshot, Provider::CommandCode),
        Favorite::OpenCodeTodayCost => today_cost(snapshot, Provider::OpenCode),
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
        .map(|favorite| value_for(snapshot, *favorite))
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
        UsageWindow,
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

    fn command_code_limits(percent: f64) -> CommandCodeLimits {
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
        CommandCodeLimits {
            plan: Some("GOAT".to_string()),
            plan_id: Some("individual-goat".to_string()),
            status: Some("active".to_string()),
            usage_percent: percent,
            credits_total: 70.0,
            credits_remaining: 38.0,
            requests_this_period: 7945,
            period_basis: Some("billing-period".to_string()),
            renews_at: None,
            days_to_renew: Some(23),
            five_hour: None,
            weekly: None,
            fetched_at: now,
        }
    }

    fn open_code_go_limits(weekly: f64) -> OpenCodeGoLimits {
        let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
        OpenCodeGoLimits {
            rolling: None,
            weekly: Some(OpenCodeGoWindow {
                percent: weekly,
                status: Some("ok".to_string()),
                resets_at: now,
            }),
            monthly: None,
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
        let mut command_code = provider(Provider::CommandCode, ProviderStatus::Ok, 1.09, None);
        command_code.command_code = Some(command_code_limits(45.7));

        let mut open_code = provider(Provider::OpenCode, ProviderStatus::Ok, 3.434, None);
        open_code.open_code_go = Some(open_code_go_limits(21.2));

        snapshot(vec![
            command_code,
            provider(
                Provider::Grok,
                ProviderStatus::Ok,
                0.0,
                Some(grok_limits(92.3)),
            ),
            open_code,
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
    fn renders_only_the_favorited_metrics() {
        assert_eq!(
            format_title(&healthy_snapshot(), &[Favorite::GrokWeekly]),
            "46%"
        );
        assert_eq!(
            format_title(
                &healthy_snapshot(),
                &[Favorite::CommandCodeTodayCost, Favorite::OpenCodeTodayCost]
            ),
            "$0.42 · $1.03"
        );
    }

    #[test]
    fn keeps_the_canonical_order_regardless_of_input_order() {
        assert_eq!(
            format_title(
                &healthy_snapshot(),
                &[Favorite::OpenCodeTodayCost, Favorite::GrokWeekly]
            ),
            "46% · $1.03"
        );
    }

    #[test]
    fn renders_the_plan_and_the_opencode_go_percentages() {
        assert_eq!(
            format_title(
                &full_snapshot(),
                &[Favorite::CommandCodePlan, Favorite::OpenCodeGoWeekly]
            ),
            "46% · 21%"
        );
    }

    #[test]
    fn every_metric_can_be_favorited_at_once() {
        assert_eq!(
            format_title(&full_snapshot(), &Favorite::ALL),
            "92% · 46% · $1.09 · 21% · $3.43"
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
        assert_eq!(
            format_title(
                &healthy_snapshot(),
                &[Favorite::CommandCodePlan, Favorite::OpenCodeGoWeekly]
            ),
            "– · –"
        );
    }
}
