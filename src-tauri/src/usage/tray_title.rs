use super::{Provider, ProviderStatus, UsageSnapshot};

const PLACEHOLDER: &str = "–";
const SEPARATOR: &str = " · ";

pub fn format_cost(value: f64) -> String {
    if value == 0.0 {
        return "$0".to_string();
    }
    format!("${value:.2}")
}

pub fn format_title(snapshot: &UsageSnapshot) -> String {
    let mut parts = vec![placeholder(); 3];

    for usage in &snapshot.providers {
        let slot = match usage.provider {
            Provider::Grok => 0,
            Provider::CommandCode => 1,
            Provider::OpenCode => 2,
        };

        parts[slot] = match usage.provider {
            Provider::Grok => usage
                .grok
                .as_ref()
                .filter(|_| matches!(usage.status, ProviderStatus::Ok))
                .map(|limits| format!("{:.0}%", limits.credit_usage_percent))
                .unwrap_or_else(placeholder),
            Provider::CommandCode | Provider::OpenCode => match usage.status {
                ProviderStatus::Ok => format_cost(usage.today.cost_usd),
                _ => placeholder(),
            },
        };
    }

    parts.join(SEPARATOR)
}

fn placeholder() -> String {
    PLACEHOLDER.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::{GrokLimits, ProviderUsage, UsageWindow};
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
            provider(
                Provider::CommandCode,
                ProviderStatus::Ok,
                0.42,
                None,
            ),
            provider(
                Provider::Grok,
                ProviderStatus::Ok,
                0.0,
                Some(grok_limits(46.4)),
            ),
            provider(
                Provider::OpenCode,
                ProviderStatus::Ok,
                1.034,
                None,
            ),
        ])
    }

    #[test]
    fn formats_costs_compactly() {
        assert_eq!(format_cost(0.0), "$0");
        assert_eq!(format_cost(0.4242), "$0.42");
        assert_eq!(format_cost(1.034), "$1.03");
        assert_eq!(format_cost(12.4), "$12.40");
    }

    #[test]
    fn renders_grok_percent_and_todays_costs() {
        assert_eq!(format_title(&healthy_snapshot()), "46% · $0.42 · $1.03");
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

        assert_eq!(format_title(&snapshot), "– · $0.42 · –");
    }

    #[test]
    fn grok_without_limits_renders_placeholder() {
        let snapshot = snapshot(vec![
            provider(Provider::CommandCode, ProviderStatus::Ok, 0.0, None),
            provider(Provider::Grok, ProviderStatus::Ok, 0.0, None),
            provider(Provider::OpenCode, ProviderStatus::Ok, 0.0, None),
        ]);

        assert_eq!(format_title(&snapshot), "– · $0 · $0");
    }
}
