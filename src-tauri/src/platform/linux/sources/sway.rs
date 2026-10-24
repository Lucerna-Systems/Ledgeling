//! sway, through its i3-compatible IPC socket: one `GET_TREE` per scan. The
//! tree carries each view's name, which the parser skips and never stores.
use super::{super::super::wayland_model::Toplevel, ask_socket, WindowSource};
use crate::world::Rect;
use serde::Deserialize;
use std::{io::Read, path::PathBuf};

const MAGIC: &[u8] = b"i3-ipc";
const GET_TREE: u32 = 4;

pub struct Sway {
    socket: PathBuf,
}

impl Sway {
    pub fn from_env() -> Option<Self> {
        let socket = PathBuf::from(std::env::var_os("SWAYSOCK")?);
        socket.exists().then_some(Self { socket })
    }
}

impl WindowSource for Sway {
    fn name(&self) -> &'static str {
        "sway"
    }

    fn toplevels(&self) -> Result<Vec<Toplevel>, String> {
        let mut request = MAGIC.to_vec();
        request.extend(0u32.to_ne_bytes());
        request.extend(GET_TREE.to_ne_bytes());
        let tree = ask_socket(&self.socket, &request, |stream| {
            let mut header = [0u8; 14];
            stream.read_exact(&mut header)?;
            let length = u32::from_ne_bytes([header[6], header[7], header[8], header[9]]);
            let mut payload = vec![0u8; length as usize];
            stream.read_exact(&mut payload)?;
            Ok(payload)
        })?;
        parse(&tree)
    }
}

#[derive(Deserialize)]
struct Node {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    rect: NodeRect,
    #[serde(default)]
    focused: bool,
    visible: Option<bool>,
    #[serde(default)]
    fullscreen_mode: i64,
    pid: Option<i64>,
    app_id: Option<String>,
    window_properties: Option<WindowProperties>,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    floating_nodes: Vec<Node>,
    /// Child ids, most recently focused first.
    #[serde(default)]
    focus: Vec<i64>,
}

#[derive(Deserialize)]
struct NodeRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// An XWayland view has no app id; its X class stands in.
#[derive(Deserialize)]
struct WindowProperties {
    class: Option<String>,
}

pub(super) fn parse(tree: &[u8]) -> Result<Vec<Toplevel>, String> {
    let root: Node = serde_json::from_slice(tree)
        .map_err(|e| format!("sway's window tree was not understood ({e})"))?;
    let mut out = Vec::new();
    walk(&root, 0, &mut out);
    Ok(out)
}

/// `rank` is how recently this subtree was focused relative to its siblings,
/// accumulated down the tree, so a window's rank orders it against all others.
fn walk(node: &Node, rank: u64, out: &mut Vec<Toplevel>) {
    let is_view = node.nodes.is_empty()
        && node.floating_nodes.is_empty()
        && (node.kind == "con" || node.kind == "floating_con")
        && (node.pid.is_some() || node.app_id.is_some() || node.window_properties.is_some());
    if is_view {
        out.push(Toplevel {
            id: node.id as u64,
            rect: Some(Rect::new(
                node.rect.x,
                node.rect.y,
                node.rect.width,
                node.rect.height,
            )),
            app_id: node.app_id.clone().or_else(|| {
                node.window_properties
                    .as_ref()
                    .and_then(|p| p.class.clone())
            }),
            pid: node.pid.and_then(|pid| u32::try_from(pid).ok()),
            shown: node.visible.unwrap_or(false),
            minimized: false,
            fullscreen: node.fullscreen_mode != 0,
            focused: node.focused,
            floating: node.kind == "floating_con",
            focus_order: rank,
        });
        return;
    }
    // Siblings earlier in `focus` were focused more recently. Each level
    // scales the rank so a deeper difference never outweighs a shallower one.
    for child in node.nodes.iter().chain(&node.floating_nodes) {
        let position = node.focus.iter().position(|id| *id == child.id);
        let recency = position.map_or(0, |p| 64u64.saturating_sub(p as u64));
        walk(child, rank.saturating_mul(65).saturating_add(recency), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Written after the shape of `swaymsg -t get_tree` (sway 1.9), not
    // recorded; names are left in to show they are ignored. The live test
    // runs the real thing under headless sway.
    const TREE: &str = r#"{
      "id":1,"type":"root","name":"root","rect":{"x":0,"y":0,"width":1920,"height":1080},
      "focus":[3],"nodes":[
        {"id":3,"type":"output","name":"DP-1","rect":{"x":0,"y":0,"width":1920,"height":1080},
         "focus":[4,9],"nodes":[
          {"id":4,"type":"workspace","name":"1","rect":{"x":0,"y":0,"width":1920,"height":1080},
           "focus":[7,5,6],"nodes":[
             {"id":5,"type":"con","name":"~/secret","rect":{"x":0,"y":0,"width":960,"height":1080},
              "focused":false,"visible":true,"fullscreen_mode":0,"pid":200,"app_id":"foot","nodes":[]},
             {"id":6,"type":"con","name":"Some page","rect":{"x":960,"y":0,"width":960,"height":1080},
              "focused":false,"visible":true,"fullscreen_mode":0,"pid":300,"app_id":null,
              "window_properties":{"class":"firefox","title":"Some page"},"nodes":[]}
           ],
           "floating_nodes":[
             {"id":7,"type":"floating_con","name":"Calculator","rect":{"x":700,"y":300,"width":400,"height":500},
              "focused":true,"visible":true,"fullscreen_mode":0,"pid":400,"app_id":"org.gnome.Calculator",
              "nodes":[]}
           ]},
          {"id":9,"type":"workspace","name":"2","rect":{"x":0,"y":0,"width":1920,"height":1080},
           "focus":[10],"nodes":[
             {"id":10,"type":"con","name":"film","rect":{"x":0,"y":0,"width":1920,"height":1080},
              "focused":false,"visible":false,"fullscreen_mode":1,"pid":500,"app_id":"mpv","nodes":[]}
           ]}
        ]}
      ]}"#;

    #[test]
    fn reads_views_with_focus_order_and_visibility() {
        let toplevels = parse(TREE.as_bytes()).unwrap();
        let by_id = |id| toplevels.iter().find(|t| t.id == id).unwrap();
        assert_eq!(toplevels.len(), 4);
        assert_eq!(by_id(5).rect, Some(Rect::new(0., 0., 960., 1080.)));
        assert_eq!(by_id(6).app_id.as_deref(), Some("firefox"));
        let calculator = by_id(7);
        assert!(calculator.focused && calculator.floating && calculator.shown);
        // Focus order on workspace 1: the calculator, then foot, then firefox.
        assert!(calculator.focus_order > by_id(5).focus_order);
        assert!(by_id(5).focus_order > by_id(6).focus_order);
        let film = by_id(10);
        assert!(!film.shown && film.fullscreen);
    }

    #[test]
    fn a_garbled_reply_is_an_error_not_a_panic() {
        assert!(parse(b"{").is_err());
    }
}
