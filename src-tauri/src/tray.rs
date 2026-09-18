use crate::preferences::Favorite;
use crate::usage::{tray_title, UsageSnapshot};
use tauri::image::Image;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, LogicalPosition, Manager, Position, Rect, Size, WebviewWindow};

pub const TRAY_ID: &str = "usage-tray";
pub const WINDOW_LABEL: &str = "main";

const EDGE_MARGIN: f64 = 8.0;
const ICON_GAP: f64 = 6.0;
const DEFAULT_WINDOW_WIDTH: f64 = 360.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LogicalRect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

pub fn title_for(snapshot: &UsageSnapshot, favorite: Option<Favorite>) -> String {
    tray_title::format_title(snapshot, favorite)
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
    let scale = icon_scale(window);
    let icon = to_logical(rect, scale);
    let window_scale = window.scale_factor().unwrap_or(scale);
    let window_width = window
        .outer_size()
        .map(|outer| outer.width as f64 / window_scale)
        .unwrap_or(DEFAULT_WINDOW_WIDTH);

    let screen = screen_bounds(window, icon);
    let (x, y) = popover_position(icon, window_width, screen);

    if std::env::var("CODE_USAGE_DEBUG_TRAY").is_ok() {
        eprintln!(
            "tray placement: scale={scale} icon={icon:?} screen={screen:?} position=({x}, {y})"
        );
    }

    let _ = window.set_position(LogicalPosition::new(x, y));
}

/// Scale factor of the display the tray icon lives on.
///
/// The tray rect arrives in physical pixels scaled by that display (`tray-icon` multiplies the
/// status item's points by its backing scale factor), so dividing it by the popover window's own
/// scale factor lands on the wrong display whenever the screens use different scales.
#[cfg(target_os = "macos")]
fn icon_scale(window: &WebviewWindow) -> f64 {
    cursor_screen_scale()
        .or_else(|| {
            window
                .current_monitor()
                .ok()
                .flatten()
                .map(|monitor| monitor.scale_factor())
        })
        .unwrap_or(1.0)
}

#[cfg(not(target_os = "macos"))]
fn icon_scale(window: &WebviewWindow) -> f64 {
    window.scale_factor().unwrap_or(1.0)
}

/// The click that opens the popover leaves the cursor on the tray icon, so the screen under the
/// cursor is the screen the icon belongs to.
#[cfg(target_os = "macos")]
fn cursor_screen_scale() -> Option<f64> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};

    let marker = MainThreadMarker::new()?;
    let cursor = NSEvent::mouseLocation();

    NSScreen::screens(marker)
        .iter()
        .find(|screen| {
            let frame = screen.frame();
            cursor.x >= frame.origin.x
                && cursor.x < frame.origin.x + frame.size.width
                && cursor.y >= frame.origin.y
                && cursor.y < frame.origin.y + frame.size.height
        })
        .map(|screen| screen.backingScaleFactor())
}

fn screen_bounds(window: &WebviewWindow, icon: LogicalRect) -> Option<LogicalRect> {
    let monitors = window.available_monitors().ok()?;

    monitors
        .iter()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            LogicalRect {
                x: monitor.position().x as f64 / scale,
                y: monitor.position().y as f64 / scale,
                width: monitor.size().width as f64 / scale,
                height: monitor.size().height as f64 / scale,
            }
        })
        .find(|bounds| bounds.contains(icon.x, icon.y))
}

fn to_logical(rect: Rect, scale: f64) -> LogicalRect {
    let (x, y) = match rect.position {
        Position::Physical(position) => (position.x as f64 / scale, position.y as f64 / scale),
        Position::Logical(position) => (position.x, position.y),
    };
    let (width, height) = match rect.size {
        Size::Physical(size) => (size.width as f64 / scale, size.height as f64 / scale),
        Size::Logical(size) => (size.width, size.height),
    };

    LogicalRect {
        x,
        y,
        width,
        height,
    }
}

