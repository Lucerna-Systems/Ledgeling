//! How a project is run: `.ledgeling/run.json` in the project, which a person
//! or an AI agent writes, or a guess from the files that are there.
use crate::packs::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub const FILE: &str = ".ledgeling/run.json";

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RunConfig {
    pub name: String,
    pub command: String,
    /// Relative to the project root.
    pub cwd: String,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunSetup {
    /// What the file says, when it exists and is valid.
    pub configs: Vec<RunConfig>,
    pub from_file: bool,
    pub problems: Vec<Diagnostic>,
    /// Guesses from the project's files, offered when there is no file.
    pub suggestions: Vec<Suggestion>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub config: RunConfig,
    /// Why this guess, in words: "package.json has a dev script".
    pub reason: String,
}

/// The instruction given to an AI (the Chat tab's model or the user's own
/// `claude`) when asked to work out how the project runs.
pub const AI_PROMPT: &str = "Work out how to run this project locally for development, then create the file .ledgeling/run.json in the project root. It must be a JSON array of objects, one per thing to run (usually just one), each with: \"name\" (a short label such as \"Web app\"), \"command\" (the exact shell command, e.g. \"npm run dev\"), \"cwd\" (folder relative to the project root, \".\" for the root), \"port\" (the port number it listens on, or null if it is not a server), and \"url\" (the address to open in a browser, or null). Include \"env\" only if environment variables are required. Do not start the project and do not change any other file.";

pub fn load(root: &Path) -> RunSetup {
    let path = root.join(FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return RunSetup {
            suggestions: suggest(root),
            ..Default::default()
        };
    };
    match validate(&text) {
        Ok(configs) => RunSetup {
            configs,
            from_file: true,
            ..Default::default()
        },
        Err(problems) => RunSetup {
            problems,
            from_file: true,
            suggestions: suggest(root),
            ..Default::default()
        },
    }
}

fn problem(path: &str, what: impl Into<String>, expected: impl Into<String>) -> Diagnostic {
    Diagnostic::error(FILE, path, what, expected)
}

pub fn validate(text: &str) -> Result<Vec<RunConfig>, Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(text).map_err(|e| {
        vec![problem(
            "",
            format!(
                "This is not valid JSON (line {}, column {}).",
                e.line(),
                e.column()
            ),
            "a JSON array of run entries",
        )]
    })?;
    let items: Vec<(String, &Value)> = match &value {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("[{i}]"), v))
            .collect(),
        Value::Object(_) => vec![(String::new(), &value)],
        _ => {
            return Err(vec![problem(
                "",
                "The file holds neither a list nor an entry.",
                "a JSON array of run entries",
            )])
        }
    };
    if items.is_empty() {
        return Err(vec![problem(
            "",
            "The list is empty.",
            "at least one run entry",
        )]);
    }
    let mut problems = Vec::new();
    let mut configs = Vec::new();
    for (at, item) in items {
        let field = |name: &str| {
            if at.is_empty() {
                name.to_string()
            } else {
                format!("{at}.{name}")
            }
        };
        let Some(object) = item.as_object() else {
            problems.push(problem(
                &at,
                "This entry is not an object.",
                "an object with name and command",
            ));
            continue;
        };
        let text_of = |name: &str| object.get(name).and_then(Value::as_str).map(str::to_string);
        let mut config = RunConfig {
            name: text_of("name").unwrap_or_default(),
            command: text_of("command").unwrap_or_default(),
            cwd: text_of("cwd").unwrap_or_else(|| ".".into()),
            ..Default::default()
        };
        if config.command.trim().is_empty() {
            problems.push(problem(
                &field("command"),
                "The command is missing or empty.",
                "a shell command such as \"npm run dev\"",
            ));
        }
        if config.name.trim().is_empty() {
            config.name = config.command.clone();
        }
        let cwd = Path::new(&config.cwd);
        if cwd.is_absolute() || config.cwd.split(['/', '\\']).any(|part| part == "..") {
            problems.push(problem(
                &field("cwd"),
                format!("`{}` points outside the project.", config.cwd),
                "a folder inside the project, such as \".\" or \"web\"",
            ));
        }
        match object.get("port") {
            None | Some(Value::Null) => {}
            Some(Value::Number(n)) if n.as_u64().is_some_and(|p| (1..=65535).contains(&p)) => {
                config.port = n.as_u64().map(|p| p as u16)
            }
            Some(other) => problems.push(problem(
                &field("port"),
                format!("`{other}` is not a port."),
                "a whole number from 1 to 65535, or null",
            )),
        }
        match object.get("url") {
            None | Some(Value::Null) => {}
            Some(Value::String(url))
                if url.starts_with("http://") || url.starts_with("https://") =>
            {
                config.url = Some(url.clone())
            }
            Some(other) => problems.push(problem(
                &field("url"),
                format!("`{other}` is not a web address."),
                "an http:// or https:// address, or null",
            )),
        }
        match object.get("env") {
            None | Some(Value::Null) => {}
            Some(Value::Object(env)) => {
                for (key, value) in env {
                    match value.as_str() {
                        Some(text) => {
                            config.env.insert(key.clone(), text.to_string());
                        }
                        None => problems.push(problem(
                            &format!("{}.{key}", field("env")),
                            "This value is not text.",
                            "a string, e.g. \"development\"",
                        )),
                    }
                }
            }
            Some(_) => problems.push(problem(
                &field("env"),
                "This is not an object.",
                "an object of names to text values",
            )),
        }
        if config.url.is_none() {
            config.url = config.port.map(|p| format!("http://localhost:{p}"));
        }
        configs.push(config);
    }
    if problems.is_empty() {
        Ok(configs)
    } else {
        Err(problems)
    }
}

