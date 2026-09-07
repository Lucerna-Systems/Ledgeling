#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    perch::app::run(tauri::generate_context!());
}
