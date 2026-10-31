//! What a Wayland compositor reports about its windows, reduced to what the
//! engine needs, and the decisions made from it. Wayland has no common way
//! for an app to see other windows, so each supported compositor has its own
//! source (see `platform/linux/sources`); they all describe windows as
//! [`Toplevel`]s and everything from there on is shared and tested here, on
//! any machine.
//!
//! Some of those sources push window titles whether or not anyone wants them.
//! A source drops the title where it arrives, and nothing here has a field to
//! hold one; `tests/privacy.rs` asserts both.
use crate::world::{MonitorInfo, Rect, WindowInfo};

/// Smaller windows are palettes and popups, not perches; the same floor as
/// on X11.
const MINIMUM_WIDTH: f64 = 80.;
const MINIMUM_HEIGHT: f64 = 40.;

/// One window as a compositor describes it, in the compositor's own logical
/// layout coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Toplevel {
    /// Stable for the window's lifetime within one source.
    pub id: u64,
    /// The window's frame, decorations included where the compositor draws
    /// them. `None` while the compositor has not said where it is.
    pub rect: Option<Rect>,
    /// The Wayland app id, or the X class for an XWayland window.
    pub app_id: Option<String>,
    pub pid: Option<u32>,
    /// On a workspace that is currently shown.
    pub shown: bool,
    pub minimized: bool,
    pub fullscreen: bool,
    pub focused: bool,
    /// Drawn above tiled windows, where the compositor tiles.
    pub floating: bool,
    /// Larger is more recently focused. No compositor here reports stacking
    /// order, and on all of them a window comes to the front when focused.
    pub focus_order: u64,
}

/// Visible windows front-to-back, in virtual-desktop physical pixels. Floating
/// windows sit above tiled ones; within each, the most recently focused is in
/// front. Ledgeling's own windows are never perches.
pub fn front_to_back(
    toplevels: &[Toplevel],
    scale: f64,
    own_pid: u32,
    monitor_for: impl Fn(Rect) -> u64,
) -> Vec<WindowInfo> {
    let mut visible: Vec<&Toplevel> = toplevels
        .iter()
        .filter(|t| t.shown && !t.minimized && t.pid != Some(own_pid))
        .filter(|t| !is_own_app(t.app_id.as_deref()))
        .collect();
    visible.sort_by_key(|t| (!t.floating, std::cmp::Reverse(t.focus_order)));
    let mut windows = Vec::new();
    for toplevel in visible {
        let Some(rect) = toplevel.rect else {
            continue;
        };
        let rect = Rect::new(
            rect.x * scale,
            rect.y * scale,
            rect.width * scale,
            rect.height * scale,
        );
        if rect.width < MINIMUM_WIDTH || rect.height < MINIMUM_HEIGHT {
            continue;
        }
        windows.push(WindowInfo {
            id: toplevel.id,
            rect,
            z_index: windows.len(),
            is_minimized: false,
            monitor_id: monitor_for(rect),
            pid: toplevel.pid,
        });
    }
    windows
}

/// Ledgeling's own overlay and bubble are layer surfaces and never listed, but
/// the dashboard is an ordinary window; a source without process ids tells
/// it apart by app id.
fn is_own_app(app_id: Option<&str>) -> bool {
    app_id.is_some_and(|id| {
        let id = id.to_ascii_lowercase();
        id == "ledgeling" || id.ends_with(".ledgeling")
    })
}

/// The focused window, if the compositor says which one it is.
pub fn focused(toplevels: &[Toplevel]) -> Option<&Toplevel> {
    toplevels
        .iter()
        .filter(|t| t.focused && t.shown)
        .max_by_key(|t| t.focus_order)
}

/// A focused fullscreen window on the pet screen: the signature of a game or
/// a film, as on every other platform.
pub fn fullscreen_active(toplevels: &[Toplevel], scale: f64, monitors: &[MonitorInfo]) -> bool {
    focused(toplevels).is_some_and(|t| {
        t.fullscreen
            && t.rect.is_none_or(|r| {
                let centre =
                    Rect::new(r.x * scale, r.y * scale, r.width * scale, r.height * scale).center();
                monitors.iter().any(|m| m.bounds.contains(centre))
            })
    })
}

