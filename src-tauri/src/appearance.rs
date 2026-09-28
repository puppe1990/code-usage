//! macOS glass behind the panel when the theme is translucent.

use crate::preferences::Theme;
use crate::tray;
use tauri::window::Color;
use tauri::{AppHandle, Manager, WebviewWindow};

pub fn apply_to_app(app: &AppHandle, theme: Theme) {
    if let Some(window) = app.get_webview_window(tray::WINDOW_LABEL) {
        apply(&window, theme);
    }
}

pub fn apply(window: &WebviewWindow, theme: Theme) {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));

    #[cfg(target_os = "macos")]
    match theme {
        Theme::Translucent => {
            let _ = window_vibrancy::apply_vibrancy(
                window,
                window_vibrancy::NSVisualEffectMaterial::HudWindow,
                Some(window_vibrancy::NSVisualEffectState::Active),
                Some(14.0),
            );
        }
        Theme::Dark | Theme::Light => {
            let _ = window_vibrancy::clear_vibrancy(window);
        }
    }
}
