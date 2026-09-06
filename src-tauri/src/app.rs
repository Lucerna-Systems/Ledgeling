use crate::{
    pet::{Activity, Pet},
    platform::{Display, NativePlatform, Platform},
    windows_mgr::{hit_test, NativeSurface, PetSurface},
    world::{self, Rect, Surface, WindowInfo},
};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub type Shared = Arc<Mutex<Runtime>>;
pub struct Runtime {
    pet: Pet,
    display: Display,
    surfaces: Vec<Surface>,
    windows: Vec<WindowInfo>,
    debug: bool,
    scan_ms: f64,
    scan_error: Option<String>,
    render_error: Option<String>,
    renderer: &'static str,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    activity: Activity,
    paused: bool,
    visible: bool,
    debug: bool,
    speed: f64,
    x: f64,
    y: f64,
    clip: &'static str,
    frame_index: u32,
    flip_x: bool,
    surfaces: Vec<Surface>,
    windows: Vec<WindowInfo>,
    screen: Rect,
    scale: f64,
    scan_ms: f64,
    scan_error: Option<String>,
    render_error: Option<String>,
    renderer: &'static str,
}
impl Runtime {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            activity: self.pet.activity,
            paused: self.pet.paused,
            visible: self.pet.visible,
            debug: self.debug,
            speed: self.pet.speed,
            x: self.pet.position.x,
            y: self.pet.position.y,
            clip: self.pet.clip(),
            frame_index: self.pet.frame_index(),
            flip_x: self.pet.flip_x,
            surfaces: self.surfaces.clone(),
            windows: self.windows.clone(),
            screen: self.display.bounds,
            scale: self.display.scale,
            scan_ms: self.scan_ms,
            scan_error: self.scan_error.clone(),
            render_error: self.render_error.clone(),
            renderer: self.renderer,
        }
    }
    fn action(&mut self, kind: &str) -> Result<(), String> {
        match kind {
            "pause" => {
                self.pet.paused = !self.pet.paused;
            }
            "visibility" => {
                self.pet.visible = !self.pet.visible;
            }
            "relocate" | "home" => {
                self.pet.visible = true;
                self.pet.paused = false;
                self.pet
                    .relocate(&self.surfaces, self.display.scale, kind == "home");
            }
            "greet" => self.pet.greet(),
            "grab" => self.pet.grab(),
            "release" => self.pet.release(&self.surfaces, self.display.scale),
            "debug" => self.debug = !self.debug,
            _ => return Err("Unknown Perch action".into()),
        }
        Ok(())
    }
}
#[tauri::command]
async fn get_snapshot(state: State<'_, Shared>) -> Result<Snapshot, String> {
    Ok(state
        .lock()
        .map_err(|_| "Perch state is unavailable")?
        .snapshot())
}
#[tauri::command]
async fn pet_action(
    kind: String,
    window: WebviewWindow,
    state: State<'_, Shared>,
) -> Result<Snapshot, String> {
    if window.label() != "main" && !["grab", "release", "greet"].contains(&kind.as_str()) {
        return Err("This control belongs to the companion window".into());
    }
    let mut runtime = state.lock().map_err(|_| "Perch state is unavailable")?;
    runtime.action(&kind)?;
    Ok(runtime.snapshot())
}
#[tauri::command]
async fn set_speed(
    speed: f64,
    window: WebviewWindow,
    state: State<'_, Shared>,
) -> Result<(), String> {
    if window.label() != "main" || !speed.is_finite() || !(100. ..=400.).contains(&speed) {
        return Err("Flight speed must be between 100 and 400".into());
    }
    state
        .lock()
        .map_err(|_| "Perch state is unavailable")?
        .pet
        .speed = speed;
    Ok(())
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle, window: WebviewWindow) {
    if window.label() == "main" {
        app.exit(0);
    }
}
fn display(window: &WebviewWindow) -> Result<Display, Box<dyn std::error::Error>> {
    let monitor = window
        .primary_monitor()?
        .ok_or("No primary monitor is available")?;
    let pos = monitor.position();
    let size = monitor.size();
    let area = monitor.work_area();
    Ok(Display {
        bounds: Rect {
            x: pos.x as f64,
            y: pos.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        },
        work: Rect {
            x: area.position.x as f64,
            y: area.position.y as f64,
            width: area.size.width as f64,
            height: area.size.height as f64,
        },
        scale: monitor.scale_factor(),
    })
}

