//! The 60s recompute loop: rebuilds the snapshot, updates the tray title and mark, and emits
//! `usage-updated` to the panel.

use crate::tray;
use crate::usage::{self, UsageSnapshot};
use crate::AppState;
use chrono::Local;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Stores the snapshot, updates the tray and notifies the panel.
pub fn publish(app: &AppHandle, snapshot: &UsageSnapshot) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.snapshot.lock() {
            *guard = Some(snapshot.clone());
        }
        if let Some(icon) = app.tray_by_id(tray::TRAY_ID) {
            let _ = icon.set_title(Some(tray::title_for(snapshot, state.favorite())));
        }
    }
    let _ = app.emit("usage-updated", snapshot);
}

/// Recomputes, publishes, then refreshes the plan-limit caches and publishes again on change.
pub fn refresh_and_publish(app: &AppHandle) {
    let snapshot = usage::snapshot(Local::now());
    publish(app, &snapshot);

    let limits_before = |snapshot: &UsageSnapshot| {
        snapshot
            .providers
            .iter()
            .map(|provider| (provider.command_code.clone(), provider.open_code_go.clone()))
            .collect::<Vec<_>>()
    };
    let before = limits_before(&snapshot);

    usage::commandcode_api::refresh_cache();
    usage::opencode_go::refresh_cache();

    let refreshed = usage::snapshot(Local::now());
    if limits_before(&refreshed) != before {
        publish(app, &refreshed);
    }
}

/// Starts the background loop that recomputes every `REFRESH_INTERVAL`.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        refresh_and_publish(&app);
        std::thread::sleep(REFRESH_INTERVAL);
    });
}
