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
pub fn get_favorites(state: State<'_, AppState>) -> Vec<Favorite> {
    state.favorites()
}

#[tauri::command]
pub fn set_favorites(app: AppHandle, favorites: Vec<Favorite>) -> Result<Vec<Favorite>, String> {
    let preferences = Preferences::with_favorites(favorites);
    preferences.save()?;

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.preferences.lock() {
            *guard = preferences.clone();
        }

        let snapshot = state.snapshot.lock().ok().and_then(|guard| guard.clone());
        if let Some(snapshot) = snapshot {
            refresh::publish(&app, &snapshot);
        }
    }

    Ok(preferences.favorites)
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