pub fn run(context: tauri::Context<tauri::Wry>) {
    tauri::Builder::default()
        .on_menu_event(handle_menu)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            pet_action,
            set_speed,
            quit_app
        ])
        .setup(|app| {
            let main = app
                .get_webview_window("main")
                .ok_or("Companion window is missing")?;
            let d = display(&main)?;
            let overlay = std::env::var("PERCH_RENDERER").as_deref() != Ok("small");
            let start_hidden = std::env::var_os("PERCH_START_HIDDEN").is_some();
            let pet_window =
                WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("index.html?pet".into()))
                    .title("Perch bird")
                    .inner_size(
                        if overlay {
                            d.bounds.width / d.scale
                        } else {
                            128.
                        },
                        if overlay {
                            d.bounds.height / d.scale
                        } else {
                            128.
                        },
                    )
                    .position(d.bounds.x / d.scale, d.bounds.y / d.scale)
                    .decorations(false)
                    .transparent(true)
                    .shadow(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .resizable(false)
                    .focused(false)
                    .focusable(false)
                    .visible(!start_hidden)
                    .build()?;
            pet_window.set_ignore_cursor_events(true)?;
            let mut pet = Pet::new(d.work);
            pet.visible = !start_hidden;
            let shared = Arc::new(Mutex::new(Runtime {
                pet,
                display: d,
                surfaces: world::surfaces(&[], d.work, d.scale),
                windows: vec![],
                debug: false,
                scan_ms: 0.,
                scan_error: None,
                render_error: None,
                renderer: if overlay { "overlay" } else { "small" },
            }));
            app.manage(shared.clone());
            setup_tray(app)?;
            let main_close = main.clone();
            main.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = main_close.hide();
                }
            });
            main.set_focus()?;
            std::thread::spawn(move || run_loop(shared, pet_window, overlay));
            Ok(())
        })
        .run(context)
        .expect("Unable to start Perch");
}
fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem, Submenu},
        tray::TrayIconBuilder,
    };
    let show = MenuItem::with_id(app, "show", "Open Perch", true, Some("CmdOrCtrl+1"))?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause / resume bird",
        true,
        Some("CmdOrCtrl+Shift+P"),
    )?;
    let visibility = MenuItem::with_id(
        app,
        "visibility",
        "Show / hide Pip",
        true,
        Some("CmdOrCtrl+Shift+H"),
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Perch", true, Some("CmdOrCtrl+Q"))?;
    let submenu = Submenu::with_items(app, "Perch", true, &[&show, &pause, &visibility, &quit])?;
    app.set_menu(Menu::with_items(app, &[&submenu])?)?;
    // Keep application-menu accelerators separate from tray menu actions.
    let tray_show = MenuItem::with_id(app, "tray-show", "Open Perch", true, None::<&str>)?;
    let tray_pause =
        MenuItem::with_id(app, "tray-pause", "Pause / resume bird", true, None::<&str>)?;
    let tray_visibility = MenuItem::with_id(
        app,
        "tray-visibility",
        "Show / hide Pip",
        true,
        None::<&str>,
    )?;
    let tray_quit = MenuItem::with_id(app, "tray-quit", "Quit Perch", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&tray_show, &tray_pause, &tray_visibility, &tray_quit],
    )?;
    TrayIconBuilder::with_id("perch")
        .icon(tauri::include_image!("icons/icon.png"))
        .tooltip("Perch · a little company")
        .menu(&menu)
        .build(app)?;
    Ok(())
}

