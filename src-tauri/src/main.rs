#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // An agent's hook runs this binary to report an event, then it exits. It
    // must not start the app.
    if std::env::args().nth(1).as_deref() == Some(ledgeling::terminal::agent::HOOK_FLAG) {
        ledgeling::terminal::agent::record_from_stdin();
        return;
    }
    ledgeling::app::run(tauri::generate_context!());
}
