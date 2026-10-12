use crate::world::{MonitorInfo, Rect};
use std::time::Duration;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// A connected screen plus the stable name the pet-screen setting keys on.
/// Tauri derives it from the display's model number, so it survives unplug,
/// replug and list reshuffles; `MonitorInfo::id` does not (it is a list index).
#[derive(Clone, Debug)]
pub struct Screen {
    pub info: MonitorInfo,
    pub name: String,
}

/// Run something on the UI thread and wait for the answer. macOS only reports
/// monitor geometry, and only creates windows, on the main thread; calling from
/// the update loop silently returns nothing.
pub fn on_main<T: Send + 'static>(
    handle: &tauri::AppHandle,
    work: impl FnOnce(&tauri::AppHandle) -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle_for_work = handle.clone();
    handle
        .run_on_main_thread(move || {
            let _ = sender.send(work(&handle_for_work));
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(2)).ok()
}

/// Screen geometry comes from the app handle rather than a window, because a
/// hidden companion window reports no monitors at all.
pub fn screens(app: &tauri::AppHandle) -> Result<Vec<Screen>, String> {
    // Ask a real window: on macOS the app-level monitor list can come back
    // empty, while the window-level one is reliable.
    let window = app.get_webview_window("main");
    let primary = match &window {
        Some(window) => window.primary_monitor(),
        None => app.primary_monitor(),
    }
    .map_err(|e| e.to_string())?;
    let primary_position = primary.as_ref().map(|m| m.position().to_owned());
    let mut out = Vec::new();
    let found = match &window {
        Some(window) => window.available_monitors(),
        None => app.available_monitors(),
    }
    .map_err(|e| e.to_string())?;
    for (index, monitor) in found.into_iter().enumerate() {
        let position = monitor.position();
        let size = monitor.size();
        let work = monitor.work_area();
        out.push(Screen {
            info: MonitorInfo {
                id: index as u64,
                bounds: Rect::new(
                    position.x as f64,
                    position.y as f64,
                    size.width as f64,
                    size.height as f64,
                ),
                work: Rect::new(
                    work.position.x as f64,
                    work.position.y as f64,
                    work.size.width as f64,
                    work.size.height as f64,
                ),
                scale: monitor.scale_factor(),
                is_primary: primary_position.as_ref() == Some(position),
            },
            name: monitor
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", index + 1)),
        });
    }
    // An empty list is a legitimate answer while the session is locked.
    Ok(out)
}

/// The one screen pets live on: the user's pick when it is connected, else
/// the main display, else the only one there is.
pub fn chosen_screen<'a>(screens: &'a [Screen], wanted: Option<&str>) -> Option<&'a Screen> {
    wanted
        .and_then(|name| screens.iter().find(|s| s.name == name))
        .or_else(|| screens.iter().find(|s| s.info.is_primary))
        .or_else(|| screens.first())
}

/// Canonical engine form of the chosen screen. The id is forced to 0 because
/// Tauri's monitor ids are list indices — a reshuffle when a display connects
/// would otherwise read as a world change. With a single monitor in engine
/// space, window-edge surfaces outside its work area are clipped away, so
/// confinement falls out of the existing geometry rules rather than a new one.
pub fn pet_monitor(screen: &Screen) -> MonitorInfo {
    MonitorInfo {
        id: 0,
        ..screen.info
    }
}

/// The one overlay window, on the pet screen. It is created once and moved
/// with `set_position`/`set_size` afterwards: recreating it reloads the
/// webview (a visible flash) and closing the old one is asynchronous, so a
/// rebuild-per-change storm leaves several dying overlays each still showing
/// its last painted scene — pets appeared duplicated across screens.
/// Windows treats a topmost window that exactly covers a monitor as a
/// fullscreen app and stops an auto-hidden taskbar from revealing. One
/// physical pixel short on the right edge is enough to avoid that.
#[cfg(target_os = "windows")]
const OVERLAY_INSET: f64 = 1.;
#[cfg(not(target_os = "windows"))]
const OVERLAY_INSET: f64 = 0.;

pub fn ensure_overlay(
    app: &tauri::AppHandle,
    monitor: &MonitorInfo,
    hidden: bool,
) -> Option<(WebviewWindow, bool)> {
    const LABEL: &str = "pet-0";
    if let Some(window) = app.get_webview_window(LABEL) {
        // Logical coordinates are pre-converted against the target screen's
        // scale because tao converts `set_size` with the window's *current*
        // screen scale, and the move is applied only after both calls return.
        // Position must land before size so that conversion is the right one.
        let scale = monitor.scale;
        let position =
            tauri::LogicalPosition::new(monitor.bounds.x / scale, monitor.bounds.y / scale);
        let size = tauri::LogicalSize::new(
            (monitor.bounds.width - OVERLAY_INSET) / scale,
            monitor.bounds.height / scale,
        );
        if let Err(error) = window.set_position(position) {
            eprintln!("Ledgeling could not move the overlay: {error}");
        }
        if let Err(error) = window.set_size(size) {
            eprintln!("Ledgeling could not resize the overlay: {error}");
        }
        return Some((window, false));
    }
    let built = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("index.html?pet&monitor=0".into()),
    )
    .title("Ledgeling")
    .inner_size(
        (monitor.bounds.width - OVERLAY_INSET) / monitor.scale,
        monitor.bounds.height / monitor.scale,
    )
    .position(
        monitor.bounds.x / monitor.scale,
        monitor.bounds.y / monitor.scale,
    )
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .accept_first_mouse(true)
    .visible_on_all_workspaces(true)
    .focusable(false)
    .visible(!hidden)
    .build();
    match built {
        Ok(window) => {
            crate::platform::pet_ui::overlay(&window);
            let _ = window.set_ignore_cursor_events(true);
            Some((window, true))
        }
        Err(error) => {
            eprintln!("Ledgeling could not create an overlay: {error}");
            None
        }
    }
}
