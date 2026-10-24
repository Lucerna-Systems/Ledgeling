//! Hyprland, through its request socket: `j/clients` for the windows and
//! `j/monitors` for which workspaces are on show. Fields not named here —
//! the title among them — are skipped by the parser and never stored.
use super::{super::super::wayland_model::Toplevel, ask_socket, hash_id, WindowSource};
use crate::world::Rect;
use serde::Deserialize;
use std::{io::Read, path::PathBuf};

pub struct Hyprland {
    socket: PathBuf,
}

impl Hyprland {
    pub fn from_env() -> Option<Self> {
        let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        // Hyprland moved its sockets from /tmp to the runtime directory.
        let candidates = runtime
            .into_iter()
            .chain([PathBuf::from("/tmp")])
            .map(|base| base.join("hypr").join(&signature).join(".socket.sock"));
        candidates
            .into_iter()
            .find(|path| path.exists())
            .map(|socket| Self { socket })
    }

    fn request(&self, command: &str) -> Result<Vec<u8>, String> {
        ask_socket(&self.socket, command.as_bytes(), |stream| {
            let mut reply = Vec::new();
            stream.read_to_end(&mut reply)?;
            Ok(reply)
        })
    }
}

impl WindowSource for Hyprland {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn toplevels(&self) -> Result<Vec<Toplevel>, String> {
        let clients = self.request("j/clients")?;
        let monitors = self.request("j/monitors")?;
        parse(&clients, &monitors)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    address: String,
    #[serde(default = "yes")]
    mapped: bool,
    #[serde(default)]
    hidden: bool,
    at: [f64; 2],
    size: [f64; 2],
    workspace: WorkspaceRef,
    #[serde(default)]
    floating: bool,
    #[serde(default)]
    class: String,
    #[serde(default)]
    pid: i64,
    /// A bool in older Hyprland, a mode number (0 = none) in newer.
    #[serde(default)]
    fullscreen: serde_json::Value,
    /// 0 for the focused window, counting up through earlier ones.
    #[serde(rename = "focusHistoryID", default = "never")]
    focus_history_id: i64,
}

#[derive(Deserialize)]
struct WorkspaceRef {
    id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Monitor {
    active_workspace: WorkspaceRef,
    special_workspace: Option<WorkspaceRef>,
}

fn yes() -> bool {
    true
}
fn never() -> i64 {
    -1
}

pub(super) fn parse(clients: &[u8], monitors: &[u8]) -> Result<Vec<Toplevel>, String> {
    let clients: Vec<Client> = serde_json::from_slice(clients)
        .map_err(|e| format!("Hyprland's window list was not understood ({e})"))?;
    let monitors: Vec<Monitor> = serde_json::from_slice(monitors)
        .map_err(|e| format!("Hyprland's monitor list was not understood ({e})"))?;
    let shown: Vec<i64> = monitors
        .iter()
        .flat_map(|m| {
            // A special workspace with id 0 means none is open.
            [
                Some(m.active_workspace.id),
                m.special_workspace.as_ref().map(|s| s.id),
            ]
        })
        .flatten()
        .filter(|id| *id != 0)
        .collect();
    Ok(clients
        .into_iter()
        .filter(|c| c.mapped)
        .map(|c| {
            let fullscreen = match &c.fullscreen {
                serde_json::Value::Bool(on) => *on,
                serde_json::Value::Number(mode) => mode.as_i64().unwrap_or(0) != 0,
                _ => false,
            };
            Toplevel {
                id: hash_id(&c.address),
                rect: Some(Rect::new(c.at[0], c.at[1], c.size[0], c.size[1])),
                app_id: Some(c.class).filter(|class| !class.is_empty()),
                pid: u32::try_from(c.pid).ok().filter(|pid| *pid > 0),
                shown: !c.hidden && shown.contains(&c.workspace.id),
                minimized: false,
                fullscreen,
                focused: c.focus_history_id == 0,
                floating: c.floating,
                focus_order: if c.focus_history_id < 0 {
                    0
                } else {
                    u32::MAX as u64 - c.focus_history_id as u64
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Written after the shape of `hyprctl -j clients` / `hyprctl -j monitors`
    // (0.4x), not recorded from a running Hyprland; titles are left in to show
    // they are ignored.
    const CLIENTS: &str = r#"[
      {"address":"0x55d1a0","mapped":true,"hidden":false,"at":[10,40],"size":[940,1030],
       "workspace":{"id":1,"name":"1"},"floating":false,"pseudo":false,"monitor":0,
       "class":"kitty","title":"~/secret-project","initialClass":"kitty",
       "initialTitle":"kitty","pid":1201,"xwayland":false,"pinned":false,
       "fullscreen":0,"fullscreenClient":0,"grouped":[],"tags":[],"swallowing":"0x0",
       "focusHistoryID":0},
      {"address":"0x55d1b0","mapped":true,"hidden":false,"at":[300,200],"size":[800,600],
       "workspace":{"id":1,"name":"1"},"floating":true,"class":"firefox",
       "title":"Inbox (3) - mail","pid":1300,"fullscreen":false,"focusHistoryID":1},
      {"address":"0x55d1c0","mapped":true,"hidden":false,"at":[0,0],"size":[1920,1080],
       "workspace":{"id":2,"name":"2"},"floating":false,"class":"mpv","title":"film.mkv",
       "pid":1400,"fullscreen":2,"focusHistoryID":2}
    ]"#;
    const MONITORS: &str = r#"[
      {"id":0,"name":"DP-1","activeWorkspace":{"id":1,"name":"1"},
       "specialWorkspace":{"id":0,"name":""},"focused":true}
    ]"#;

    #[test]
    fn reads_geometry_focus_and_workspaces() {
        let toplevels = parse(CLIENTS.as_bytes(), MONITORS.as_bytes()).unwrap();
        assert_eq!(toplevels.len(), 3);
        let kitty = &toplevels[0];
        assert_eq!(kitty.rect, Some(Rect::new(10., 40., 940., 1030.)));
        assert!(kitty.focused && kitty.shown && !kitty.floating);
        assert_eq!(kitty.app_id.as_deref(), Some("kitty"));
        assert_eq!(kitty.pid, Some(1201));
        assert!(toplevels[1].floating && !toplevels[1].focused);
        assert!(kitty.focus_order > toplevels[1].focus_order);
        // On workspace 2, which is not shown; its fullscreen mode is a number.
        assert!(!toplevels[2].shown && toplevels[2].fullscreen);
    }

    #[test]
    fn a_garbled_reply_is_an_error_not_a_panic() {
        assert!(parse(b"not json", MONITORS.as_bytes()).is_err());
    }
}
