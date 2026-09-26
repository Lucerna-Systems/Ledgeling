//! Linux adapter. On X11 it reads window geometry, stacking order, EWMH type
//! and state, idle time and the foreground process's executable name. It never
//! asks for a window title, never captures the screen, reads no key state and
//! installs no input hook.
//!
//! On Wayland other apps' windows cannot be listed by design, so pets use the
//! screen edges; see docs/verification.md for what each session supports.
mod idle;
mod x11;

use super::{
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

/// A native Wayland overlay can be neither positioned nor kept above other
/// windows, so on Wayland the toolkit is steered to XWayland when it exists
/// and the user has not chosen a backend themselves.
pub fn prepare_process() {
    if current_session() == Session::Wayland
        && std::env::var_os("GDK_BACKEND").is_none()
        && std::env::var_os("DISPLAY").is_some()
    {
        std::env::set_var("GDK_BACKEND", "x11");
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
            return Err(
                "Wayland does not let apps see other windows. Using the desktop edge.".into(),
            );
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
            // Only XWayland apps could be named, and the awareness layer keeps
            // the last category it saw, so one XWayland game would stick.
            return Ok(None);
        }
        let Some(pid) = self.with_x11(|x| x.active_pid())? else {
            return Ok(None);
        };
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok();
        let name = x11_model::process_name(exe.as_ref().and_then(|p| p.to_str()), comm.as_deref());
        Ok(name.map(|name| ProcessInfo { name, pid }))
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
