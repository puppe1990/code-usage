use crate::usage::{tray_title, UsageSnapshot};
use tauri::image::Image;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, LogicalPosition, Manager, Position, Rect, Size, WebviewWindow};

pub const TRAY_ID: &str = "usage-tray";
pub const WINDOW_LABEL: &str = "main";

pub fn title_for(snapshot: &UsageSnapshot) -> String {
    tray_title::format_title(snapshot)
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?)
        .icon_as_template(true)
        .title("…")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_window_at(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_window_at(app: &AppHandle, rect: Rect) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.mark_shown();
    }
    position_window(&window, rect);
    let _ = window.show();
    let _ = window.set_focus();
}

fn position_window(window: &WebviewWindow, rect: Rect) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let (icon_x, icon_y, icon_width, icon_height): (f64, f64, f64, f64) = {
        let (x, y) = match rect.position {
            Position::Physical(p) => (p.x as f64 / scale, p.y as f64 / scale),
            Position::Logical(p) => (p.x, p.y),
        };
        let (width, height) = match rect.size {
            Size::Physical(s) => (s.width as f64 / scale, s.height as f64 / scale),
            Size::Logical(s) => (s.width, s.height),
        };
        (x, y, width, height)
    };

    let Ok(outer) = window.outer_size() else {
        return;
    };
    let window_width = outer.width as f64 / scale;

    let mut x = icon_x + icon_width / 2.0 - window_width / 2.0;
    let y = icon_y + icon_height + 6.0;

    if let Ok(Some(monitor)) = window.current_monitor() {
        let monitor_x = monitor.position().x as f64 / scale;
        let monitor_width = monitor.size().width as f64 / scale;
        let min_x = monitor_x + 8.0;
        let max_x = (monitor_x + monitor_width - window_width - 8.0).max(min_x);
        x = x.clamp(min_x, max_x);
    }

    let _ = window.set_position(LogicalPosition::new(x, y));
}
