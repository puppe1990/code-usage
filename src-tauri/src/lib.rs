mod commands;
mod preferences;
mod refresh;
mod tray;
pub mod usage;

use preferences::{Favorite, Preferences};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{Manager, WindowEvent};

pub struct AppState {
    pub snapshot: Mutex<Option<usage::UsageSnapshot>>,
    pub preferences: Mutex<Preferences>,
    pub shown_at: Mutex<Option<Instant>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(None),
            preferences: Mutex::new(Preferences::load()),
            shown_at: Mutex::new(None),
        }
    }
}

impl AppState {
    pub fn mark_shown(&self) {
        if let Ok(mut guard) = self.shown_at.lock() {
            *guard = Some(Instant::now());
        }
    }

    pub fn favorite(&self) -> Option<Favorite> {
        self.preferences
            .lock()
            .ok()
            .and_then(|preferences| preferences.favorite)
    }

    fn was_just_shown(&self, threshold: std::time::Duration) -> bool {
        self.shown_at
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .map(|shown| shown.elapsed() < threshold)
            .unwrap_or(false)
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_usage,
            commands::get_favorite,
            commands::set_favorite,
            commands::refresh_now,
            commands::hide_panel,
            commands::quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::init(app.handle())?;
            refresh::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                let just_shown = window
                    .app_handle()
                    .try_state::<AppState>()
                    .map(|state| state.was_just_shown(std::time::Duration::from_millis(500)))
                    .unwrap_or(false);
                if !just_shown {
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Code Usage");
}
