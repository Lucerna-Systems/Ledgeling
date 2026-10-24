//! niri, through its IPC socket: windows, workspaces and outputs, one JSON
//! line each. niri scrolls windows past the screen edge, so only tiles it
//! places in the current workspace view are listed. Window titles in the
//! reply are skipped by the parser and never stored.
use super::{super::super::wayland_model::Toplevel, ask_socket, WindowSource};
use crate::world::Rect;
use serde::Deserialize;
use std::{collections::HashMap, io::BufRead, path::PathBuf};

pub struct Niri {
    socket: PathBuf,
}

impl Niri {
    pub fn from_env() -> Option<Self> {
        let socket = PathBuf::from(std::env::var_os("NIRI_SOCKET")?);
        socket.exists().then_some(Self { socket })
    }

    fn request(&self, request: &str) -> Result<Vec<u8>, String> {
        ask_socket(
            &self.socket,
            format!("\"{request}\"\n").as_bytes(),
            |stream| {
                let mut line = Vec::new();
                std::io::BufReader::new(stream).read_until(b'\n', &mut line)?;
                Ok(line)
            },
        )
    }
}

impl WindowSource for Niri {
    fn name(&self) -> &'static str {
        "niri"
    }

    fn toplevels(&self) -> Result<Vec<Toplevel>, String> {
        let windows = self.request("Windows")?;
        let workspaces = self.request("Workspaces")?;
        let outputs = self.request("Outputs")?;
        parse(&windows, &workspaces, &outputs)
    }
}

#[derive(Deserialize)]
enum Reply<T> {
    Ok(T),
    Err(String),
}

#[derive(Deserialize)]
enum Response {
    Windows(Vec<Window>),
    Workspaces(Vec<Workspace>),
    Outputs(HashMap<String, Output>),
}

#[derive(Deserialize)]
struct Window {
    id: u64,
    app_id: Option<String>,
    pid: Option<i32>,
    workspace_id: Option<u64>,
    #[serde(default)]
    is_focused: bool,
    #[serde(default)]
    is_floating: bool,
    layout: Option<Layout>,
    focus_timestamp: Option<Timestamp>,
}

#[derive(Deserialize)]
struct Layout {
    tile_size: (f64, f64),
    /// Logical pixels from the workspace view's top-left; `None` when the
    /// tile is not in the view.
    tile_pos_in_workspace_view: Option<(f64, f64)>,
}

#[derive(Deserialize)]
struct Timestamp {
    secs: u64,
    nanos: u32,
}

#[derive(Deserialize)]
struct Workspace {
    id: u64,
    output: Option<String>,
    #[serde(default)]
    is_active: bool,
}

#[derive(Deserialize)]
struct Output {
    logical: Option<Logical>,
}

#[derive(Deserialize)]
struct Logical {
    x: f64,
    y: f64,
}

fn response(reply: &[u8], what: &str) -> Result<Response, String> {
    match serde_json::from_slice::<Reply<Response>>(reply) {
        Ok(Reply::Ok(response)) => Ok(response),
        Ok(Reply::Err(error)) => Err(format!("niri refused the {what} request ({error})")),
        Err(e) => Err(format!("niri's {what} reply was not understood ({e})")),
    }
}

