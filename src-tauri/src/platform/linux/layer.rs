//! The overlay as a native Wayland layer surface, on compositors that offer
//! `zwlr_layer_shell_v1`: COSMIC, KDE Plasma, sway, Hyprland, niri, Wayfire,
//! labwc and most others apart from GNOME. A layer surface sits above every
//! window and is never placed by the window manager, which is what an XWayland
//! overlay can only approximate, and the compositor delivers the pointer to
//! it directly, so pets can be picked up over native Wayland windows.
//!
//! Wayland never tells a client where the pointer is outside its own surface.
//! Instead of polling a global cursor, the overlay's input region is the pets'
//! sprite boxes: the compositor routes the pointer to the overlay only there,
//! and everywhere else clicks go to whatever is underneath.
use crate::world::Rect;
use gtk::prelude::*;
use gtk_layer_shell::LayerShell;
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

static SUPPORTED: OnceLock<bool> = OnceLock::new();

/// Decided once, before GTK starts, from the compositor's globals.
pub fn supported() -> bool {
    *SUPPORTED.get_or_init(probe)
}

fn probe() -> bool {
    use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};

    struct Globals(bool);
    impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
        fn event(
            state: &mut Self,
            _: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { interface, .. } = event {
                state.0 |= interface == "zwlr_layer_shell_v1";
            }
        }
    }

    let Ok(connection) = Connection::connect_to_env() else {
        return false;
    };
    let mut queue = connection.new_event_queue();
    connection.display().get_registry(&queue.handle(), ());
    let mut globals = Globals(false);
    queue.roundtrip(&mut globals).is_ok() && globals.0
}

/// Can the overlay be a layer surface in this process? Needs GTK on the
/// Wayland display, which `GDK_BACKEND=x11` would rule out. Main thread only.
pub fn usable() -> bool {
    supported() && gtk_layer_shell::is_supported()
}

/// Turns the overlay's GTK window into a full-screen layer surface on the pet
/// screen. It must not be mapped yet: a mapped window already has its
/// `xdg_toplevel` role, and a surface's role cannot change.
pub fn attach(window: &tauri::WebviewWindow, monitor: Rect) -> Result<(), String> {
    let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
    if gtk_window.is_visible() {
        gtk_window.hide();
    }
    if gtk_window.is_realized() {
        gtk_window.unrealize();
    }
    gtk_window.init_layer_shell();
    gtk_window.set_namespace("ledgeling");
    gtk_window.set_layer(gtk_layer_shell::Layer::Overlay);
    gtk_window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::None);
    for edge in [
        gtk_layer_shell::Edge::Left,
        gtk_layer_shell::Edge::Right,
        gtk_layer_shell::Edge::Top,
        gtk_layer_shell::Edge::Bottom,
    ] {
        gtk_window.set_anchor(edge, true);
    }
    // -1: cover panels and docks too rather than being pushed aside by them,
    // so the overlay's origin is the screen's origin.
    gtk_window.set_exclusive_zone(-1);
    set_monitor(window, monitor);
    gtk_window.set_app_paintable(true);
    // Nothing catches the pointer until the first scene says where pets are.
    *region() = Some(Vec::new());
    apply_region(&gtk_window, &[]);
    Ok(())
}

/// The pet screen's top-left in logical pixels, as GDK placed it. A layer
/// surface is positioned by margins from its output's edges, so the bubble's
/// desktop coordinates are taken relative to this.
static SCREEN_ORIGIN: Mutex<(f64, f64)> = Mutex::new((0., 0.));

/// Puts the layer surface on the output whose geometry matches the pet
/// screen. GDK reports logical geometry, so the screen's physical bounds are
/// matched after dividing by that monitor's own scale.
pub fn set_monitor(window: &tauri::WebviewWindow, monitor: Rect) {
    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    if let Some(found) = gdk_monitor(monitor) {
        let g = found.geometry();
        if let Ok(mut origin) = SCREEN_ORIGIN.lock() {
            *origin = (g.x() as f64, g.y() as f64);
        }
        gtk_window.set_monitor(&found);
    }
}

