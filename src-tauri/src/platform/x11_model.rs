//! The Linux adapter's decisions, kept apart from the X connection so they can
//! be tested on any machine: which windows are perches, where their visible
//! edge is, what counts as fullscreen, and which session we are in.
use crate::world::{MonitorInfo, Rect, WindowInfo};

/// Smaller windows are palettes, splash remnants or tray icons, not perches.
const MINIMUM_WIDTH: f64 = 80.;
const MINIMUM_HEIGHT: f64 = 40.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    X11,
    Wayland,
}

/// `XDG_SESSION_TYPE` is authoritative when set; `WAYLAND_DISPLAY` alone also
/// means Wayland, since an X11 session does not normally export it.
pub fn session(xdg_session_type: Option<&str>, wayland_display: Option<&str>) -> Session {
    match xdg_session_type.map(str::to_ascii_lowercase).as_deref() {
        Some("wayland") => Session::Wayland,
        Some("x11") => Session::X11,
        _ if wayland_display.is_some_and(|d| !d.is_empty()) => Session::Wayland,
        _ => Session::X11,
    }
}

/// `_NET_WM_WINDOW_TYPE`, reduced to what decides whether a window is a perch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowType {
    Normal,
    Dialog,
    Utility,
    Toolbar,
    Dock,
    Desktop,
    Menu,
    Tooltip,
    Notification,
    Splash,
    Other,
}

/// `_NET_WM_STATE` flags the adapter acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowState {
    Hidden,
    Fullscreen,
    SkipTaskbar,
    Above,
}

/// Left, right, top, bottom — the order both `_NET_FRAME_EXTENTS` and
/// `_GTK_FRAME_EXTENTS` use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extents {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}
impl Extents {
    pub fn from_cardinals(values: &[u32]) -> Option<Self> {
        match values {
            [left, right, top, bottom, ..] => Some(Self {
                left: *left,
                right: *right,
                top: *top,
                bottom: *bottom,
            }),
            _ => None,
        }
    }
}

/// What the X server reports about one managed client window.
#[derive(Clone, Debug, PartialEq)]
pub struct X11Window {
    pub id: u32,
    pub viewable: bool,
    /// Client-area origin in root coordinates, and its size.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Empty when the property is absent, which EWMH says means normal.
    pub types: Vec<WindowType>,
    pub states: Vec<WindowState>,
    /// Decorations the window manager draws outside the client area.
    pub frame: Option<Extents>,
    /// Shadow a client-side-decorated GTK window draws inside its own area.
    pub gtk_frame: Option<Extents>,
    pub pid: Option<u32>,
}

impl X11Window {
    fn has(&self, state: WindowState) -> bool {
        self.states.contains(&state)
    }

    /// The edge a person sees: the client area grown by the window manager's
    /// frame and shrunk by a GTK shadow. Without both, pets hover beside
    /// title bars or sit tens of pixels off client-decorated windows.
    pub fn visible_rect(&self) -> Rect {
        let frame = self.frame.unwrap_or_default();
        let shadow = self.gtk_frame.unwrap_or_default();
        let left = frame.left as f64 - shadow.left as f64;
        let right = frame.right as f64 - shadow.right as f64;
        let top = frame.top as f64 - shadow.top as f64;
        let bottom = frame.bottom as f64 - shadow.bottom as f64;
        Rect::new(
            self.x as f64 - left,
            self.y as f64 - top,
            (self.width as f64 + left + right).max(0.),
            (self.height as f64 + top + bottom).max(0.),
        )
    }

    /// Ledgeling's overlay is always-on-top and kept off the taskbar; its companion
    /// window is neither, so pets can still perch on that one.
    fn is_perch_overlay(&self, own_pid: u32) -> bool {
        self.pid == Some(own_pid)
            && (self.has(WindowState::SkipTaskbar) || self.has(WindowState::Above))
    }

    fn is_application_window(&self) -> bool {
        self.types.is_empty()
            || self
                .types
                .iter()
                .any(|t| matches!(t, WindowType::Normal | WindowType::Dialog))
    }
}

