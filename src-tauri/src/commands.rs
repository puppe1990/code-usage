//! IPC surface called by the panel (`invoke` in src/main.ts).

use crate::accounts::{self, Account};
use crate::preferences::{complete_order, Favorite, Preferences, Theme};
use crate::refresh;
use crate::tray;
use crate::usage::{self, Provider, UsageSnapshot};
use crate::AppState;
use tauri::{AppHandle, LogicalSize, Manager, Size, State};
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
pub fn get_hidden(state: State<'_, AppState>) -> Vec<Favorite> {
    state
        .preferences
        .lock()
        .ok()
        .map(|preferences| preferences.hidden.clone())
        .unwrap_or_default()
}

fn store_preferences(app: &AppHandle, preferences: Preferences) -> Result<Preferences, String> {
    preferences.save()?;
    tray::sync_icon(app, preferences.favorite);

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.preferences.lock() {
            *guard = preferences.clone();
        }

        let snapshot = state.snapshot.lock().ok().and_then(|guard| guard.clone());
        if let Some(snapshot) = snapshot {
            refresh::publish(app, &snapshot);
        }
    }

    Ok(preferences)
}

fn current_preferences(app: &AppHandle) -> Preferences {
    app.try_state::<AppState>()
        .and_then(|state| state.preferences.lock().ok().map(|guard| guard.clone()))
        .unwrap_or_else(Preferences::load)
}

#[tauri::command]
/// Saves the star (or clears it with `None`), then syncs the tray title and mark.
pub fn set_favorite(
    app: AppHandle,
    favorite: Option<Favorite>,
) -> Result<Option<Favorite>, String> {
    let mut preferences = current_preferences(&app);
    preferences.favorite = favorite;
    Ok(store_preferences(&app, preferences)?.favorite)
}

#[tauri::command]
/// Hides harnesses from Overview and from the tabs; a hidden favorite leaves the tray.
pub fn set_hidden(app: AppHandle, hidden: Vec<Favorite>) -> Result<Vec<Favorite>, String> {
    let mut preferences = current_preferences(&app);
    preferences.hidden = hidden;
    if let Some(favorite) = preferences.favorite {
        if preferences.hidden.contains(&favorite) {
            preferences.favorite = None;
        }
    }
    Ok(store_preferences(&app, preferences)?.hidden)
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Theme {
    state
        .preferences
        .lock()
        .ok()
        .map(|preferences| preferences.theme)
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: Theme) -> Result<Theme, String> {
    let mut preferences = current_preferences(&app);
    preferences.theme = theme;
    let saved = store_preferences(&app, preferences)?.theme;
    crate::appearance::apply_to_app(&app, saved);
    Ok(saved)
}

#[tauri::command]
pub fn get_order(state: State<'_, AppState>) -> Vec<Favorite> {
    state
        .preferences
        .lock()
        .ok()
        .map(|preferences| complete_order(&preferences.order))
        .unwrap_or_else(|| complete_order(&[]))
}

#[tauri::command]
pub fn set_order(app: AppHandle, order: Vec<Favorite>) -> Result<Vec<Favorite>, String> {
    let mut preferences = current_preferences(&app);
    preferences.order = complete_order(&order);
    Ok(store_preferences(&app, preferences)?.order)
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
        // Grok and Codex read the plan windows from what the CLI itself wrote: nothing is cached
        Provider::Grok | Provider::Codex => {}
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        refresh::refresh_and_publish(&handle);
    });

    accounts::list(provider)
}

#[tauri::command]
pub fn fit_panel(app: AppHandle, height: f64) -> Result<(), String> {
    let Some(window) = app.get_webview_window(tray::WINDOW_LABEL) else {
        return Err("panel window missing".into());
    };
    let height = height.clamp(160.0, 820.0);
    window
        .set_size(Size::Logical(LogicalSize::new(360.0, height)))
        .map_err(|error| error.to_string())?;
    crate::appearance::apply(&window, current_preferences(&app).theme);
    Ok(())
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
