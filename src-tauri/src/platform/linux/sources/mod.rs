//! Where windows come from on Wayland. There is no common protocol for an app
//! to see other apps' windows, so each compositor that offers one is asked in
//! its own way, and pets use the screen edges everywhere else.
//!
//! Every source here talks only to the compositor of this session: a Wayland
//! protocol on its display, or its IPC socket under `XDG_RUNTIME_DIR`. Titles
//! arrive with some of these answers whether wanted or not, and each source
//! drops them where they arrive; `tests/privacy.rs` asserts that.
mod cosmic;
mod hyprland;
mod niri;
mod sway;

use super::super::wayland_model::Toplevel;

pub trait WindowSource: Send + Sync {
    /// For diagnostics only.
    fn name(&self) -> &'static str;
    /// The compositor's windows as of now.
    fn toplevels(&self) -> Result<Vec<Toplevel>, String>;
}

/// The source for the compositor this session runs, if Ledgeling knows one.
/// The IPC compositors announce themselves through their socket variables;
/// COSMIC is recognised by the protocol it offers.
pub fn detect() -> Option<Box<dyn WindowSource>> {
    if let Some(source) = hyprland::Hyprland::from_env() {
        return Some(Box::new(source));
    }
    if let Some(source) = niri::Niri::from_env() {
        return Some(Box::new(source));
    }
    if let Some(source) = sway::Sway::from_env() {
        return Some(Box::new(source));
    }
    cosmic::Cosmic::connect().map(|source| Box::new(source) as Box<dyn WindowSource>)
}

/// One request and its whole reply over a compositor's IPC socket, with a
/// short timeout: a hung compositor must not stall the update loop.
fn ask_socket(
    path: &std::path::Path,
    request: &[u8],
    read: impl FnOnce(&mut std::os::unix::net::UnixStream) -> std::io::Result<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let timeout = Some(std::time::Duration::from_millis(250));
    let mut stream = std::os::unix::net::UnixStream::connect(path)
        .map_err(|e| format!("The compositor's socket is unavailable ({e})"))?;
    stream.set_read_timeout(timeout).ok();
    stream.set_write_timeout(timeout).ok();
    stream
        .write_all(request)
        .map_err(|e| format!("The compositor did not take the request ({e})"))?;
    read(&mut stream).map_err(|e| format!("The compositor did not answer ({e})"))
}

/// A string id as a stable number, for compositors whose window ids are not
/// numbers.
fn hash_id(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