pub fn save(root: &Path, configs: &[RunConfig]) -> Result<(), String> {
    let path = root.join(FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(configs).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| e.to_string())
}

fn config(name: &str, command: &str, port: Option<u16>) -> RunConfig {
    RunConfig {
        name: name.into(),
        command: command.into(),
        cwd: ".".into(),
        port,
        url: port.map(|p| format!("http://localhost:{p}")),
        env: BTreeMap::new(),
    }
}

/// Best guesses from well-known project files, most likely first.
pub fn suggest(root: &Path) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if let Some(package) = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    {
        let runner = if root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
            "bun"
        } else {
            "npm run"
        };
        let has_dep = |name: &str| {
            ["dependencies", "devDependencies"]
                .iter()
                .any(|k| package[k].get(name).is_some())
        };
        for script in ["dev", "start", "serve"] {
            let Some(body) = package["scripts"][script].as_str() else {
                continue;
            };
            let port = explicit_port(body).or(if has_dep("vite") || body.contains("vite") {
                Some(5173)
            } else if has_dep("next") || has_dep("react-scripts") || has_dep("express") {
                Some(3000)
            } else if has_dep("astro") {
                Some(4321)
            } else if has_dep("@angular/core") {
                Some(4200)
            } else {
                None
            });
            let command = if runner == "npm run" {
                format!("npm run {script}")
            } else {
                format!("{runner} {script}")
            };
            out.push(Suggestion {
                config: config(
                    if port.is_some() { "Web app" } else { "App" },
                    &command,
                    port,
                ),
                reason: format!("package.json has a `{script}` script"),
            });
            break;
        }
    }
    if root.join("manage.py").exists() {
        out.push(Suggestion {
            config: config("Django", "python manage.py runserver", Some(8000)),
            reason: "manage.py is here".into(),
        });
    }
    if root.join("Cargo.toml").exists() {
        out.push(Suggestion {
            config: config("Rust", "cargo run", None),
            reason: "Cargo.toml is here".into(),
        });
    }
    if root.join("go.mod").exists() {
        out.push(Suggestion {
            config: config("Go", "go run .", None),
            reason: "go.mod is here".into(),
        });
    }
    for compose in [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yml",
        "docker-compose.yaml",
    ] {
        if root.join(compose).exists() {
            out.push(Suggestion {
                config: config("Containers", "docker compose up", None),
                reason: format!("{compose} is here"),
            });
            break;
        }
    }
    if let Ok(makefile) = std::fs::read_to_string(root.join("Makefile")) {
        for target in ["dev", "run", "serve", "start"] {
            if makefile
                .lines()
                .any(|l| l.starts_with(&format!("{target}:")))
            {
                out.push(Suggestion {
                    config: config("Make", &format!("make {target}"), None),
                    reason: format!("the Makefile has a `{target}` target"),
                });
                break;
            }
        }
    }
    if root.join("index.html").exists() && out.is_empty() {
        out.push(Suggestion {
            config: config("Static site", "python3 -m http.server 8000", Some(8000)),
            reason: "index.html is here".into(),
        });
    }
    out
}

/// `--port 4000`, `--port=4000`, `-p 4000` or `PORT=4000` inside a script.
fn explicit_port(script: &str) -> Option<u16> {
    let words: Vec<&str> = script.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        for prefix in ["--port=", "PORT="] {
            if let Some(n) = word.strip_prefix(prefix) {
                return n.parse().ok();
            }
        }
        if matches!(*word, "--port" | "-p") {
            return words.get(i + 1)?.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_file_loads() {
        let configs = validate(
            r#"[{"name":"Web","command":"npm run dev","cwd":".","port":5173,"url":null}]"#,
        )
        .unwrap();
        assert_eq!(configs[0].port, Some(5173));
        assert_eq!(configs[0].url.as_deref(), Some("http://localhost:5173"));
        let single = validate(r#"{"command":"cargo run"}"#).unwrap();
        assert_eq!(single[0].name, "cargo run");
        assert_eq!(single[0].cwd, ".");
    }

    #[test]
    fn every_problem_names_where_and_what_was_expected() {
        let problems =
            validate(r#"[{"command":"","port":"x","cwd":"../up","url":"ftp://a","env":{"A":1}}]"#)
                .unwrap_err();
        let paths: Vec<_> = problems.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(
            paths,
            ["[0].command", "[0].cwd", "[0].port", "[0].url", "[0].env.A"]
        );
        assert!(problems
            .iter()
            .all(|p| p.file == FILE && !p.expected.is_empty()));
        let broken = validate("{nope").unwrap_err();
        assert!(broken[0].problem.contains("line 1"));
        assert!(
            !broken[0].problem.contains("key must be a string"),
            "no raw serde text"
        );
    }

    #[test]
    fn suggestions_follow_the_project_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"},"devDependencies":{"vite":"7"}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let found = suggest(dir.path());
        assert_eq!(found[0].config.command, "pnpm dev");
        assert_eq!(found[0].config.port, Some(5173));
        assert_eq!(found[1].config.command, "cargo run");
    }

    #[test]
    fn explicit_ports_win() {
        assert_eq!(explicit_port("next dev --port 4000"), Some(4000));
        assert_eq!(explicit_port("PORT=8080 node server.js"), Some(8080));
        assert_eq!(explicit_port("vite"), None);
    }

    #[test]
    fn a_saved_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let configs = vec![config("Web", "npm run dev", Some(5173))];
        save(dir.path(), &configs).unwrap();
        let setup = load(dir.path());
        assert!(setup.from_file);
        assert_eq!(setup.configs, configs);
    }
}