fn handle_menu(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    let kind = event.id.as_ref().trim_start_matches("tray-");
    if std::env::var_os("PERCH_DIAGNOSTICS").is_some() {
        eprintln!("Perch menu action: {kind}");
    }
    match kind {
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        "pause" | "visibility" => {
            let shared = app.state::<Shared>().inner().clone();
            let kind = kind.to_string();
            std::thread::spawn(move || {
                if let Ok(mut s) = shared.lock() {
                    let _ = s.action(&kind);
                }
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn run_loop(shared: Shared, window: WebviewWindow, overlay: bool) {
    let platform = NativePlatform;
    let mut renderer = NativeSurface::new(window.clone(), overlay);
    let mut last_tick = Instant::now();
    let mut last_scan = Instant::now() - Duration::from_secs(1);
    let mut last_display = Instant::now();
    let mut fast_until = Instant::now();
    let mut visible = true;
    let mut interactive = false;
    let mut debug = false;
    let diagnostics = std::env::var_os("PERCH_DIAGNOSTICS").is_some();
    let mut last_diagnostic = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f64();
        last_tick = now;
        let Ok(mut s) = shared.lock() else {
            break;
        };
        if s.pet.visible != visible {
            visible = s.pet.visible;
            let result = if visible {
                window.show()
            } else {
                window.hide()
            };
            if let Err(e) = result {
                s.render_error = Some(e.to_string());
            }
        }
        if debug != s.debug {
            debug = s.debug;
            let _ = window.emit("world:debug", s.snapshot());
        }
        if !visible || s.pet.paused {
            if interactive {
                let _ = window.set_ignore_cursor_events(true);
                interactive = false;
            }
            let _ = renderer.render(&s.pet, s.display);
            drop(s);
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        if last_display.elapsed() > Duration::from_secs(2) {
            if let Ok(d) = display(&window) {
                if d.bounds != s.display.bounds
                    || d.scale != s.display.scale
                    || d.work != s.display.work
                {
                    s.display = d;
                    if overlay {
                        let _ = window.set_size(tauri::LogicalSize::new(
                            d.bounds.width / d.scale,
                            d.bounds.height / d.scale,
                        ));
                        let _ = window.set_position(tauri::LogicalPosition::new(
                            d.bounds.x / d.scale,
                            d.bounds.y / d.scale,
                        ));
                    }
                    s.surfaces = world::surfaces(&s.windows, d.work, d.scale);
                }
            }
            last_display = now;
        }
        let scan_interval = if now < fast_until {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(500)
        };
        if last_scan.elapsed() >= scan_interval {
            let start = Instant::now();
            match platform.enumerate_windows(s.display.scale) {
                Ok(windows) => {
                    if windows != s.windows {
                        fast_until = now + Duration::from_secs(2);
                    }
                    s.windows = windows;
                    s.scan_error = None;
                }
                Err(error) => {
                    s.windows.clear();
                    s.scan_error = Some(error);
                }
            }
            s.surfaces = world::surfaces(&s.windows, s.display.work, s.display.scale);
            s.scan_ms = start.elapsed().as_secs_f64() * 1000.;
            last_scan = now;
            if s.debug {
                let _ = window.emit("world:debug", s.snapshot());
            }
        }
        let cursor = platform.cursor(s.display.scale);
        let Runtime {
            pet,
            display,
            surfaces,
            ..
        } = &mut *s;
        if pet.activity == Activity::Held && !platform.mouse_down() {
            pet.release(surfaces, display.scale);
        }
        pet.tick(dt, surfaces, display.work, display.scale, cursor);
        let hit =
            pet.activity == Activity::Held || cursor.is_some_and(|c| hit_test(pet, *display, c));
        if hit != interactive {
            match window.set_ignore_cursor_events(!hit) {
                Ok(()) => interactive = hit,
                Err(e) => s.render_error = Some(e.to_string()),
            }
        }
        if let Err(e) = renderer.render(&s.pet, s.display) {
            s.render_error = Some(e.to_string());
        }
        if diagnostics && last_diagnostic.elapsed() >= Duration::from_secs(5) {
            eprintln!(
                "{}",
                serde_json::json!({"activity": s.pet.activity, "position": s.pet.position, "windowCount": s.windows.len(), "perchCount": s.surfaces.iter().filter(|surface| surface.owner != 0).count(), "scanMs": s.scan_ms, "error": s.scan_error})
            );
            last_diagnostic = Instant::now();
        }
        let moving = matches!(s.pet.activity, Activity::Flying | Activity::Held);
        drop(s);
        std::thread::sleep(
            Duration::from_secs_f64(if moving { 1. / 60. } else { 1. / 30. })
                .saturating_sub(now.elapsed()),
        );
    }
}
