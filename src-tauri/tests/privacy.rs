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
                "{} references `{api}`, which Ledgeling must never call",
                path.display()
            );
        }
    }
}

#[test]
fn wayland_window_sources_drop_titles_where_they_arrive() {
    // Some compositors push every window's title to anyone listing windows.
    // The only thing a source may do with one is match the event and drop
    // it: no field, binding or parsed key may carry a title onward. Test
    // fixtures below `#[cfg(test)]` hold recorded replies, titles included.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/platform");
    let mut checked = 0;
    for (path, text) in read_all() {
        let in_scope =
            path.starts_with(root.join("linux/sources")) || path.ends_with("wayland_model.rs");
        if !in_scope {
            continue;
        }
        checked += 1;
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        for line in code.lines() {
            if line.to_ascii_lowercase().contains("title") {
                assert!(
                    line.contains("Title { .. } => {}"),
                    "{} handles a window title: `{}`",
                    path.display(),
                    line.trim()
                );
            }
        }
    }
    assert!(
        checked >= 5,
        "the Wayland window sources should have been found"
    );
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
            // The gate for every optional online feature; see the next tests.
            || path.components().any(|c| c.as_os_str() == "net")
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

/// The `net` folder is exempt above only so it can hold the gate. Inside it,
/// the HTTP client lives in one file, and that file's `send` checks consent
/// before it does anything else.
#[test]
fn the_http_client_lives_in_one_file_and_checks_consent_first() {
    let net = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/net");
    let mut with_client = Vec::new();
    for entry in std::fs::read_dir(&net).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let uses = text.lines().any(|l| {
            !l.trim_start().starts_with("//") && (l.contains("reqwest") || l.contains("hyper::"))
        });
        if uses {
            with_client.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    assert_eq!(
        with_client,
        ["client.rs"],
        "only net/client.rs may hold an HTTP client"
    );
    let client = std::fs::read_to_string(net.join("client.rs")).unwrap();
    let send = client
        .split("pub fn send(")
        .nth(1)
        .expect("net/client.rs must define `send`");
    let first_statement = send.lines().nth(1).unwrap_or_default().trim();
    assert_eq!(
        first_statement, "self.gate(scope)?;",
        "`send` must check consent before anything else"
    );
    // The consent check is the only thing that can make a scope reachable.
    assert_eq!(
        client
            .matches("reqwest::blocking::Client::builder()")
            .count(),
        1,
        "a second client would be a way around `send`"
    );
}

#[test]
fn every_online_scope_starts_off_and_no_other_code_builds_a_request() {
    // Settings must not carry a network switch the frontend could flip by
    // sending a whole Settings object: consent lives in `net`, written only by
    // `net_set`.
    let settings =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/settings.rs"))
            .unwrap();
    assert!(
        !settings.to_lowercase().contains("online"),
        "consent does not belong in Settings"
    );
    let consent =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/net/consent.rs"))
            .unwrap();
    // Compare words, not bytes: Windows checks files out with CRLF endings.
    let words = consent.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        words.contains("#[default] Off"),
        "the default mode must be Off"
    );
}

/// Whatever is read at compile time is baked into the binary and can be read
/// back out of it, so a secret must never be. Only the public server host lists
/// may be (Cargo's own `CARGO_*` values are fine); anything else needs a
/// deliberate change here, and a good reason.
#[test]
fn only_public_settings_are_compiled_into_the_app() {
    const PUBLIC: &[&str] = &["LEDGELING_MARKETPLACE_HOSTS", "LEDGELING_ANALYTICS_HOSTS"];
    const MARKER: &str = "env!(\"";
    let mut checked = 0;
    for (path, text) in read_all() {
        if path.ends_with("privacy.rs") {
            continue;
        }
        for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            let mut rest = line;
            while let Some(at) = rest.find(MARKER) {
                let name: String = rest[at + MARKER.len()..]
                    .chars()
                    .take_while(|c| *c != '"')
                    .collect();
                // `option_env!` contains `env!`, so the prefix decides which it is.
                let optional = rest[..at].ends_with("option_");
                rest = &rest[at + MARKER.len()..];
                if !optional && name.starts_with("CARGO_") {
                    continue;
                }
                checked += 1;
                assert!(
                    PUBLIC.contains(&name.as_str()),
                    "{} compiles `{name}` into the binary; secrets must never be baked in",
                    path.display()
                );
            }
        }
    }
    assert!(checked >= 2, "the host settings should have been found");
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
    // Every capability file, so a new window cannot quietly widen the grant.
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut checked = 0;
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        checked += 1;
        let text = std::fs::read_to_string(&path).unwrap();
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
                "{}: `{permission}` is more than local events and window state",
                path.display()
            );
        }
        for forbidden in ["shell", "fs:", "http", "updater", "process:"] {
            assert!(
                !granted.iter().any(|p| p.contains(forbidden)),
                "{}: capabilities grant `{forbidden}`",
                path.display()
            );
        }
    }
    assert!(checked >= 2, "the capability files should have been found");
}

#[test]
fn a_pack_cannot_describe_anything_executable() {
    // The condition grammar is the only thing a pack can express beyond data,
    // and it has no call, no assignment, and a closed signal namespace.
    use ledgeling::packs::Condition;
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
    use ledgeling::awareness::{AppCategory, CategoryMap};
    let map = CategoryMap::default();
    // The category is all that survives the mapping; the name is not stored on
    // the returned value in any form.
    let category = map.categorize("Slack");
    assert_eq!(category, AppCategory::Comms);
    assert_eq!(std::mem::size_of_val(&category), 1);
}
