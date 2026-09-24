//! The privacy constraints are product-defining, so they are asserted rather
//! than trusted: no window title is ever requested from the OS, and no code
//! passive companion path can reach the network. Explicit task adapters are opt-in.
use std::path::{Path, PathBuf};

fn source_files() -> Vec<PathBuf> {
    fn walk(directory: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "m") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("native"),
        &mut out,
    );
    assert!(out.len() > 10, "the source tree should have been found");
    out
}

/// Source with comments stripped: these tests are about calls that exist, not
/// about prose explaining which calls deliberately do not.
fn read_all() -> Vec<(PathBuf, String)> {
    source_files()
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let code = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            Some((path, code))
        })
        .collect()
}

#[test]
fn no_window_title_api_is_referenced_anywhere_in_the_build() {
    // Window titles carry email subjects, document names, customer names and
    // URLs. Foreground detection stops at the executable name.
    const TITLE_APIS: &[&str] = &[
        "GetWindowTextW",
        "GetWindowTextA",
        "GetWindowText",
        "InternalGetWindowText",
        "kCGWindowName",
        "kCGWindowOwnerName",
        "AXTitle",
        "NSWindow.title",
        // Also covers _NET_WM_NAME; the icon and visible names are titles too.
        "WM_NAME",
        "_NET_WM_VISIBLE_NAME",
        "ICON_NAME",
        "XFetchName",
        "XGetWMName",
        // A process's arguments and environment carry file names and paths;
        // Linux foreground detection stops at /proc/<pid>/exe and comm.
        "/cmdline",
        "/environ",
    ];
    for (path, text) in read_all() {
        if path.ends_with("privacy.rs") {
            continue;
        }
        for api in TITLE_APIS {
            assert!(
                !text.contains(api),
                "{} references the window-title API `{api}`",
                path.display()
            );
        }
    }
}

#[test]
fn no_screen_capture_keystroke_or_clipboard_api_is_referenced() {
    const FORBIDDEN: &[&str] = &[
        "CGWindowListCreateImage",
        "CGDisplayCreateImage",
        "BitBlt",
        "PrintWindow",
        "SetWindowsHookEx",
        "GetAsyncKeyState",
        "GetKeyboardState",
        "NSPasteboard",
        "GetClipboardData",
        "CGEventTapCreate",
        "XGetImage",
        "get_image",
        "XShmGetImage",
        "XQueryKeymap",
        "query_keymap",
        "XGrabKey",
        "grab_key",
        "XRecord",
        "record_enable_context",
        "xinput",
    ];
    for (path, text) in read_all() {
        if path.ends_with("privacy.rs") {
            continue;
        }
        for api in FORBIDDEN {
            assert!(
                !text.contains(api),
                "{} references `{api}`, which Perch must never call",
                path.display()
            );
        }
    }
}

#[test]
fn networking_is_confined_to_explicit_task_adapters() {
    const NETWORK: &[&str] = &[
        "TcpStream",
        "UdpSocket",
        "TcpListener",
        "reqwest",
        "hyper::",
        "ureq",
        "curl",
        "std::net",
        "WebSocket",
    ];
    for (path, text) in read_all() {
        if path.ends_with("privacy.rs")
            || [
                "tasks/providers.rs",
                "tasks/search.rs",
                "tasks/commands.rs",
                "tasks/apple.rs",
            ]
            .iter()
            .any(|p| path.ends_with(p))
        {
            continue;
        }
        for api in NETWORK {
            let hits: Vec<&str> = text
                .lines()
                .filter(|line| line.contains(api))
                // Documentation URLs in comments are not network calls.
                .filter(|line| {
                    !line.trim_start().starts_with("//") && !line.trim_start().starts_with("///")
                })
                .collect();
            assert!(
                hits.is_empty(),
                "{} may reach the network via `{api}`: {hits:?}",
                path.display()
            );
        }
    }
}

#[test]
fn telemetry_and_frontend_network_plugins_are_absent() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for crate_name in [
        "sentry",
        "tokio-tungstenite",
        "opentelemetry",
        "tauri-plugin-http",
        "tauri-plugin-updater",
        "tauri-plugin-shell",
    ] {
        assert!(
            !manifest.contains(crate_name),
            "`{crate_name}` must not bypass the task boundary"
        );
    }
}

#[test]
fn the_tauri_capabilities_grant_nothing_beyond_local_events() {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json"),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("capabilities are JSON");
    let granted: Vec<String> = parsed["permissions"]
        .as_array()
        .expect("a permissions list")
        .iter()
        .map(|p| p.as_str().unwrap_or_default().to_string())
        .collect();
    for permission in &granted {
        assert!(
            permission.starts_with("core:event:") || permission.starts_with("core:window:"),
            "`{permission}` is more than local events and window state"
        );
    }
    for forbidden in ["shell", "fs:", "http", "updater", "process:"] {
        assert!(
            !granted.iter().any(|p| p.contains(forbidden)),
            "capabilities grant `{forbidden}`"
        );
    }
}

#[test]
fn a_pack_cannot_describe_anything_executable() {
    // The condition grammar is the only thing a pack can express beyond data,
    // and it has no call, no assignment, and a closed signal namespace.
    use perch::packs::Condition;
    for attempt in [
        "system('rm -rf /')",
        "idle_seconds > 5; drop table",
        "require('fs')",
        "eval(1)",
        "__proto__ == 1",
        "energy > 0.5 && globalThis",
        "fetch('http://example.com')",
    ] {
        assert!(
            Condition::parse(attempt).is_err(),
            "`{attempt}` must not parse"
        );
    }
    assert!(Condition::parse("idle_seconds > 300").is_ok());
}

#[test]
fn the_foreground_process_name_is_mapped_and_never_retained() {
    use perch::awareness::{AppCategory, CategoryMap};
    let map = CategoryMap::default();
    // The category is all that survives the mapping; the name is not stored on
    // the returned value in any form.
    let category = map.categorize("Slack");
    assert_eq!(category, AppCategory::Comms);
    assert_eq!(std::mem::size_of_val(&category), 1);
}
