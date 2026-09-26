//! Opt-in launch at login. Off by default, and removable from the same switch
//! that turned it on — Ledgeling installs nothing behind the user's back.
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

pub const IDENTIFIER: &str = "technology.lucerna-systems.ledgeling";

pub fn apply(enabled: bool, executable: &Path) -> Result<(), String> {
    if enabled {
        enable(executable)
    } else {
        disable()
    }
}

#[cfg(target_os = "macos")]
fn agent_path(identifier: &str) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Your home folder could not be found.")?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{identifier}.plist")))
}

#[cfg(target_os = "macos")]
fn remove_agent(identifier: &str) -> Result<(), String> {
    let path = agent_path(identifier)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove {}: {error}", path.display())),
    }
}

#[cfg(target_os = "macos")]
fn enable(executable: &Path) -> Result<(), String> {
    // A pre-rename agent would launch the app a second time at login.
    remove_agent(crate::migrate::LEGACY_IDENTIFIER)?;
    let path = agent_path(IDENTIFIER)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{IDENTIFIER}</string>
    <key>ProgramArguments</key><array><string>{}</string></array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
        executable.display()
    );
    std::fs::write(&path, plist).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

#[cfg(target_os = "macos")]
fn disable() -> Result<(), String> {
    remove_agent(crate::migrate::LEGACY_IDENTIFIER)?;
    remove_agent(IDENTIFIER)
}

#[cfg(target_os = "windows")]
mod registry {
    use std::os::windows::ffi::OsStrExt;
    type Key = isize;
    const HKEY_CURRENT_USER: Key = -2147483647;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;
    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: Key,
            path: *const u16,
            options: u32,
            access: u32,
            out: *mut Key,
        ) -> i32;
        fn RegSetValueExW(
            key: Key,
            name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            size: u32,
        ) -> i32;
        fn RegDeleteValueW(key: Key, name: *const u16) -> i32;
        fn RegCloseKey(key: Key) -> i32;
    }
    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    fn run_key() -> Result<Key, String> {
        let path = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
        let mut key: Key = 0;
        let result =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key) };
        if result != 0 {
            return Err(format!(
                "Windows would not open the startup list (error {result})."
            ));
        }
        Ok(key)
    }
    pub fn set(name: &str, command: &str) -> Result<(), String> {
        let key = run_key()?;
        let value = wide(command);
        let bytes = value.len() * 2;
        let result = unsafe {
            RegSetValueExW(
                key,
                wide(name).as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const u8,
                bytes as u32,
            )
        };
        unsafe { RegCloseKey(key) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!(
                "Windows would not add Ledgeling to startup (error {result})."
            ))
        }
    }
    pub fn clear(name: &str) -> Result<(), String> {
        let key = run_key()?;
        let result = unsafe { RegDeleteValueW(key, wide(name).as_ptr()) };
        unsafe { RegCloseKey(key) };
        // 2 = ERROR_FILE_NOT_FOUND: it was already absent.
        if result == 0 || result == 2 {
            Ok(())
        } else {
            Err(format!(
                "Windows would not remove Ledgeling from startup (error {result})."
            ))
        }
    }
}

/// The startup entry's name before the app was renamed; cleared so an
/// upgraded install never launches twice at login.
#[cfg(target_os = "windows")]
const LEGACY_ENTRY: &str = "Perch";

#[cfg(target_os = "windows")]
fn enable(executable: &Path) -> Result<(), String> {
    registry::clear(LEGACY_ENTRY)?;
    registry::set("Ledgeling", &format!("\"{}\"", executable.display()))
}

#[cfg(target_os = "windows")]
fn disable() -> Result<(), String> {
    registry::clear(LEGACY_ENTRY)?;
    registry::clear("Ledgeling")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn enable(_executable: &Path) -> Result<(), String> {
    Err("Opening at login is not wired up on this platform yet.".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn disable() -> Result<(), String> {
    Ok(())
}