fn popover_position(
    icon: LogicalRect,
    window_width: f64,
    screen: Option<LogicalRect>,
) -> (f64, f64) {
    let mut x = icon.x + icon.width / 2.0 - window_width / 2.0;
    let y = icon.y + icon.height + ICON_GAP;

    if let Some(screen) = screen {
        let min_x = screen.x + EDGE_MARGIN;
        let max_x = (screen.x + screen.width - window_width - EDGE_MARGIN).max(min_x);
        x = x.clamp(min_x, max_x);
    }

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW_WIDTH: f64 = 360.0;

    fn icon_at(x: f64, y: f64) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width: 158.0,
            height: 22.0,
        }
    }

    fn screen_at_origin() -> Option<LogicalRect> {
        Some(LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 1728.0,
            height: 1117.0,
        })
    }

    #[test]
    fn converts_the_physical_tray_rect_with_the_icon_scale() {
        let rect = Rect {
            position: Position::Physical(tauri::PhysicalPosition::new(2250, 30)),
            size: Size::Physical(tauri::PhysicalSize::new(316, 44)),
        };

        let logical = to_logical(rect, 2.0);

        assert_eq!(
            logical,
            LogicalRect {
                x: 1125.0,
                y: 15.0,
                width: 158.0,
                height: 22.0,
            }
        );
    }

    #[test]
    fn keeps_logical_tray_rects_untouched() {
        let rect = Rect {
            position: Position::Logical(tauri::LogicalPosition::new(1125.0, 15.0)),
            size: Size::Logical(tauri::LogicalSize::new(158.0, 22.0)),
        };

        assert_eq!(to_logical(rect, 2.0), icon_at(1125.0, 15.0));
    }

    #[test]
    fn centers_the_popover_under_the_icon() {
        let (x, y) = popover_position(icon_at(1125.0, 15.0), WINDOW_WIDTH, screen_at_origin());

        assert_eq!(x, 1024.0);
        assert_eq!(y, 15.0 + 22.0 + ICON_GAP);
    }

    // regression: using the window's scale factor instead of the icon's used to send the popover
    // to the neighbouring display (x came out in physical pixels)
    #[test]
    fn does_not_place_the_popover_at_the_physical_coordinates() {
        let rect = Rect {
            position: Position::Physical(tauri::PhysicalPosition::new(2250, 30)),
            size: Size::Physical(tauri::PhysicalSize::new(316, 44)),
        };

        let icon = to_logical(rect, 2.0);
        let (x, _) = popover_position(icon, WINDOW_WIDTH, screen_at_origin());

        assert_eq!(x, 1024.0);
        assert!(x < 1728.0, "popover must stay inside the icon's screen");
    }

    #[test]
    fn clamps_the_popover_inside_the_icon_screen() {
        let (left, _) = popover_position(icon_at(0.0, 0.0), WINDOW_WIDTH, screen_at_origin());
        let (right, _) = popover_position(icon_at(1720.0, 0.0), WINDOW_WIDTH, screen_at_origin());

        assert_eq!(left, EDGE_MARGIN);
        assert_eq!(right, 1728.0 - WINDOW_WIDTH - EDGE_MARGIN);
    }

    #[test]
    fn places_the_popover_on_the_screen_that_holds_the_icon() {
        let external = LogicalRect {
            x: 1728.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        let icon = icon_at(2000.0, 15.0);

        assert!(external.contains(icon.x, icon.y));

        let (x, _) = popover_position(icon, WINDOW_WIDTH, Some(external));
        assert_eq!(x, 2000.0 + 79.0 - WINDOW_WIDTH / 2.0);
    }

    #[test]
    fn keeps_the_popover_on_screen_when_the_monitor_is_unknown() {
        let (x, _) = popover_position(icon_at(1125.0, 15.0), WINDOW_WIDTH, None);

        assert_eq!(x, 1024.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs a real screen setup and the main thread"]
    fn prints_the_screen_scales() {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSEvent, NSScreen};

        let marker = MainThreadMarker::new().expect("main thread");
        let cursor = NSEvent::mouseLocation();

        for screen in NSScreen::screens(marker).iter() {
            let frame = screen.frame();
            println!(
                "screen frame=({}, {}, {}, {}) scale={}",
                frame.origin.x,
                frame.origin.y,
                frame.size.width,
                frame.size.height,
                screen.backingScaleFactor()
            );
        }

        println!(
            "cursor=({}, {}) icon scale={:?}",
            cursor.x,
            cursor.y,
            cursor_screen_scale()
        );
    }
}
