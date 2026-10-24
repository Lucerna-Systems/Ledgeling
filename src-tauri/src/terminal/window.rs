//! The terminal window: one ordinary window, shared by every pet. It follows
//! whichever pet asked last, and keeps the others' terminals running behind it.
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "terminal";

pub fn open(app: &tauri::AppHandle, title: &str) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_title(title);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("terminal.html".into()))
        .title(title)
        .inner_size(920., 600.)
        .min_inner_size(420., 280.)
        .center()
        .resizable(true)
        .visible(true)
        .focused(true)
        .build()
        .map_err(|e| e.to_string())
}
