//! The coding window: one compact floating window, docked beside the user's
//! browser on the pet screen the first time and wherever they left it after.
//! Focus mode maximizes it instead, and leaving focus mode restores that spot.
use super::{store::WindowRect, SharedCoding};
use crate::world::MonitorInfo;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "code";
/// Logical pixels: narrow enough to leave most of the screen to the browser.
const DEFAULT_WIDTH: f64 = 460.;
const MARGIN: f64 = 12.;

/// Where the window goes: its last spot if that is still on a connected
/// screen, else docked to the right edge of the pet screen.
pub fn placement(
    saved: Option<WindowRect>,
    screens: &[MonitorInfo],
    pet: &MonitorInfo,
) -> (WindowRect, f64) {
    if let Some(rect) = saved {
        if let Some(screen) = screens.iter().find(|s| {
            let (cx, cy) = (rect.x + rect.width / 2., rect.y + 20.);
            cx >= s.bounds.x
                && cx < s.bounds.x + s.bounds.width
                && cy >= s.bounds.y
                && cy < s.bounds.y + s.bounds.height
        }) {
            return (rect, screen.scale);
        }
    }
    let scale = pet.scale;
    let work = pet.work;
    let width = (DEFAULT_WIDTH * scale).min(work.width);
    (
        WindowRect {
            x: work.x + work.width - width - MARGIN * scale,
            y: work.y + MARGIN * scale,
            width,
            height: work.height - 2. * MARGIN * scale,
        },
        scale,
    )
}

pub fn open(
    app: &tauri::AppHandle,
    coding: SharedCoding,
    rect: WindowRect,
    scale: f64,
    pinned: bool,
    focus: bool,
) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("code.html".into()))
        .title("Ledgeling Code")
        .inner_size(rect.width / scale, rect.height / scale)
        .position(rect.x / scale, rect.y / scale)
        .min_inner_size(380., 480.)
        .resizable(true)
        .always_on_top(pinned)
        .visible(true)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);
    let window = builder.build().map_err(|e| e.to_string())?;
    if focus {
        let _ = window.maximize();
    }
    let tracked = window.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::CloseRequested { .. }
        ) {
            let (Ok(position), Ok(size)) = (tracked.outer_position(), tracked.inner_size()) else {
                return;
            };
            // Focus mode fills the screen; the compact size is what is kept.
            if size.width == 0
                || tracked.is_minimized().unwrap_or(false)
                || tracked.is_maximized().unwrap_or(false)
            {
                return;
            }
            if let Ok(mut slot) = coding.lock() {
                if let Some(service) = slot.as_mut() {
                    service.store.data.window = Some(WindowRect {
                        x: position.x as f64,
                        y: position.y as f64,
                        width: size.width as f64,
                        height: size.height as f64,
                    });
                    if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                        let _ = service.store.save();
                    }
                }
            }
        }
    });
    Ok(window)
}

/// Focus mode fills the screen the window is on; leaving it returns to the
/// compact size.
pub fn set_focus_mode(window: &WebviewWindow, focus: bool) -> Result<(), String> {
    if focus {
        window.maximize()
    } else {
        window.unmaximize()
    }
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Rect;

    fn screen(x: f64, scale: f64) -> MonitorInfo {
        MonitorInfo {
            id: 0,
            bounds: Rect::new(x, 0., 2000., 1200.),
            work: Rect::new(x, 50., 2000., 1150.),
            scale,
            is_primary: x == 0.,
        }
    }

    #[test]
    fn the_first_open_docks_to_the_right_of_the_pet_screen() {
        let pet = screen(0., 2.);
        let (rect, scale) = placement(None, &[pet], &pet);
        assert_eq!(scale, 2.);
        assert_eq!(rect.width, 920.);
        assert_eq!(rect.x + rect.width, 2000. - 24.);
        assert_eq!(rect.y, 74.);
    }

    #[test]
    fn a_saved_spot_on_a_missing_screen_falls_back() {
        let pet = screen(0., 1.);
        let saved = WindowRect {
            x: 5000.,
            y: 10.,
            width: 400.,
            height: 800.,
        };
        assert_ne!(placement(Some(saved), &[pet], &pet).0, saved);
        let other = screen(2000., 1.);
        let saved = WindowRect {
            x: 2100.,
            y: 10.,
            width: 400.,
            height: 800.,
        };
        assert_eq!(placement(Some(saved), &[pet, other], &pet).0, saved);
    }
}