/// An app id as a name the category map knows, the way it knows executables:
/// reverse-DNS ids keep their last part (`org.mozilla.firefox` → `firefox`,
/// `com.system76.CosmicTerm` → `cosmicterm`), lowercased.
pub fn app_name(app_id: &str) -> Option<String> {
    let id = app_id.trim().trim_end_matches(".desktop");
    let last = if id.matches('.').count() >= 2 {
        id.rsplit('.').next().unwrap_or(id)
    } else {
        id
    };
    Some(last.to_lowercase()).filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN_PID: u32 = 4242;

    fn toplevel(id: u64, x: f64, y: f64, width: f64, height: f64) -> Toplevel {
        Toplevel {
            id,
            rect: Some(Rect::new(x, y, width, height)),
            app_id: Some(format!("app{id}")),
            shown: true,
            focus_order: id,
            ..Default::default()
        }
    }

    #[test]
    fn most_recently_focused_is_in_front() {
        let windows = front_to_back(
            &[
                toplevel(1, 0., 0., 400., 300.),
                toplevel(2, 100., 100., 400., 300.),
            ],
            1.,
            OWN_PID,
            |_| 0,
        );
        assert_eq!(windows.iter().map(|w| w.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(windows[0].z_index, 0);
    }

    #[test]
    fn floating_windows_sit_above_tiled_ones() {
        let mut floating = toplevel(1, 0., 0., 400., 300.);
        floating.floating = true;
        let tiled = toplevel(2, 0., 0., 800., 600.);
        let windows = front_to_back(&[floating, tiled], 1., OWN_PID, |_| 0);
        assert_eq!(windows[0].id, 1);
    }

    #[test]
    fn hidden_minimized_tiny_and_own_windows_are_not_perches() {
        let mut other_workspace = toplevel(1, 0., 0., 400., 300.);
        other_workspace.shown = false;
        let mut minimized = toplevel(2, 0., 0., 400., 300.);
        minimized.minimized = true;
        let tiny = toplevel(3, 0., 0., 30., 30.);
        let mut ours = toplevel(4, 0., 0., 400., 300.);
        ours.pid = Some(OWN_PID);
        let mut dashboard = toplevel(5, 0., 0., 400., 300.);
        dashboard.app_id = Some("ledgeling".into());
        let mut unplaced = toplevel(6, 0., 0., 400., 300.);
        unplaced.rect = None;
        let windows = front_to_back(
            &[other_workspace, minimized, tiny, ours, dashboard, unplaced],
            1.,
            OWN_PID,
            |_| 0,
        );
        assert!(windows.is_empty(), "{windows:?}");
    }

    #[test]
    fn logical_layout_becomes_physical_pixels() {
        let windows = front_to_back(&[toplevel(1, 10., 20., 400., 300.)], 2., OWN_PID, |_| 0);
        assert_eq!(windows[0].rect, Rect::new(20., 40., 800., 600.));
    }

    #[test]
    fn app_ids_become_names_the_category_map_knows() {
        assert_eq!(app_name("org.mozilla.firefox").as_deref(), Some("firefox"));
        assert_eq!(
            app_name("com.system76.CosmicTerm").as_deref(),
            Some("cosmicterm")
        );
        assert_eq!(app_name("org.gnome.Nautilus").as_deref(), Some("nautilus"));
        assert_eq!(app_name("Alacritty").as_deref(), Some("alacritty"));
        assert_eq!(
            app_name("code-url-handler").as_deref(),
            Some("code-url-handler")
        );
        assert_eq!(app_name("  "), None);
    }

    #[test]
    fn focused_fullscreen_window_on_the_pet_screen_suspends() {
        let monitor = MonitorInfo {
            id: 0,
            bounds: Rect::new(0., 0., 1920., 1080.),
            work: Rect::new(0., 0., 1920., 1080.),
            scale: 1.,
            is_primary: true,
        };
        let mut game = toplevel(1, 0., 0., 1920., 1080.);
        game.focused = true;
        assert!(!fullscreen_active(&[game.clone()], 1., &[monitor]));
        game.fullscreen = true;
        assert!(fullscreen_active(&[game.clone()], 1., &[monitor]));
        game.rect = Some(Rect::new(1920., 0., 1920., 1080.));
        assert!(!fullscreen_active(&[game], 1., &[monitor]));
    }
}