pub(super) fn parse(
    windows: &[u8],
    workspaces: &[u8],
    outputs: &[u8],
) -> Result<Vec<Toplevel>, String> {
    let Response::Windows(windows) = response(windows, "window")? else {
        return Err("niri answered the window request with something else".into());
    };
    let Response::Workspaces(workspaces) = response(workspaces, "workspace")? else {
        return Err("niri answered the workspace request with something else".into());
    };
    let Response::Outputs(outputs) = response(outputs, "output")? else {
        return Err("niri answered the output request with something else".into());
    };
    // Where each shown workspace's view starts on the desktop.
    let origins: HashMap<u64, (f64, f64)> = workspaces
        .iter()
        .filter(|w| w.is_active)
        .filter_map(|w| {
            let logical = outputs.get(w.output.as_ref()?)?.logical.as_ref()?;
            Some((w.id, (logical.x, logical.y)))
        })
        .collect();
    Ok(windows
        .into_iter()
        .map(|w| {
            let origin = w.workspace_id.and_then(|id| origins.get(&id)).copied();
            let rect = w.layout.as_ref().zip(origin).and_then(|(layout, (x, y))| {
                let (left, top) = layout.tile_pos_in_workspace_view?;
                let (width, height) = layout.tile_size;
                Some(Rect::new(x + left, y + top, width, height))
            });
            Toplevel {
                id: w.id,
                shown: rect.is_some(),
                rect,
                app_id: w.app_id,
                pid: w.pid.and_then(|pid| u32::try_from(pid).ok()),
                minimized: false,
                // niri has no fullscreen flag in its window list; a
                // fullscreen window fills the view and is caught there.
                fullscreen: false,
                focused: w.is_focused,
                floating: w.is_floating,
                focus_order: w
                    .focus_timestamp
                    .map_or(0, |t| t.secs * 1_000 + u64::from(t.nanos / 1_000_000)),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Written after niri-ipc's `Window`, `Workspace` and `Output` types, not
    // recorded from a running niri; titles are left in to show they are
    // ignored.
    const WINDOWS: &str = r#"{"Ok":{"Windows":[
      {"id":11,"title":"~/secret","app_id":"Alacritty","pid":800,"workspace_id":1,
       "is_focused":true,"is_floating":false,"is_urgent":false,
       "layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[960.0,1048.0],
                 "window_size":[956,1044],"tile_pos_in_workspace_view":[16.0,16.0],
                 "window_offset_in_tile":[2.0,2.0]},
       "focus_timestamp":{"secs":120,"nanos":500000000}},
      {"id":12,"title":"Off to the right","app_id":"firefox","pid":900,"workspace_id":1,
       "is_focused":false,"is_floating":false,"is_urgent":false,
       "layout":{"pos_in_scrolling_layout":[2,1],"tile_size":[960.0,1048.0],
                 "window_size":[956,1044],"tile_pos_in_workspace_view":null,
                 "window_offset_in_tile":[2.0,2.0]},
       "focus_timestamp":{"secs":100,"nanos":0}},
      {"id":13,"title":"Elsewhere","app_id":"mpv","pid":901,"workspace_id":2,
       "is_focused":false,"is_floating":true,"is_urgent":false,
       "layout":{"pos_in_scrolling_layout":null,"tile_size":[640.0,360.0],
                 "window_size":[640,360],"tile_pos_in_workspace_view":[10.0,10.0],
                 "window_offset_in_tile":[0.0,0.0]},
       "focus_timestamp":null}
    ]}}"#;
    const WORKSPACES: &str = r#"{"Ok":{"Workspaces":[
      {"id":1,"idx":1,"name":null,"output":"DP-2","is_urgent":false,"is_active":true,
       "is_focused":true,"active_window_id":11},
      {"id":2,"idx":2,"name":null,"output":"DP-2","is_urgent":false,"is_active":false,
       "is_focused":false,"active_window_id":13}
    ]}}"#;
    const OUTPUTS: &str = r#"{"Ok":{"Outputs":{"DP-2":{"name":"DP-2","make":"Dell",
      "model":"U2720Q","serial":null,"physical_size":[600,340],"modes":[],
      "current_mode":0,"is_custom_mode":false,"vrr_supported":false,"vrr_enabled":false,
      "logical":{"x":1920,"y":0,"width":1920,"height":1080,"scale":1.0,"transform":"Normal"}}}}}"#;

    #[test]
    fn places_tiles_in_the_view_and_skips_the_rest() {
        let toplevels = parse(
            WINDOWS.as_bytes(),
            WORKSPACES.as_bytes(),
            OUTPUTS.as_bytes(),
        )
        .unwrap();
        let terminal = &toplevels[0];
        assert_eq!(terminal.rect, Some(Rect::new(1936., 16., 960., 1048.)));
        assert!(terminal.shown && terminal.focused);
        assert!(terminal.focus_order > toplevels[1].focus_order);
        // Scrolled out of view, and on a workspace not on show.
        assert!(!toplevels[1].shown);
        assert!(!toplevels[2].shown && toplevels[2].floating);
    }

    #[test]
    fn an_error_reply_is_reported() {
        let error = parse(
            br#"{"Err":"nope"}"#,
            WORKSPACES.as_bytes(),
            OUTPUTS.as_bytes(),
        );
        assert!(error.unwrap_err().contains("nope"));
    }
}
