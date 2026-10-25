//! COSMIC, through `zcosmic_toplevel_info_v1` (version 2 or later), which adds
//! geometry and state to the windows `ext_foreign_toplevel_list_v1` lists,
//! and `ext_workspace_v1` for which workspaces are on show. The compositor
//! pushes every change to a thread that sleeps in between, so a scan only
//! copies what it last said.
//!
//! `ext_foreign_toplevel_list_v1` sends each window's title unasked. Its
//! handler matches that event and drops it; nothing here can hold one.
use super::{super::super::wayland_model::Toplevel, WindowSource};
use crate::world::Rect;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use wayland_client::{
    backend::ObjectId,
    event_created_child,
    protocol::{wl_output, wl_registry},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::{
    ext::{
        foreign_toplevel_list::v1::client::{
            ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
            ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
        },
        workspace::v1::client::{
            ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
            ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
            ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
        },
    },
    xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
};

#[allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#[allow(non_upper_case_globals, non_snake_case, unused_imports, missing_docs)]
#[allow(clippy::all)]
mod protocol {
    pub mod workspace {
        use wayland_client;
        use wayland_client::protocol::*;
        pub mod __interfaces {
            use wayland_client::backend as wayland_backend;
            use wayland_client::protocol::__interfaces::*;
            wayland_scanner::generate_interfaces!("./protocols/cosmic-workspace-unstable-v1.xml");
        }
        use self::__interfaces::*;
        wayland_scanner::generate_client_code!("./protocols/cosmic-workspace-unstable-v1.xml");
    }
    pub mod toplevel_info {
        use super::workspace::*;
        use wayland_client;
        use wayland_client::protocol::*;
        use wayland_protocols::ext::foreign_toplevel_list::v1::client::*;
        use wayland_protocols::ext::workspace::v1::client::*;
        pub mod __interfaces {
            use super::super::workspace::__interfaces::*;
            use wayland_client::backend as wayland_backend;
            use wayland_client::protocol::__interfaces::*;
            use wayland_protocols::ext::foreign_toplevel_list::v1::client::__interfaces::*;
            use wayland_protocols::ext::workspace::v1::client::__interfaces::*;
            wayland_scanner::generate_interfaces!(
                "./protocols/cosmic-toplevel-info-unstable-v1.xml"
            );
        }
        use self::__interfaces::*;
        wayland_scanner::generate_client_code!("./protocols/cosmic-toplevel-info-unstable-v1.xml");
    }
}
use protocol::toplevel_info::{
    zcosmic_toplevel_handle_v1::{self, ZcosmicToplevelHandleV1},
    zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
};

/// `zcosmic_toplevel_handle_v1.state` values.
const MINIMIZED: u32 = 1;
const ACTIVATED: u32 = 2;
const FULLSCREEN: u32 = 3;

#[derive(Default)]
struct Window {
    id: u64,
    app_id: Option<String>,
    /// Output-relative position and size, and the output it is relative to.
    geometry: Option<(ObjectId, i32, i32, i32, i32)>,
    states: Vec<u32>,
    workspaces: HashSet<ObjectId>,
    focus_order: u64,
}

#[derive(Default)]
struct Known {
    windows: HashMap<ObjectId, Window>,
    /// Each output's top-left in the compositor's logical layout.
    outputs: HashMap<ObjectId, (i32, i32)>,
    active_workspaces: HashSet<ObjectId>,
    next_id: u64,
    focus_clock: u64,
}

struct State {
    known: Arc<Mutex<Known>>,
    info: Option<ZcosmicToplevelInfoV1>,
    list: Option<ExtForeignToplevelListV1>,
    workspaces: Option<ExtWorkspaceManagerV1>,
    xdg_outputs: Option<ZxdgOutputManagerV1>,
    pending_outputs: Vec<wl_output::WlOutput>,
}

impl State {
    fn known(&self) -> std::sync::MutexGuard<'_, Known> {
        self.known.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct Cosmic {
    known: Arc<Mutex<Known>>,
}

impl Cosmic {
    /// `None` unless the compositor offers toplevel info with geometry, the
    /// toplevel list it builds on, and workspaces.
    pub fn connect() -> Option<Self> {
        let connection = Connection::connect_to_env().ok()?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        connection.display().get_registry(&handle, ());
        let known = Arc::new(Mutex::new(Known::default()));
        let mut state = State {
            known: known.clone(),
            info: None,
            list: None,
            workspaces: None,
            xdg_outputs: None,
            pending_outputs: Vec::new(),
        };
        queue.roundtrip(&mut state).ok()?;
        if state.info.is_none() || state.list.is_none() || state.workspaces.is_none() {
            return None;
        }
        if let Some(manager) = state.xdg_outputs.clone() {
            for output in std::mem::take(&mut state.pending_outputs) {
                manager.get_xdg_output(&output, &handle, output.id());
            }
        }
        queue.roundtrip(&mut state).ok()?;
        std::thread::Builder::new()
            .name("ledgeling-cosmic-windows".into())
            .spawn(move || while queue.blocking_dispatch(&mut state).is_ok() {})
            .ok()?;
        Some(Self { known })
    }
}

impl WindowSource for Cosmic {
    fn name(&self) -> &'static str {
        "cosmic"
    }

    fn toplevels(&self) -> Result<Vec<Toplevel>, String> {
        let known = self
            .known
            .lock()
            .map_err(|_| "COSMIC's windows are unavailable")?;
        Ok(known
            .windows
            .values()
            .map(|window| {
                let rect = window.geometry.as_ref().and_then(|(output, x, y, w, h)| {
                    let (left, top) = known.outputs.get(output).copied()?;
                    Some(Rect::new(
                        f64::from(left + x),
                        f64::from(top + y),
                        f64::from(*w),
                        f64::from(*h),
                    ))
                });
                // A window on no workspace (sticky, or not yet assigned) is
                // taken to be on show.
                let shown = window.workspaces.is_empty()
                    || window
                        .workspaces
                        .iter()
                        .any(|w| known.active_workspaces.contains(w));
                Toplevel {
                    id: window.id,
                    rect,
                    app_id: window.app_id.clone(),
                    pid: None,
                    shown,
                    minimized: window.states.contains(&MINIMIZED),
                    fullscreen: window.states.contains(&FULLSCREEN),
                    focused: window.states.contains(&ACTIVATED),
                    // COSMIC floats windows over its tiles, but does not say
                    // which are floating.
                    floating: false,
                    focus_order: window.focus_order,
                }
            })
            .collect())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            // Version 1 has no geometry.
            "zcosmic_toplevel_info_v1" if version >= 2 => {
                state.info = Some(registry.bind(name, version.min(3), queue, ()));
            }
            "ext_foreign_toplevel_list_v1" => {
                state.list = Some(registry.bind(name, 1, queue, ()));
            }
            "ext_workspace_manager_v1" => {
                state.workspaces = Some(registry.bind(name, 1, queue, ()));
            }
            "zxdg_output_manager_v1" => {
                state.xdg_outputs = Some(registry.bind(name, version.min(3), queue, ()));
            }
            "wl_output" => {
                let output: wl_output::WlOutput = registry.bind(name, version.min(4), queue, ());
                match state.xdg_outputs.clone() {
                    Some(manager) => {
                        manager.get_xdg_output(&output, queue, output.id());
                    }
                    None => state.pending_outputs.push(output),
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            let mut known = state.known();
            known.next_id += 1;
            let id = known.next_id;
            known.windows.insert(
                toplevel.id(),
                Window {
                    id,
                    ..Default::default()
                },
            );
            drop(known);
            if let Some(info) = &state.info {
                info.get_cosmic_toplevel(&toplevel, queue, toplevel.id());
            }
        }
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(window) = state.known().windows.get_mut(&handle.id()) {
                    window.app_id = Some(app_id);
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.known().windows.remove(&handle.id());
                handle.destroy();
            }
            // Dropped unread; see the module comment.
            ext_foreign_toplevel_handle_v1::Event::Title { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<ZcosmicToplevelInfoV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZcosmicToplevelInfoV1,
        _: zcosmic_toplevel_info_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    // Only sent to version 1 clients, but a child event must be declared.
    event_created_child!(State, ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (ZcosmicToplevelHandleV1, ObjectId::null()),
    ]);
}

/// The user data is the id of the `ext_foreign_toplevel_handle_v1` it extends.
impl Dispatch<ZcosmicToplevelHandleV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _: &ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        toplevel: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut known = state.known();
        let known = &mut *known;
        let Some(window) = known.windows.get_mut(toplevel) else {
            return;
        };
        match event {
            zcosmic_toplevel_handle_v1::Event::Geometry {
                output,
                x,
                y,
                width,
                height,
            } => window.geometry = Some((output.id(), x, y, width, height)),
            zcosmic_toplevel_handle_v1::Event::State { state } => {
                let states: Vec<u32> = state
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| u32::from_ne_bytes(*b))
                    .collect();
                if states.contains(&ACTIVATED) && !window.states.contains(&ACTIVATED) {
                    known.focus_clock += 1;
                    window.focus_order = known.focus_clock;
                }
                window.states = states;
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceEnter { workspace } => {
                window.workspaces.insert(workspace.id());
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceLeave { workspace } => {
                window.workspaces.remove(&workspace.id());
            }
            // Deprecated since version 2, and dropped unread like the
            // foreign-toplevel title.
            zcosmic_toplevel_handle_v1::Event::Title { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtWorkspaceManagerV1,
        _: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtWorkspaceGroupHandleV1,
        _: ext_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        workspace: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_handle_v1::Event::State { state: flags } => {
                let active = flags
                    .into_result()
                    .is_ok_and(|f| f.contains(ext_workspace_handle_v1::State::Active));
                let mut known = state.known();
                if active {
                    known.active_workspaces.insert(workspace.id());
                } else {
                    known.active_workspaces.remove(&workspace.id());
                }
            }
            ext_workspace_handle_v1::Event::Removed => {
                state.known().active_workspaces.remove(&workspace.id());
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Without xdg-output, the output's own position is the best there is.
        if let wl_output::Event::Geometry { x, y, .. } = event {
            if state.xdg_outputs.is_none() {
                state.known().outputs.insert(output.id(), (x, y));
            }
        }
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: <ZxdgOutputManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// The user data is the id of the `wl_output` this describes.
impl Dispatch<ZxdgOutputV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        output: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_output_v1::Event::LogicalPosition { x, y } = event {
            state.known().outputs.insert(output.clone(), (x, y));
        }
    }
}
