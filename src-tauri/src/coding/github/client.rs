use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn program() -> Result<PathBuf, String> {
    super::super::tools::find("gh").ok_or_else(|| {
        "The GitHub CLI (gh) is not installed. Install it from the GitHub view or Add-ons.".into()
    })
}

/// `gh` configured never to prompt, colour or check for updates, since nobody
/// can answer an invisible process.
pub fn command(dir: &Path) -> Result<Command, String> {
    let mut command = Command::new(program()?);
    command
        .current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_SPINNER_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    if let Some(path) = super::super::pty::login_path() {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    Ok(command)
}

/// Runs `gh` in `dir`, handing `input` to it on stdin when given.
pub fn run_in(dir: &Path, args: &[String], input: Option<&str>) -> Result<String, String> {
    let mut command = command(dir)?;
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .spawn()
        .map_err(|_| "The GitHub CLI (gh) could not be started.".to_string())?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(friendly(if stderr.trim().is_empty() {
            &stdout
        } else {
            &stderr
        }))
    }
}

/// Everything except checkout runs outside any checkout, with the repository
/// named explicitly, so `gh` never switches or deletes a local branch.
pub fn run(args: &[String], input: Option<&str>) -> Result<String, String> {
    run_in(&std::env::temp_dir(), args, input)
}

pub fn json_of(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|_| "GitHub sent an answer Ledgeling could not read.".into())
}

/// Turns `gh`'s messages into ones that say what to do next.
pub fn friendly(message: &str) -> String {
    let text = message
        .trim()
        .trim_start_matches("gh: ")
        .trim_start_matches("GraphQL: ");
    let lower = text.to_lowercase();
    if lower.contains("gh auth login") || lower.contains("not logged in") {
        "You are not signed in to GitHub. Press Sign in in the GitHub view.".into()
    } else if lower.contains("insufficient_scopes") || lower.contains("required scopes") {
        "Your GitHub sign-in lacks permission for boards. Press Grant board access.".into()
    } else if lower.contains("none of the git remotes")
        || lower.contains("no git remotes")
        || lower.contains("not a git repository")
    {
        "This worktree has no GitHub remote.".into()
    } else if lower.contains("could not resolve host") || lower.contains("error connecting") {
        "GitHub could not be reached. Check your connection.".into()
    } else if text.is_empty() {
        "The GitHub CLI failed without saying why.".into()
    } else {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .take(6)
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(600)
            .collect()
    }
}

pub fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

pub fn repo_arg(repo: &str) -> Result<String, String> {
    let valid = repo.split('/').count() == 2
        && !repo.starts_with('-')
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
    if valid {
        Ok(format!("--repo={repo}"))
    } else {
        Err("That is not a GitHub repository name.".into())
    }
}

pub fn title_arg(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Give it a title.".into());
    }
    Ok(format!("--title={}", title.replace('\n', " ")))
}

pub fn names(flag: &str, values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| format!("--{flag}={}", v.replace(',', "")))
        .collect()
}