/// Turn `_NET_CLIENT_LIST_STACKING` (bottom-to-top) into the trait's
/// front-to-back list, dropping everything that is not a visible perch.
pub fn front_to_back(
    stacking: &[X11Window],
    own_pid: u32,
    monitor_for: impl Fn(Rect) -> u64,
) -> Vec<WindowInfo> {
    let mut windows = Vec::new();
    for window in stacking.iter().rev() {
        if !window.viewable
            || window.has(WindowState::Hidden)
            || !window.is_application_window()
            || window.is_perch_overlay(own_pid)
        {
            continue;
        }
        let rect = window.visible_rect();
        if rect.width < MINIMUM_WIDTH || rect.height < MINIMUM_HEIGHT {
            continue;
        }
        windows.push(WindowInfo {
            id: window.id as u64,
            rect,
            z_index: windows.len(),
            is_minimized: false,
            monitor_id: monitor_for(rect),
        });
    }
    windows
}

/// A focused window that says it is fullscreen, or that covers a whole
/// monitor panels included, is the signature of a game or a film. Ledgeling's own
/// overlay covers the pet screen too, and a click on a pet can focus it.
pub fn is_fullscreen(window: &X11Window, monitors: &[MonitorInfo], own_pid: u32) -> bool {
    if !window.viewable || window.has(WindowState::Hidden) || window.is_perch_overlay(own_pid) {
        return false;
    }
    let rect = window.visible_rect();
    monitors.iter().any(|m| {
        let covers = (rect.x - m.bounds.x).abs() < 2.
            && (rect.y - m.bounds.y).abs() < 2.
            && (rect.width - m.bounds.width).abs() < 2.
            && (rect.height - m.bounds.height).abs() < 2.;
        covers || (window.has(WindowState::Fullscreen) && m.bounds.contains(rect.center()))
    })
}

