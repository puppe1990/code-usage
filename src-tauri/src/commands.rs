use crate::preferences::{Favorite, Preferences};
use crate::refresh;
use crate::tray;
use crate::usage::UsageSnapshot;
use crate::AppState;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn get_usage(state: State<'_, AppState>) -> Option<UsageSnapshot> {
    state.snapshot.lock().ok().and_then(|guard| guard.clone())
}

#[tauri::command]
pub fn get_favorite(state: State<'_, AppState>) -> Option<Favorite> {
    state.favorite()
}

#[tauri::command]
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
pub async fn refresh_now(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    std::thread::spawn(move || {
        refresh::refresh_and_publish(&handle);
    });
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
