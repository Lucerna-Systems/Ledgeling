//! Live checks of the Linux adapter against the display this process is given.
//! They open real windows, so they are ignored by default. CI runs them under
//! Xvfb with openbox and under headless sway; on a desktop, run
//! `cargo test --test linux_desktop -- --ignored --test-threads=1` and keep
//! your hands off the mouse while the windows flash past.
#![cfg(target_os = "linux")]

use ledgeling::{
    platform::{NativePlatform, Platform},
    world::{MonitorInfo, Rect, WindowInfo},
};
use std::time::{Duration, Instant};
use x11rb::{
    connection::Connection,
    properties::{WmHints, WmSizeHints, WmSizeHintsSpecification},
    protocol::xproto::{
        AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
        Window, WindowClass,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT,
};

struct Display {
    connection: RustConnection,
    root: Window,
    width: u16,
    height: u16,
    white: u32,
    opened: Vec<Window>,
}

impl Display {
    fn connect() -> Self {
        let (connection, screen) =
            x11rb::connect(None).expect("these tests need DISPLAY to point at an X server");
        let s = &connection.setup().roots[screen];
        let (root, width, height, white) =
            (s.root, s.width_in_pixels, s.height_in_pixels, s.white_pixel);
        Self {
            connection,
            root,
            width,
            height,
            white,
            opened: Vec::new(),
        }
    }

    fn atom(&self, name: &str) -> u32 {
        self.connection
            .intern_atom(false, name.as_bytes())
            .unwrap()
            .reply()
            .unwrap()
            .atom
    }

    fn monitor(&self) -> MonitorInfo {
        let bounds = Rect::new(0., 0., self.width as f64, self.height as f64);
        MonitorInfo {
            id: 0,
            bounds,
            work: bounds,
            scale: 1.,
            is_primary: true,
        }
    }

    /// A managed window at a user-specified position, so the window manager
    /// does not place it wherever it likes.
    fn open(
        &mut self,
        rect: (i16, i16, u16, u16),
        kind: &str,
        states: &[&str],
        focusable: bool,
    ) -> Window {
        let c = &self.connection;
        let window = c.generate_id().unwrap();
        c.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            self.root,
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(self.white),
        )
        .unwrap();
        let mut hints = WmSizeHints::new();
        hints.position = Some((
            WmSizeHintsSpecification::UserSpecified,
            rect.0 as i32,
            rect.1 as i32,
        ));
        hints.size = Some((
            WmSizeHintsSpecification::UserSpecified,
            rect.2 as i32,
            rect.3 as i32,
        ));
        hints.set_normal_hints(c, window).unwrap();
        let mut wm_hints = WmHints::new();
        wm_hints.input = Some(focusable);
        wm_hints.set(c, window).unwrap();
        let kind = self.atom(&format!("_NET_WM_WINDOW_TYPE_{kind}"));
        let type_atom = self.atom("_NET_WM_WINDOW_TYPE");
        c.change_property32(
            PropMode::REPLACE,
            window,
            type_atom,
            AtomEnum::ATOM,
            &[kind],
        )
        .unwrap();
        let states: Vec<u32> = states
            .iter()
            .map(|s| self.atom(&format!("_NET_WM_STATE_{s}")))
            .collect();
        let state_atom = self.atom("_NET_WM_STATE");
        c.change_property32(
            PropMode::REPLACE,
            window,
            state_atom,
            AtomEnum::ATOM,
            &states,
        )
        .unwrap();
        let pid_atom = self.atom("_NET_WM_PID");
        c.change_property32(
            PropMode::REPLACE,
            window,
            pid_atom,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .unwrap();
        c.map_window(window).unwrap();
        c.flush().unwrap();
        self.opened.push(window);
        window
    }

    /// What a pager does: ask the window manager, never change state directly.
    fn ask_wm(&self, window: Window, message: &str, data: [u32; 5]) {
        let event = ClientMessageEvent::new(32, window, self.atom(message), data);
        self.connection
            .send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )
            .unwrap();
        self.connection.flush().unwrap();
    }

    fn frame_extents(&self, window: Window) -> Option<[u32; 4]> {
        let reply = self
            .connection
            .get_property(
                false,
                window,
                self.atom("_NET_FRAME_EXTENTS"),
                AtomEnum::CARDINAL,
                0,
                4,
            )
            .unwrap()
            .reply()
            .unwrap();
        let values: Vec<u32> = reply.value32()?.collect();
        values.try_into().ok()
    }

    fn client_origin(&self, window: Window) -> (f64, f64) {
        let reply = self
            .connection
            .translate_coordinates(window, self.root, 0, 0)
            .unwrap()
            .reply()
            .unwrap();
        (reply.dst_x as f64, reply.dst_y as f64)
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        for window in &self.opened {
            let _ = self.connection.destroy_window(*window);
        }
        let _ = self.connection.flush();
    }
}

fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timed out waiting until {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Our own windows, in the order the adapter reported them.
fn ours(platform: &NativePlatform, opened: &[Window]) -> Vec<WindowInfo> {
    platform
        .enumerate_windows()
        .expect("the X11 adapter lists windows")
        .into_iter()
        .filter(|w| opened.contains(&(w.id as Window)))
        .collect()
}

fn ids(windows: &[WindowInfo]) -> Vec<Window> {
    windows.iter().map(|w| w.id as Window).collect()
}

#[test]
#[ignore = "opens windows on a live X server"]
fn x11_adapter_sees_the_desktop_as_a_person_does() {
    assert_ne!(
        std::env::var("XDG_SESSION_TYPE").as_deref(),
        Ok("wayland"),
        "run this in an X11 session"
    );
    let mut display = Display::connect();
    let platform = NativePlatform::default();
    platform.set_monitors(vec![display.monitor()]);

    // Three overlapping windows, opened back to front.
    let a = display.open((100, 120, 600, 400), "NORMAL", &[], true);
    let b = display.open((300, 260, 600, 400), "NORMAL", &[], true);
    let c = display.open((500, 400, 600, 400), "NORMAL", &[], true);
    // Things that are not perches: a panel, a palette, and an overlay that
    // carries this process's PID the way Perch's own does.
    let dock = display.open((0, 0, display.width, 32), "DOCK", &[], false);
    let palette = display.open((1200, 100, 200, 300), "UTILITY", &[], false);
    let overlay = display.open(
        (0, 0, display.width, display.height),
        "NORMAL",
        &["ABOVE", "SKIP_TASKBAR"],
        false,
    );
    let opened = display.opened.clone();
    wait_for("the three windows are listed", || {
        ours(&platform, &opened).len() >= 3
    });

    let listed = ours(&platform, &opened);
    eprintln!("listed: {listed:#?}");
    assert_eq!(ids(&listed), [c, b, a], "front to back, newest on top");
    for excluded in [dock, palette, overlay] {
        assert!(
            !ids(&listed).contains(&excluded),
            "{excluded:#x} is not a perch"
        );
    }

    // The top edge is the frame the window manager drew, not the client area.
    let (client_x, client_y) = display.client_origin(a);
    let rect = listed.iter().find(|w| w.id == a as u64).unwrap().rect;
    match display.frame_extents(a) {
        Some([left, _, top, _]) => {
            eprintln!("frame extents of A: left {left}, top {top}");
            assert_eq!(rect.x, client_x - left as f64);
            assert_eq!(rect.y, client_y - top as f64);
        }
        None => assert_eq!((rect.x, rect.y), (client_x, client_y)),
    }

    // Minimising removes a window; activating one brings it to the front.
    display.ask_wm(b, "WM_CHANGE_STATE", [3, 0, 0, 0, 0]);
    wait_for("the minimised window is gone", || {
        !ids(&ours(&platform, &opened)).contains(&b)
    });
    display.ask_wm(a, "_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0]);
    wait_for("the activated window is in front", || {
        ids(&ours(&platform, &opened)).first() == Some(&a)
    });

    // The foreground process is this test binary, named by its executable.
    wait_for("a foreground process is reported", || {
        platform.foreground_process().ok().flatten().is_some()
    });
    let foreground = platform.foreground_process().unwrap().unwrap();
    eprintln!("foreground: {foreground:?}");
    assert_eq!(foreground.pid, std::process::id());
    assert!(
        foreground.name.starts_with("linux_desktop"),
        "{}",
        foreground.name
    );

    // Fullscreen on the focused window suspends pets; leaving it resumes.
    assert!(!platform.fullscreen_app_active().unwrap());
    let fullscreen = display.atom("_NET_WM_STATE_FULLSCREEN");
    display.ask_wm(a, "_NET_WM_STATE", [1, fullscreen, 0, 2, 0]);
    wait_for("fullscreen is detected", || {
        platform.fullscreen_app_active().unwrap()
    });
    display.ask_wm(a, "_NET_WM_STATE", [0, fullscreen, 0, 2, 0]);
    wait_for("fullscreen ends", || {
        !platform.fullscreen_app_active().unwrap()
    });

    // The cursor is read in root pixels.
    display
        .connection
        .warp_pointer(x11rb::NONE, display.root, 0, 0, 0, 0, 321, 234)
        .unwrap();
    display.connection.flush().unwrap();
    wait_for("the cursor is read", || {
        platform
            .cursor()
            .is_some_and(|p| (p.x, p.y) == (321., 234.))
    });
    assert_eq!(platform.mouse_down(), None, "no input state is read");

    let idle = platform
        .idle_seconds()
        .expect("the X server reports idle time");
    eprintln!("idle: {idle:.1} s");
    assert!(idle.is_finite() && idle >= 0.);

    // The budget is 3 ms per scan with a realistic number of windows.
    for i in 0..20 {
        display.open((40 + i * 30, 60 + i * 20, 500, 350), "NORMAL", &[], true);
    }
    let opened = display.opened.clone();
    wait_for("twenty more windows are listed", || {
        ours(&platform, &opened).len() >= 22
    });
    let mut timings: Vec<f64> = (0..50)
        .map(|_| {
            let started = Instant::now();
            platform.enumerate_windows().unwrap();
            started.elapsed().as_secs_f64() * 1000.
        })
        .collect();
    timings.sort_by(f64::total_cmp);
    let total = platform.enumerate_windows().unwrap().len();
    eprintln!(
        "scan of {total} windows: median {:.3} ms, worst {:.3} ms",
        timings[25], timings[49]
    );
    assert!(
        timings[25] < 3.,
        "median scan {:.3} ms is over budget",
        timings[25]
    );
}

#[test]
#[ignore = "needs a live Wayland compositor"]
fn wayland_session_degrades_to_screen_edges() {
    assert!(
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        "run this inside a Wayland session"
    );
    let platform = NativePlatform::default();
    let error = platform.enumerate_windows().unwrap_err();
    eprintln!("enumerate: {error}");
    assert!(error.contains("Wayland"));
    assert_eq!(platform.foreground_process().unwrap(), None);

    // A compositor with ext-idle-notify (or GNOME's idle monitor) reports idle
    // time; a headless one has no input at all, so it goes idle at once.
    let idle = platform
        .idle_seconds()
        .expect("the compositor reports idle time");
    std::thread::sleep(Duration::from_millis(2500));
    let later = platform.idle_seconds().unwrap();
    eprintln!("idle: {idle:.1} s, then {later:.1} s");
    assert!(later >= 1., "idle time grows while nothing happens");

    if std::env::var_os("DISPLAY").is_some() {
        std::env::remove_var("GDK_BACKEND");
        ledgeling::platform::prepare_process();
        assert_eq!(std::env::var("GDK_BACKEND").as_deref(), Ok("x11"));
        eprintln!("cursor via XWayland: {:?}", platform.cursor());
    }
}