/// The executable's file name, lowercased — from `/proc/<pid>/exe` when it is
/// readable, else `comm`. Never the command line: that carries file names.
pub fn process_name(exe: Option<&str>, comm: Option<&str>) -> Option<String> {
    let from_exe = exe
        .map(|path| path.trim_end_matches(" (deleted)"))
        .and_then(|path| path.rsplit('/').next());
    from_exe
        .or(comm)
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{engine::Engine, platform::mock::MockPlatform, settings::Settings};
    use std::collections::HashMap;

    const OWN_PID: u32 = 4242;

    fn window(id: u32, x: i32, y: i32, width: u32, height: u32) -> X11Window {
        X11Window {
            id,
            viewable: true,
            x,
            y,
            width,
            height,
            types: vec![WindowType::Normal],
            states: vec![],
            frame: None,
            gtk_frame: None,
            pid: Some(1000 + id),
        }
    }

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            id: 0,
            bounds: Rect::new(0., 0., 1920., 1080.),
            work: Rect::new(0., 32., 1920., 1048.),
            scale: 1.,
            is_primary: true,
        }
    }

    fn ids(windows: &[WindowInfo]) -> Vec<u64> {
        windows.iter().map(|w| w.id).collect()
    }

    #[test]
    fn stacking_order_is_reversed_to_front_to_back() {
        let stacking = [
            window(1, 0, 0, 800, 600),
            window(2, 100, 100, 800, 600),
            window(3, 200, 200, 800, 600),
        ];
        let windows = front_to_back(&stacking, OWN_PID, |_| 0);
        assert_eq!(ids(&windows), [3, 2, 1]);
        assert_eq!(
            windows.iter().map(|w| w.z_index).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn frame_extents_put_the_top_edge_on_the_title_bar() {
        let mut w = window(1, 300, 237, 800, 600);
        w.frame = Extents::from_cardinals(&[1, 1, 37, 1]);
        assert_eq!(w.visible_rect(), Rect::new(299., 200., 802., 638.));
    }

    #[test]
    fn a_gtk_shadow_is_not_part_of_the_visible_window() {
        let mut w = window(1, 100, 100, 852, 652);
        w.gtk_frame = Extents::from_cardinals(&[26, 26, 23, 29]);
        assert_eq!(w.visible_rect(), Rect::new(126., 123., 800., 600.));
    }

    #[test]
    fn short_extents_properties_are_ignored() {
        assert_eq!(Extents::from_cardinals(&[4, 4, 30]), None);
        assert_eq!(Extents::from_cardinals(&[]), None);
    }

    #[test]
    fn unmapped_minimised_and_non_application_windows_are_skipped() {
        let mut unmapped = window(1, 0, 0, 800, 600);
        unmapped.viewable = false;
        let mut minimised = window(2, 0, 0, 800, 600);
        minimised.states = vec![WindowState::Hidden];
        let mut dock = window(3, 0, 1040, 1920, 40);
        dock.types = vec![WindowType::Dock];
        let mut desktop = window(4, 0, 0, 1920, 1080);
        desktop.types = vec![WindowType::Desktop];
        let mut tooltip = window(5, 10, 10, 200, 60);
        tooltip.types = vec![WindowType::Tooltip];
        let mut notification = window(6, 1500, 40, 400, 100);
        notification.types = vec![WindowType::Notification];
        let mut menu = window(7, 10, 10, 200, 300);
        menu.types = vec![WindowType::Menu];
        let mut untyped = window(8, 0, 0, 800, 600);
        untyped.types.clear();
        let mut dialog = window(9, 0, 0, 400, 200);
        dialog.types = vec![WindowType::Dialog];
        let stacking = [
            unmapped,
            minimised,
            dock,
            desktop,
            tooltip,
            notification,
            menu,
            untyped,
            dialog,
        ];
        assert_eq!(ids(&front_to_back(&stacking, OWN_PID, |_| 0)), [9, 8]);
    }

    #[test]
    fn perchs_own_overlay_is_skipped_but_its_companion_window_is_not() {
        let mut overlay = window(1, 0, 0, 1920, 1080);
        overlay.pid = Some(OWN_PID);
        overlay.states = vec![WindowState::Above, WindowState::SkipTaskbar];
        let mut companion = window(2, 400, 200, 1000, 760);
        companion.pid = Some(OWN_PID);
        let mut someone_elses_above = window(3, 100, 100, 600, 400);
        someone_elses_above.states = vec![WindowState::Above];
        let stacking = [companion, someone_elses_above, overlay];
        assert_eq!(ids(&front_to_back(&stacking, OWN_PID, |_| 0)), [3, 2]);
    }

    #[test]
    fn tiny_windows_are_not_perches() {
        let stacking = [window(1, 0, 0, 79, 400), window(2, 0, 0, 400, 39)];
        assert!(front_to_back(&stacking, OWN_PID, |_| 0).is_empty());
    }

    #[test]
    fn fullscreen_is_the_state_flag_or_covering_a_monitor() {
        let monitors = [monitor()];
        let mut flagged = window(1, 0, 0, 1280, 720);
        flagged.states = vec![WindowState::Fullscreen];
        assert!(is_fullscreen(&flagged, &monitors, OWN_PID));
        assert!(is_fullscreen(
            &window(2, 0, 0, 1920, 1080),
            &monitors,
            OWN_PID
        ));
        assert!(!is_fullscreen(
            &window(3, 0, 32, 1920, 1048),
            &monitors,
            OWN_PID
        ));
        let mut elsewhere = flagged.clone();
        elsewhere.x = 3000;
        assert!(
            !is_fullscreen(&elsewhere, &monitors, OWN_PID),
            "not on the pet screen"
        );
        let mut minimised = flagged;
        minimised.states.push(WindowState::Hidden);
        assert!(!is_fullscreen(&minimised, &monitors, OWN_PID));
        assert!(!is_fullscreen(&window(4, 0, 0, 1920, 1080), &[], OWN_PID));
    }

    #[test]
    fn a_focused_perch_overlay_is_not_a_fullscreen_app() {
        let mut overlay = window(1, 0, 0, 1920, 1080);
        overlay.pid = Some(OWN_PID);
        overlay.states = vec![WindowState::Above, WindowState::SkipTaskbar];
        assert!(!is_fullscreen(&overlay, &[monitor()], OWN_PID));
    }

    #[test]
    fn process_names_come_from_the_executable_path() {
        assert_eq!(
            process_name(Some("/usr/lib/firefox/firefox"), Some("GeckoMain")),
            Some("firefox".into())
        );
        assert_eq!(
            process_name(Some("/usr/share/code/code (deleted)"), None),
            Some("code".into())
        );
        assert_eq!(process_name(None, Some("Slack\n")), Some("slack".into()));
        assert_eq!(process_name(None, None), None);
        assert_eq!(process_name(None, Some("  ")), None);
    }

    #[test]
    fn session_type_wins_over_wayland_display() {
        assert_eq!(session(Some("wayland"), None), Session::Wayland);
        assert_eq!(session(Some("x11"), Some("wayland-0")), Session::X11);
        assert_eq!(session(None, Some("wayland-0")), Session::Wayland);
        assert_eq!(session(Some("tty"), Some("")), Session::X11);
        assert_eq!(session(None, None), Session::X11);
    }

    fn engine_with(windows: Vec<WindowInfo>) -> (Engine, MockPlatform) {
        let mut settings = Settings::default();
        settings.pets.clear();
        let mut engine = Engine::new(settings, HashMap::new());
        let platform = MockPlatform::new(vec![monitor()], vec![windows]);
        engine.set_monitors(vec![monitor()]);
        engine.tick(1. / 60., &platform);
        (engine, platform)
    }

    #[test]
    fn a_synthetic_desktop_yields_perches_on_the_visible_frames_only() {
        // A GNOME-like desktop, bottom to top: the desktop window, a terminal
        // with a WM frame, a client-decorated editor with a shadow overlapping
        // it, the top panel, a minimised chat app, and Ledgeling's own overlay.
        let mut desktop = window(1, 0, 0, 1920, 1080);
        desktop.types = vec![WindowType::Desktop];
        let mut terminal = window(2, 200, 337, 900, 500);
        terminal.frame = Extents::from_cardinals(&[0, 0, 37, 0]);
        let mut editor = window(3, 574, 177, 852, 652);
        editor.gtk_frame = Extents::from_cardinals(&[26, 26, 23, 29]);
        let mut panel = window(4, 0, 0, 1920, 32);
        panel.types = vec![WindowType::Dock];
        let mut chat = window(5, 100, 100, 800, 600);
        chat.states = vec![WindowState::Hidden];
        let mut overlay = window(6, 0, 0, 1920, 1080);
        overlay.pid = Some(OWN_PID);
        overlay.states = vec![WindowState::Above, WindowState::SkipTaskbar];
        let stacking = [desktop, terminal, editor, panel, chat, overlay];

        let windows = front_to_back(&stacking, OWN_PID, |_| 0);
        assert_eq!(ids(&windows), [3, 2]);
        let (engine, _) = engine_with(windows);

        let tops: Vec<(u64, f64, f64, f64)> = engine
            .surfaces
            .iter()
            .filter(|s| s.kind == crate::world::SurfaceKind::WindowTop)
            .map(|s| (s.owner, s.start.y, s.start.x, s.end.x))
            .collect();
        // The editor's top sits on its visible edge, not its shadow.
        assert!(tops.contains(&(3, 200., 600., 1400.)), "{tops:?}");
        // The terminal's top is its title bar, cut where the editor covers it.
        assert!(tops.contains(&(2, 300., 200., 600.)), "{tops:?}");
        assert!(
            tops.iter().all(|t| t.0 == 2 || t.0 == 3),
            "no perch on the overlay, the panel, the desktop or a minimised window: {tops:?}"
        );
    }
}
