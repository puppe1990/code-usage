//! IPC surface called by the panel (`invoke` in src/main.ts).

use crate::accounts::{self, Account};
use crate::preferences::{Favorite, Preferences};
use crate::refresh;
use crate::tray;
use crate::usage::{self, Provider, UsageSnapshot};
use crate::AppState;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn get_usage(state: State<'_, AppState>) -> Option<UsageSnapshot> {
    state.snapshot.lock().ok().and_then(|guard| guard.clone())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
/// Enables/disables the login item and returns the state the system reports afterwards.
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let autolaunch = app.autolaunch();

    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(|error| error.to_string())?;

    autolaunch.is_enabled().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_favorite(state: State<'_, AppState>) -> Option<Favorite> {
    state.favorite()
}

#[tauri::command]
/// Saves the star (or clears it with `None`), then syncs the tray title and mark.
pub fn set_favorite(
    app: AppHandle,
    favorite: Option<Favorite>,
) -> Result<Option<Favorite>, String> {
    let preferences = Preferences::with_favorite(favorite);
    preferences.save()?;

    tray::sync_icon(&app, preferences.favorite);

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.preferences.lock() {
            *guard = preferences.clone();
        }

        let snapshot = state.snapshot.lock().ok().and_then(|guard| guard.clone());
        if let Some(snapshot) = snapshot {
            refresh::publish(&app, &snapshot);
        }
    }

    Ok(preferences.favorite)
}

#[tauri::command]
/// Kicks a recompute on a worker thread and returns immediately.
pub async fn refresh_now(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    std::thread::spawn(move || {
        refresh::refresh_and_publish(&handle);
    });
    Ok(())
}

#[tauri::command]
/// Logins saved for a harness, with the active one marked (`agent-account-switchers` stores).
pub fn list_accounts(provider: Provider) -> Result<Vec<Account>, String> {
    accounts::list(provider)
}

#[tauri::command]
/// Switches the harness to `name`, drops the plan limits cached for the account that just left,
/// and recomputes on a worker thread; returns the refreshed list the panel renders.
pub fn switch_account(
    app: AppHandle,
    provider: Provider,
    name: String,
) -> Result<Vec<Account>, String> {
    accounts::switch(provider, &name)?;

    match provider {
        Provider::CommandCode => usage::commandcode_api::forget_limits(),
        Provider::OpenCode => usage::opencode_go::forget_limits(),
        Provider::Grok => {}
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        refresh::refresh_and_publish(&handle);
    });

    accounts::list(provider)
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    if let Some(window) = app.get_webview_window(tray::WINDOW_LABEL) {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
