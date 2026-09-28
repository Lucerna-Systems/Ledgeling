//! Desktop apps (Tauri, Electron): how to start one for development, how to
//! build it for a preview, and where the last build is. A web project needs
//! none of this; its dev server's port is opened in the browser instead.
use serde::Serialize;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeApp {
    pub framework: &'static str,
    /// Starts the app with live reload.
    pub dev: String,
    /// Produces a runnable build, for previewing the app as users get it.
    pub build: String,
    pub built: Option<Built>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Built {
    /// Relative to the worktree, for display.
    pub path: String,
    pub name: String,
    /// Milliseconds since the epoch, so the UI can say how old the build is.
    pub modified: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

pub fn detect(root: &Path) -> Option<NativeApp> {
    detect_for(root, Os::current())
}

pub fn detect_for(root: &Path, os: Os) -> Option<NativeApp> {
    let package = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let has_dep = |name: &str| {
        package.as_ref().is_some_and(|p| {
            ["dependencies", "devDependencies"]
                .iter()
                .any(|k| p[k].get(name).is_some())
        })
    };
    let script = |name: &str| {
        package
            .as_ref()
            .and_then(|p| p["scripts"][name].as_str())
            .is_some()
    };
    let runner = super::run::runner(root);
    let run_script = |name: &str, args: &str| {
        let base = if runner == "npm run" {
            format!("npm run {name}")
        } else {
            format!("{runner} {name}")
        };
        // npm keeps flags for itself unless they follow `--`.
        if args.is_empty() {
            base
        } else if runner == "npm run" && args.split_whitespace().any(|w| w.starts_with('-')) {
            format!("{base} -- {args}")
        } else {
            format!("{base} {args}")
        }
    };

    if let Some(dir) = tauri_dir(root) {
        // A dev-only .app is enough to preview on macOS; the DMG takes minutes.
        let bundles = if os == Os::Mac { "--bundles app" } else { "" };
        let (dev, build) = if script("tauri") {
            (
                run_script("tauri", "dev"),
                run_script("tauri", format!("build {bundles}").trim_end()),
            )
        } else if has_dep("@tauri-apps/cli") {
            (
                "npx tauri dev".into(),
                format!("npx tauri build {bundles}").trim_end().to_string(),
            )
        } else {
            (
                "cargo tauri dev".into(),
                format!("cargo tauri build {bundles}")
                    .trim_end()
                    .to_string(),
            )
        };
        let targets = [dir.join("target"), root.join("target")];
        let built = newest(root, targets.iter().flat_map(|t| tauri_builds(t, os)));
        return Some(NativeApp {
            framework: "Tauri",
            dev,
            build,
            built,
        });
    }

    if has_dep("electron") {
        let dev = ["dev", "start"]
            .into_iter()
            .find(|s| script(s))
            .map(|s| run_script(s, ""))
            .unwrap_or_else(|| "npx electron .".into());
        let build = ["make", "dist", "package", "build"]
            .into_iter()
            .find(|s| script(s))
            .map(|s| run_script(s, ""))
            .unwrap_or_else(|| "npx electron-builder --dir".into());
        let outputs = ["out", "dist", "release"].map(|d| root.join(d));
        let built = newest(root, outputs.iter().flat_map(|o| electron_builds(o, os)));
        return Some(NativeApp {
            framework: "Electron",
            dev,
            build,
            built,
        });
    }
    None
}

fn tauri_dir(root: &Path) -> Option<PathBuf> {
    [root.join("src-tauri"), root.to_path_buf()]
        .into_iter()
        .find(|dir| {
            ["tauri.conf.json", "tauri.conf.json5", "Tauri.toml"]
                .iter()
                .any(|f| dir.join(f).is_file())
        })
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    path.is_file()
}

fn tauri_builds(target: &Path, os: Os) -> Vec<PathBuf> {
    let release = target.join("release");
    match os {
        Os::Mac => children(&release.join("bundle/macos"))
            .into_iter()
            .filter(|p| has_extension(p, "app"))
            .collect(),
        Os::Windows => children(&release)
            .into_iter()
            .filter(|p| has_extension(p, "exe"))
            .collect(),
        Os::Linux => {
            let images: Vec<_> = children(&release.join("bundle/appimage"))
                .into_iter()
                .filter(|p| has_extension(p, "AppImage"))
                .collect();
            if !images.is_empty() {
                return images;
            }
            children(&release)
                .into_iter()
                .filter(|p| p.extension().is_none() && executable(p))
                .collect()
        }
    }
}

/// electron-builder and Forge both nest the app a folder or two down, e.g.
/// `dist/mac-arm64/App.app` or `out/App-darwin-arm64/App.app`.
fn electron_builds(dir: &Path, os: Os) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut queue = vec![(dir.to_path_buf(), 0)];
    while let Some((dir, depth)) = queue.pop() {
        for path in children(&dir) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let wanted = match os {
                Os::Mac => has_extension(&path, "app"),
                Os::Windows => {
                    has_extension(&path, "exe")
                        && !["setup", "uninstall", "elevate", "squirrel"]
                            .iter()
                            .any(|skip| name.contains(skip))
                }
                Os::Linux => has_extension(&path, "AppImage"),
            };
            if wanted {
                found.push(path);
            } else if path.is_dir() && depth < 2 && !has_extension(&path, "app") {
                queue.push((path, depth + 1));
            }
        }
    }
    found
}

