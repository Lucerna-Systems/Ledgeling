use super::Shared;
use tauri::Manager;

pub fn setup_tray(app: &tauri::App, developer: bool) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::TrayIconBuilder;
    set_app_menu(app.handle(), developer)?;
    // macOS tints a template image to match the menu bar, like Apple's own
    // icons; elsewhere a black silhouette would vanish on a dark taskbar.
    #[cfg(target_os = "macos")]
    let tray = TrayIconBuilder::with_id("ledgeling")
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let tray = TrayIconBuilder::with_id("ledgeling").icon(tauri::include_image!("icons/icon.png"));
    tray.tooltip("Ledgeling · a little company")
        .menu(&tray_menu(app.handle(), developer)?)
        .build(app)?;
    Ok(())
}

/// Coding mode appears in the menus only while Developer tools are on.
pub fn set_menus(app: &tauri::AppHandle, developer: bool) -> tauri::Result<()> {
    set_app_menu(app, developer)?;
    if let Some(tray) = app.tray_by_id("ledgeling") {
        tray.set_menu(Some(tray_menu(app, developer)?))?;
    }
    Ok(())
}

fn set_app_menu(app: &tauri::AppHandle, developer: bool) -> tauri::Result<()> {
    use tauri::menu::{IsMenuItem, Menu, MenuItem, Submenu};
    let show = MenuItem::with_id(app, "show", "Open Ledgeling", true, Some("CmdOrCtrl+1"))?;
    let code = MenuItem::with_id(app, "code", "Coding mode", true, Some("CmdOrCtrl+Shift+E"))?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause / resume",
        true,
        Some("CmdOrCtrl+Shift+P"),
    )?;
    let visibility = MenuItem::with_id(
        app,
        "visibility",
        "Show / hide pets",
        true,
        Some("CmdOrCtrl+Shift+H"),
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ledgeling", true, Some("CmdOrCtrl+Q"))?;
    let mut items: Vec<&dyn IsMenuItem<tauri::Wry>> = vec![&show];
    if developer {
        items.push(&code);
    }
    items.extend([&pause as &dyn IsMenuItem<tauri::Wry>, &visibility, &quit]);
    let submenu = Submenu::with_items(app, "Ledgeling", true, &items)?;
    app.set_menu(Menu::with_items(app, &[&submenu])?)?;
    Ok(())
}

/// Tray items are separate so their accelerators do not double-fire.
fn tray_menu(
    app: &tauri::AppHandle,
    developer: bool,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{IsMenuItem, Menu, MenuItem};
    let show = MenuItem::with_id(app, "tray-show", "Open Ledgeling", true, None::<&str>)?;
    let code = MenuItem::with_id(app, "tray-code", "Coding mode", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "tray-pause", "Pause / resume", true, None::<&str>)?;
    let visibility = MenuItem::with_id(
        app,
        "tray-visibility",
        "Show / hide pets",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit Ledgeling", true, None::<&str>)?;
    let mut items: Vec<&dyn IsMenuItem<tauri::Wry>> = vec![&show];
    if developer {
        items.push(&code);
    }
    items.extend([&pause as &dyn IsMenuItem<tauri::Wry>, &visibility, &quit]);
    Menu::with_items(app, &items)
}

pub fn handle_menu(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    let kind = event.id.as_ref().trim_start_matches("tray-").to_string();
    match kind.as_str() {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        #[cfg(feature = "coding")]
        "code" => {
            // Menu events arrive on the UI thread, which opening the window
            // must not block on.
            let handle = app.clone();
            std::thread::spawn(move || {
                if let Err(error) = crate::coding::commands::open_window(&handle, None) {
                    eprintln!("Ledgeling could not open coding mode: {error}");
                }
            });
        }
        "pause" | "visibility" => {
            let shared = app.state::<Shared>().inner().clone();
            std::thread::spawn(move || {
                if let Ok(mut app) = shared.lock() {
                    match kind.as_str() {
                        "pause" => app.engine.settings.paused = !app.engine.settings.paused,
                        _ => app.engine.settings.hidden = !app.engine.settings.hidden,
                    }
                    app.dirty = true;
                }
            });
        }
        "quit" => {
            if let Ok(mut app_state) = app.state::<Shared>().inner().lock() {
                let _ = app_state.persist();
            }
            app.exit(0);
        }
        _ => {}
    }
}
