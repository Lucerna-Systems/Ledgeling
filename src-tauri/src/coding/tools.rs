//! The developer tools coding mode can use, found on the user's PATH. Nothing
//! is installed without the user pressing Install and watching it run.
use serde::Serialize;
use std::{path::PathBuf, process::Command};

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: &'static str,
    pub name: &'static str,
    pub purpose: &'static str,
    pub version: Option<String>,
    /// The bundled install script for this platform, when there is one.
    pub installer: Option<String>,
    pub homepage: &'static str,
}

struct Known {
    id: &'static str,
    name: &'static str,
    program: &'static str,
    purpose: &'static str,
    homepage: &'static str,
    scripted: bool,
}

const KNOWN: &[Known] = &[
    Known {
        id: "git",
        name: "Git",
        program: "git",
        purpose: "Worktrees, changes and commits",
        homepage: "https://git-scm.com/downloads",
        scripted: false,
    },
    Known {
        id: "node",
        name: "Node.js",
        program: "node",
        purpose: "Runs most web projects",
        homepage: "https://nodejs.org/",
        scripted: false,
    },
    Known {
        id: "claude",
        name: "Claude Code",
        program: "claude",
        purpose: "The AI coding agent for the Terminal tab",
        homepage: "https://docs.anthropic.com/en/docs/claude-code",
        scripted: true,
    },
    Known {
        id: "orca",
        name: "Orca",
        program: "orca",
        purpose: "Optional: open worktrees in the Orca IDE",
        homepage: "https://github.com/stablyai/orca",
        scripted: true,
    },
];

/// Where a program lives on the login shell's PATH, which is the one the
/// Terminal tab uses.
pub fn find(program: &str) -> Option<PathBuf> {
    let path = super::pty::login_path().or_else(|| std::env::var("PATH").ok())?;
    let extensions: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    std::env::split_paths(&path).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let candidate = dir.join(format!("{program}{ext}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

fn version(program: &PathBuf) -> Option<String> {
    let mut command = Command::new(program);
    command.arg("--version");
    if let Some(path) = super::pty::login_path() {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or_default().trim();
    Some(if line.is_empty() {
        "installed".into()
    } else {
        line.chars().take(60).collect()
    })
}

pub fn detect(scripts: Option<PathBuf>) -> Vec<Tool> {
    let extension = if cfg!(windows) { "ps1" } else { "sh" };
    KNOWN
        .iter()
        .map(|known| Tool {
            id: known.id,
            name: known.name,
            purpose: known.purpose,
            version: find(known.program).and_then(|p| version(&p)),
            installer: scripts
                .as_ref()
                .filter(|_| known.scripted)
                .map(|dir| dir.join(format!("install-{}.{extension}", known.id)))
                .filter(|p| p.is_file())
                .map(|p| p.to_string_lossy().into_owned()),
            homepage: known.homepage,
        })
        .collect()
}
