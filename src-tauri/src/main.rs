#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod pet;
mod platform;
mod spike;
mod windows_mgr;
mod world;
fn main() {
    let context = tauri::generate_context!();
    if std::env::var_os("PERCH_RENDER_SPIKE").is_some() {
        spike::run(context);
    } else {
        app::run(context);
    }
}
