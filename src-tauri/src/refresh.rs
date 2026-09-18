use crate::tray;
use crate::usage::{self, UsageSnapshot};
use crate::AppState;
use chrono::Local;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

pub fn publish(app: &AppHandle, snapshot: &UsageSnapshot) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.snapshot.lock() {
            *guard = Some(snapshot.clone());
        }
    }
    if let Some(icon) = app.tray_by_id(tray::TRAY_ID) {
        let _ = icon.set_title(Some(tray::title_for(snapshot)));
    }
    let _ = app.emit("usage-updated", snapshot);
}

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

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        refresh_and_publish(&app);
        std::thread::sleep(REFRESH_INTERVAL);
    });
}
