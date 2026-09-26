#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ledgeling::app::run(tauri::generate_context!());
}
