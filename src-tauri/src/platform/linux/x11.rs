//! The X11 queries. Every per-window request in a scan is sent before any
//! reply is read, so a scan costs about one round trip however many windows
//! are open.
use crate::platform::x11_model::{Extents, WindowState, WindowType, X11Window};
use crate::world::Point;
use x11rb::{
    connection::{Connection, RequestConnection},
    cookie::Cookie,
    protocol::{
        res::{self, ConnectionExt as _},
        screensaver::{self, ConnectionExt as _},
        xproto::{AtomEnum, ConnectionExt as _, GetPropertyReply, MapState, Window},
    },
    rust_connection::RustConnection,
};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        _NET_CLIENT_LIST_STACKING,
        _NET_ACTIVE_WINDOW,
        _NET_WM_PID,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_STATE,
        _NET_FRAME_EXTENTS,
        _GTK_FRAME_EXTENTS,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _NET_WM_WINDOW_TYPE_TOOLBAR,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
        _NET_WM_WINDOW_TYPE_POPUP_MENU,
        _NET_WM_WINDOW_TYPE_TOOLTIP,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_ABOVE,
    }
}

/// Enough for any real type or state list, and for four extents.
const PROPERTY_LENGTH: u32 = 64;

pub struct X11 {
    connection: RustConnection,
    root: Window,
    atoms: Atoms,
    screensaver: bool,
    resource: bool,
}

type PropertyCookie<'a> = Cookie<'a, RustConnection, GetPropertyReply>;

fn error(e: impl std::fmt::Display) -> String {
    format!("The X server did not answer ({e}). Using the desktop edge.")
}