fn gdk_monitor(monitor: Rect) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    let center = monitor.center();
    (0..display.n_monitors())
        .filter_map(|i| display.monitor(i))
        .find(|m| {
            let g = m.geometry();
            let scale = m.scale_factor().max(1) as f64;
            let (x, y) = (center.x / scale, center.y / scale);
            x >= g.x() as f64
                && x < (g.x() + g.width()) as f64
                && y >= g.y() as f64
                && y < (g.y() + g.height()) as f64
        })
}

/// Makes the pet bubble a layer surface above everything, placed by margins
/// on the pet screen. It takes the keyboard when clicked, for typing a task,
/// and gives it back when something else is. Before its first showing only.
pub fn attach_bubble(window: &tauri::WebviewWindow) -> Result<(), String> {
    let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
    if gtk_window.is_realized() {
        gtk_window.unrealize();
    }
    gtk_window.init_layer_shell();
    gtk_window.set_namespace("ledgeling-bubble");
    gtk_window.set_layer(gtk_layer_shell::Layer::Overlay);
    gtk_window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::OnDemand);
    gtk_window.set_anchor(gtk_layer_shell::Edge::Left, true);
    gtk_window.set_anchor(gtk_layer_shell::Edge::Top, true);
    gtk_window.set_exclusive_zone(0);
    gtk_window.set_app_paintable(true);
    if let Some(overlay) = window.app_handle().get_webview_window("pet-0") {
        if let Ok(overlay) = overlay.gtk_window() {
            if let Some(monitor) = overlay.monitor() {
                gtk_window.set_monitor(&monitor);
            }
        }
    }
    Ok(())
}

/// Moves and sizes the bubble, given in logical desktop coordinates.
pub fn place_bubble(window: &tauri::WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
    let window = window.clone();
    let _ = window.clone().run_on_main_thread(move || {
        let Ok(gtk_window) = window.gtk_window() else {
            return;
        };
        let (left, top) = SCREEN_ORIGIN.lock().map(|o| *o).unwrap_or((0., 0.));
        gtk_window.set_layer_shell_margin(gtk_layer_shell::Edge::Left, (x - left).round() as i32);
        gtk_window.set_layer_shell_margin(gtk_layer_shell::Edge::Top, (y - top).round() as i32);
        gtk_window.set_size_request(width.round() as i32, height.round() as i32);
        gtk_window.resize(width.round() as i32, height.round() as i32);
    });
}

/// The last input region applied, in the overlay's logical pixels. `None`
/// until the overlay is a layer surface, which is how callers know whether
/// regions apply at all.
fn region() -> std::sync::MutexGuard<'static, Option<Vec<[i32; 4]>>> {
    static REGION: Mutex<Option<Vec<[i32; 4]>>> = Mutex::new(None);
    REGION.lock().unwrap_or_else(|e| e.into_inner())
}

/// Is the overlay a layer surface whose input follows the pets?
pub fn active() -> bool {
    region().is_some()
}

/// Lets the pointer reach the overlay only inside `boxes`, given in the
/// overlay's logical pixels. Applying a region costs a trip to the main
/// thread, so an unchanged one is skipped.
pub fn set_input_boxes(window: &tauri::WebviewWindow, boxes: Vec<[i32; 4]>) {
    {
        let mut current = region();
        if current.as_ref().is_none_or(|current| *current == boxes) {
            return;
        }
        *current = Some(boxes.clone());
    }
    let window = window.clone();
    let _ = window.clone().run_on_main_thread(move || {
        if let Ok(gtk_window) = window.gtk_window() {
            apply_region(&gtk_window, &boxes);
        }
    });
}

fn apply_region(gtk_window: &gtk::ApplicationWindow, boxes: &[[i32; 4]]) {
    let region = gtk::cairo::Region::create();
    for [x, y, width, height] in boxes {
        let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(*x, *y, *width, *height));
    }
    gtk_window.input_shape_combine_region(Some(&region));
}