fn newest(root: &Path, candidates: impl Iterator<Item = PathBuf>) -> Option<Built> {
    candidates
        .filter_map(|path| {
            let modified = path
                .metadata()
                .and_then(|m| m.modified())
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_millis() as u64;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(modified, path)| Built {
            path: path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            name: path
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            modified,
        })
}

/// Start the newest build as its own app. The path is found again here
/// rather than taken from the window, so only a real build output is run.
pub fn open_built(root: &Path) -> Result<(), String> {
    let app = detect(root).ok_or("This project is not a desktop app.")?;
    let built = app
        .built
        .ok_or("There is no build yet. Build it for a preview first.")?;
    let path = root.join(&built.path);
    if cfg!(target_os = "macos") {
        return open::that_detached(&path).map_err(|_| "The build could not be opened.".into());
    }
    let mut child = std::process::Command::new(&path)
        .current_dir(path.parent().unwrap_or(root))
        .spawn()
        .map_err(|_| "The build could not be started.".to_string())?;
    // Reaped in the background so a closed preview does not linger as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    #[test]
    fn a_web_project_is_not_a_desktop_app() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", r#"{"scripts":{"dev":"vite"}}"#);
        assert_eq!(detect_for(dir.path(), Os::Mac), None);
    }

    #[test]
    fn tauri_uses_the_package_script_and_finds_the_app_bundle() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"scripts":{"tauri":"tauri"}}"#,
        );
        write(dir.path(), "src-tauri/tauri.conf.json", "{}");
        let app = detect_for(dir.path(), Os::Mac).unwrap();
        assert_eq!(app.framework, "Tauri");
        assert_eq!(app.dev, "npm run tauri dev");
        assert_eq!(app.build, "npm run tauri -- build --bundles app");
        assert_eq!(app.built, None);
        write(
            dir.path(),
            "src-tauri/target/release/bundle/macos/Demo.app/Contents/Info.plist",
            "",
        );
        let built = detect_for(dir.path(), Os::Mac).unwrap().built.unwrap();
        assert_eq!(built.name, "Demo");
        assert_eq!(built.path, "src-tauri/target/release/bundle/macos/Demo.app");
    }

    #[test]
    fn tauri_without_a_package_script_falls_back_to_cargo() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src-tauri/Tauri.toml", "");
        write(dir.path(), "pnpm-lock.yaml", "");
        let app = detect_for(dir.path(), Os::Windows).unwrap();
        assert_eq!(app.dev, "cargo tauri dev");
        assert_eq!(app.build, "cargo tauri build");
        write(dir.path(), "src-tauri/target/release/demo.exe", "");
        assert_eq!(
            detect_for(dir.path(), Os::Windows)
                .unwrap()
                .built
                .unwrap()
                .name,
            "demo"
        );
    }

    #[test]
    fn electron_finds_builder_and_forge_output_but_not_installers() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"scripts":{"start":"electron .","make":"electron-forge make"},"devDependencies":{"electron":"30"}}"#,
        );
        write(dir.path(), "yarn.lock", "");
        let app = detect_for(dir.path(), Os::Windows).unwrap();
        assert_eq!(app.framework, "Electron");
        assert_eq!(app.dev, "yarn start");
        assert_eq!(app.build, "yarn make");
        write(
            dir.path(),
            "out/make/squirrel.windows/x64/Demo Setup.exe",
            "",
        );
        assert_eq!(detect_for(dir.path(), Os::Windows).unwrap().built, None);
        write(dir.path(), "out/Demo-win32-x64/Demo.exe", "");
        assert_eq!(
            detect_for(dir.path(), Os::Windows)
                .unwrap()
                .built
                .unwrap()
                .name,
            "Demo"
        );
        write(
            dir.path(),
            "dist/mac-arm64/Demo.app/Contents/Info.plist",
            "",
        );
        assert_eq!(
            detect_for(dir.path(), Os::Mac).unwrap().built.unwrap().path,
            "dist/mac-arm64/Demo.app"
        );
    }
}