impl X11 {
    pub fn connect() -> Result<Self, String> {
        let (connection, screen) = x11rb::connect(None).map_err(error)?;
        let root = connection.setup().roots[screen].root;
        let atoms = Atoms::new(&connection)
            .map_err(error)?
            .reply()
            .map_err(error)?;
        let screensaver = connection
            .extension_information(screensaver::X11_EXTENSION_NAME)
            .map_err(error)?
            .is_some();
        // The client-id query needs version 1.2 negotiated before it answers.
        let resource = connection
            .extension_information(res::X11_EXTENSION_NAME)
            .map_err(error)?
            .is_some()
            && connection
                .res_query_version(1, 2)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|v| (v.server_major, v.server_minor) >= (1, 2));
        Ok(Self {
            connection,
            root,
            atoms,
            screensaver,
            resource,
        })
    }

    /// A dead socket makes every later request fail; a vanished window does not.
    pub fn is_healthy(&self) -> bool {
        self.connection
            .get_input_focus()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some()
    }

    fn property(&self, window: Window, atom: u32) -> Result<PropertyCookie<'_>, String> {
        self.connection
            .get_property(false, window, atom, AtomEnum::ANY, 0, PROPERTY_LENGTH)
            .map_err(error)
    }

    /// Managed client windows, bottom-to-top, as the window manager stacks them.
    pub fn stacking(&self) -> Result<Vec<Window>, String> {
        let reply = self
            .property(self.root, self.atoms._NET_CLIENT_LIST_STACKING)?
            .reply()
            .map_err(error)?;
        let windows: Option<Vec<Window>> = reply.value32().map(|w| w.collect());
        windows.ok_or_else(|| {
            "The window manager does not publish a stacking order. Using the desktop edge.".into()
        })
    }

    /// Geometry, EWMH type and state, frame extents and PID of each window.
    /// Windows that vanish mid-scan are left out.
    pub fn describe(&self, windows: &[Window]) -> Vec<X11Window> {
        let a = &self.atoms;
        let mut pending = Vec::with_capacity(windows.len());
        for &window in windows {
            let c = &self.connection;
            let requests = (
                c.get_window_attributes(window),
                c.get_geometry(window),
                c.translate_coordinates(window, self.root, 0, 0),
                self.property(window, a._NET_WM_WINDOW_TYPE),
                self.property(window, a._NET_WM_STATE),
                self.property(window, a._NET_FRAME_EXTENTS),
                self.property(window, a._GTK_FRAME_EXTENTS),
                self.property(window, a._NET_WM_PID),
            );
            pending.push((window, requests));
        }
        let mut described = Vec::with_capacity(windows.len());
        for (window, (attributes, geometry, origin, types, states, frame, gtk, pid)) in pending {
            let (Ok(attributes), Ok(geometry), Ok(origin)) = (attributes, geometry, origin) else {
                continue;
            };
            let (Ok(attributes), Ok(geometry), Ok(origin)) =
                (attributes.reply(), geometry.reply(), origin.reply())
            else {
                continue;
            };
            let values = |cookie: Result<PropertyCookie<'_>, String>| -> Vec<u32> {
                cookie
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .and_then(|r| r.value32().map(|v| v.collect()))
                    .unwrap_or_default()
            };
            described.push(X11Window {
                id: window,
                viewable: attributes.map_state == MapState::VIEWABLE,
                x: origin.dst_x as i32,
                y: origin.dst_y as i32,
                width: geometry.width as u32,
                height: geometry.height as u32,
                types: values(types)
                    .into_iter()
                    .map(|t| self.window_type(t))
                    .collect(),
                states: values(states)
                    .into_iter()
                    .filter_map(|s| self.window_state(s))
                    .collect(),
                frame: Extents::from_cardinals(&values(frame)),
                gtk_frame: Extents::from_cardinals(&values(gtk)),
                pid: values(pid).first().copied(),
            });
        }
        described
    }

    fn window_type(&self, atom: u32) -> WindowType {
        let a = &self.atoms;
        match atom {
            t if t == a._NET_WM_WINDOW_TYPE_NORMAL => WindowType::Normal,
            t if t == a._NET_WM_WINDOW_TYPE_DIALOG => WindowType::Dialog,
            t if t == a._NET_WM_WINDOW_TYPE_UTILITY => WindowType::Utility,
            t if t == a._NET_WM_WINDOW_TYPE_TOOLBAR => WindowType::Toolbar,
            t if t == a._NET_WM_WINDOW_TYPE_DOCK => WindowType::Dock,
            t if t == a._NET_WM_WINDOW_TYPE_DESKTOP => WindowType::Desktop,
            t if t == a._NET_WM_WINDOW_TYPE_MENU
                || t == a._NET_WM_WINDOW_TYPE_DROPDOWN_MENU
                || t == a._NET_WM_WINDOW_TYPE_POPUP_MENU =>
            {
                WindowType::Menu
            }
            t if t == a._NET_WM_WINDOW_TYPE_TOOLTIP => WindowType::Tooltip,
            t if t == a._NET_WM_WINDOW_TYPE_NOTIFICATION => WindowType::Notification,
            t if t == a._NET_WM_WINDOW_TYPE_SPLASH => WindowType::Splash,
            _ => WindowType::Other,
        }
    }

    fn window_state(&self, atom: u32) -> Option<WindowState> {
        let a = &self.atoms;
        match atom {
            s if s == a._NET_WM_STATE_HIDDEN => Some(WindowState::Hidden),
            s if s == a._NET_WM_STATE_FULLSCREEN => Some(WindowState::Fullscreen),
            s if s == a._NET_WM_STATE_SKIP_TASKBAR => Some(WindowState::SkipTaskbar),
            s if s == a._NET_WM_STATE_ABOVE => Some(WindowState::Above),
            _ => None,
        }
    }

    pub fn active_window(&self) -> Result<Option<Window>, String> {
        let reply = self
            .property(self.root, self.atoms._NET_ACTIVE_WINDOW)?
            .reply()
            .map_err(error)?;
        Ok(reply
            .value32()
            .and_then(|mut v| v.next())
            .filter(|&w| w != x11rb::NONE))
    }

    /// The server's own record of which process opened the connection is
    /// preferred: `_NET_WM_PID` is whatever the client wrote, and a sandboxed
    /// app writes a PID from its own namespace.
    pub fn active_pid(&self) -> Result<Option<u32>, String> {
        let Some(window) = self.active_window()? else {
            return Ok(None);
        };
        if self.resource {
            let spec = res::ClientIdSpec {
                client: window,
                mask: res::ClientIdMask::LOCAL_CLIENT_PID,
            };
            let from_server = self
                .connection
                .res_query_client_ids(&[spec])
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|reply| {
                    reply
                        .ids
                        .iter()
                        .find(|id| id.spec.mask.contains(res::ClientIdMask::LOCAL_CLIENT_PID))
                        .and_then(|id| id.value.first().copied())
                });
            if from_server.is_some() {
                return Ok(from_server);
            }
        }
        let reply = self
            .property(window, self.atoms._NET_WM_PID)?
            .reply()
            .map_err(error)?;
        Ok(reply.value32().and_then(|mut v| v.next()))
    }

    pub fn idle_seconds(&self) -> Result<f64, String> {
        if !self.screensaver {
            return Err("The X server has no idle counter.".into());
        }
        let info = self
            .connection
            .screensaver_query_info(self.root)
            .map_err(error)?
            .reply()
            .map_err(error)?;
        Ok(info.ms_since_user_input as f64 / 1000.)
    }

    /// Root coordinates are physical pixels, the engine's own space.
    pub fn pointer(&self) -> Result<Point, String> {
        let reply = self
            .connection
            .query_pointer(self.root)
            .map_err(error)?
            .reply()
            .map_err(error)?;
        Ok(Point::new(reply.root_x as f64, reply.root_y as f64))
    }
}
