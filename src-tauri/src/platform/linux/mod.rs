//! Linux adapter. On X11 it reads window geometry, stacking order, EWMH type
//! and state, idle time and the foreground process's executable name. It never
//! asks for a window title, never captures the screen, reads no key state and
//! installs no input hook.
//!
//! Wayland has no common way to list other apps' windows. Where the compositor
//! offers its own (COSMIC, Hyprland, sway, niri; see `sources`), windows come
//! from there, and the focused one's app id stands in for its process name.
//! Elsewhere pets use the screen edges; see docs/verification.md for what each
//! session supports.
mod idle;
pub mod layer;
mod sources;
mod x11;

use super::{
    wayland_model,
    x11_model::{self, Session},
    Platform, ProcessInfo,
};
use crate::world::{MonitorInfo, Point, Rect, WindowInfo};
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Retrying a missing display on every scan would be cheap but pointless.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

fn current_session() -> Session {
    x11_model::session(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

/// On Wayland the overlay is a layer surface where the compositor offers
/// one. Elsewhere — GNOME, chiefly — an ordinary Wayland window can be neither
/// positioned nor kept above other windows, so the toolkit is steered to
/// XWayland when it exists and the user has not chosen a backend themselves.
pub fn prepare_process() {
    if current_session() == Session::Wayland
        && !layer::supported()
        && std::env::var_os("GDK_BACKEND").is_none()
        && std::env::var_os("DISPLAY").is_some()
    {
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

/// Where the pointer is over the layer-surface overlay, in virtual-desktop
/// physical pixels, as the overlay's own pointer events report it. `None`
/// once it has left the pets.
static OVERLAY_POINTER: Mutex<Option<Point>> = Mutex::new(None);

pub fn set_overlay_pointer(point: Option<Point>) {
    if let Ok(mut pointer) = OVERLAY_POINTER.lock() {
        *pointer = point;
    }
}

struct Display {
    connection: Option<x11::X11>,
    last_attempt: Option<Instant>,
}

pub struct NativePlatform {
    session: Session,
    display: Mutex<Display>,
    monitors: Mutex<Vec<MonitorInfo>>,
    wayland_idle: OnceLock<idle::WaylandIdle>,
    /// The compositor's window source, looked for once, on first use.
    wayland_windows: OnceLock<Option<Box<dyn sources::WindowSource>>>,
}

impl Default for NativePlatform {
    fn default() -> Self {
        Self {
            session: current_session(),
            display: Mutex::new(Display {
                connection: None,
                last_attempt: None,
            }),
            monitors: Mutex::new(Vec::new()),
            wayland_idle: OnceLock::new(),
            wayland_windows: OnceLock::new(),
        }
    }
}

impl NativePlatform {
    /// Runs `query` against the X server, connecting lazily and dropping a
    /// connection that failed so the next call starts afresh.
    fn with_x11<T>(&self, query: impl FnOnce(&x11::X11) -> Result<T, String>) -> Result<T, String> {
        let mut display = self
            .display
            .lock()
            .map_err(|_| "X11 state is unavailable")?;
        if display.connection.is_none() {
            if display
                .last_attempt
                .is_some_and(|at| at.elapsed() < RECONNECT_INTERVAL)
            {
                return Err("No X display is reachable. Using the desktop edge.".into());
            }
            display.last_attempt = Some(Instant::now());
            display.connection = Some(x11::X11::connect()?);
        }
        let connection = display.connection.as_ref().expect("connected above");
        let result = query(connection);
        if result.is_err() && !connection.is_healthy() {
            display.connection = None;
        }
        result
    }

    fn window_source(&self) -> Option<&dyn sources::WindowSource> {
        self.wayland_windows
            .get_or_init(|| {
                let source = sources::detect();
                if std::env::var_os("LEDGELING_DIAGNOSTICS").is_some() {
                    let name = source.as_ref().map_or("none", |s| s.name());
                    eprintln!("Ledgeling's Wayland window source: {name}");
                }
                source
            })
            .as_deref()
    }

    /// Compositors lay windows out in logical pixels; the pet screen's scale
    /// turns them into the physical pixels the engine works in.
    fn layout_scale(&self) -> f64 {
        self.monitors
            .lock()
            .ok()
            .and_then(|m| m.first().map(|m| m.scale))
            .unwrap_or(1.)
    }

    fn monitor_for(&self, rect: Rect) -> u64 {
        self.monitors
            .lock()
            .ok()
            .and_then(|monitors| {
                monitors
                    .iter()
                    .find(|m| m.bounds.contains(rect.center()))
                    .or_else(|| monitors.first())
                    .map(|m| m.id)
            })
            .unwrap_or(0)
    }
}

impl Platform for NativePlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String> {
        if self.session == Session::Wayland {
            // XWayland lists only X clients; perching on those would put pets
            // on edges that native Wayland windows are covering.
            let Some(source) = self.window_source() else {
                return Err(
                    "This Wayland desktop does not let apps see other windows. Using the desktop edge."
                        .into(),
                );
            };
            let toplevels = source.toplevels()?;
            return Ok(wayland_model::front_to_back(
                &toplevels,
                self.layout_scale(),
                std::process::id(),
                |rect| self.monitor_for(rect),
            ));
        }
        let own_pid = std::process::id();
        self.with_x11(|x| {
            let stacking = x.describe(&x.stacking()?);
            Ok(x11_model::front_to_back(&stacking, own_pid, |rect| {
                self.monitor_for(rect)
            }))
        })
    }

    fn idle_seconds(&self) -> Result<f64, String> {
        match self.session {
            Session::X11 => self.with_x11(|x| x.idle_seconds()),
            // XWayland only sees input aimed at X clients.
            Session::Wayland => self
                .wayland_idle
                .get_or_init(idle::WaylandIdle::connect)
                .seconds(),
        }
    }

    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String> {
        if self.session == Session::Wayland {
            // Without a window source only XWayland apps could be named, and
            // the awareness layer keeps the last category it saw, so one
            // XWayland game would stick.
            let Some(source) = self.window_source() else {
                return Ok(None);
            };
            let toplevels = source.toplevels()?;
            let Some(focused) = wayland_model::focused(&toplevels) else {
                return Ok(None);
            };
            // The executable is what the category map knows best; the app id
            // stands in where the compositor gives no process.
            if let Some(info) = focused.pid.and_then(process) {
                return Ok(Some(info));
            }
            return Ok(focused
                .app_id
                .as_deref()
                .and_then(wayland_model::app_name)
                .map(|name| ProcessInfo { name, pid: 0 }));
        }
        let Some(pid) = self.with_x11(|x| x.active_pid())? else {
            return Ok(None);
        };
        Ok(process(pid))
    }

    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        let cache = self
            .monitors
            .lock()
            .map_err(|_| "Monitor geometry is unavailable")?;
        if cache.is_empty() {
            Err("No monitor geometry has been reported yet".into())
        } else {
            Ok(cache.clone())
        }
    }

    fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        if let Ok(mut cache) = self.monitors.lock() {
            *cache = monitors;
        }
    }

    fn fullscreen_app_active(&self) -> Result<bool, String> {
        let monitors = self.monitors().unwrap_or_default();
        if monitors.is_empty() {
            return Ok(false);
        }
        if self.session == Session::Wayland {
            if let Some(source) = self.window_source() {
                let toplevels = source.toplevels()?;
                return Ok(wayland_model::fullscreen_active(
                    &toplevels,
                    self.layout_scale(),
                    &monitors,
                ));
            }
        }
        // On Wayland this still sees fullscreen XWayland apps, which is where
        // most games run; a focused native window reports no active X window.
        let own_pid = std::process::id();
        self.with_x11(|x| {
            Ok(x.active_window()?
                .and_then(|window| x.describe(&[window]).pop())
                .is_some_and(|window| x11_model::is_fullscreen(&window, &monitors, own_pid)))
        })
    }

    fn cursor(&self) -> Option<Point> {
        if layer::active() {
            return OVERLAY_POINTER.lock().ok().and_then(|pointer| *pointer);
        }
        // On Wayland, XWayland only learns where the pointer is while it is
        // over an X window, so this goes stale over native Wayland windows.
        self.with_x11(|x| x.pointer()).ok()
    }

    fn mouse_down(&self) -> Option<bool> {
        // The pointer query's button mask also carries modifier-key state,
        // and Ledgeling reads no key state. The overlay's pointer capture reports
        // the release instead, as on Windows.
        None
    }
}

/// The executable's name for a process id: `/proc/<pid>/exe`, else `comm`.
fn process(pid: u32) -> Option<ProcessInfo> {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok();
    let name = x11_model::process_name(exe.as_ref().and_then(|p| p.to_str()), comm.as_deref());
    name.map(|name| ProcessInfo { name, pid })
}
