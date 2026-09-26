//! Approvals are the one place a user must understand exactly what is about to
//! happen, and every producer of one — the file planner, Codex's JSON-RPC, the
//! Claude bridge's hook payloads — speaks in its own internal shapes. This
//! turns any of them into the same short, human summary. No approval surface
//! ever renders the raw `details` value.
use super::types::ApprovalSummary;
use serde_json::Value;
use std::path::Path;

/// How many lines an approval ever shows before collapsing into "+N more".
const MAX_ITEMS: usize = 6;
/// Longest single value kept from an unrecognised payload.
const MAX_VALUE: usize = 220;

pub fn summarize(title: &str, details: &Value) -> ApprovalSummary {
    if let Some(operations) = details["operations"].as_array() {
        return file_changes(operations);
    }
    if let Some(runtime) = details["runtime"].as_str() {
        return agent_start(runtime, details);
    }
    // The Claude bridge names the tool it is about to use; Codex wraps its own
    // request, which may carry a command or a set of file changes.
    if let Some(tool) = details["tool"].as_str() {
        return claude_action(tool, details);
    }
    if details.get("request").is_some() || details.get("changes").is_some() {
        return codex_action(details);
    }
    unrecognised(title, details)
}

fn file_changes(operations: &[Value]) -> ApprovalSummary {
    let kind_of = |op: &Value| op["kind"].as_str().unwrap_or("").to_string();
    let moves = operations.iter().filter(|op| kind_of(op) == "move").count();
    let writes = operations
        .iter()
        .filter(|op| kind_of(op) == "write")
        .count();
    let destinations: Vec<&str> = operations
        .iter()
        .filter_map(|op| op["destination"].as_str())
        .collect();
    // Sources count towards the root as well, so a move deeper into the folder
    // the user chose still shows the subfolder it lands in.
    let touched: Vec<&str> = destinations
        .iter()
        .copied()
        .chain(operations.iter().filter_map(|op| op["source"].as_str()))
        .collect();
    let root = common_parent(&touched);
    let into = shared_folder(&destinations).map(|name| format!(" into {name}"));
    let headline = match (moves, writes) {
        (m, 0) if m > 0 => format!(
            "Move {}{}",
            count(m, "file"),
            into.clone().unwrap_or_default()
        ),
        (0, w) if w > 0 => format!("Create {}", count(w, "new file")),
        _ => format!("Apply {}", count(operations.len(), "file change")),
    };
    let items = operations
        .iter()
        .take(MAX_ITEMS)
        .map(|op| {
            let destination = relative(op["destination"].as_str().unwrap_or(""), root.as_deref());
            match (kind_of(op).as_str(), op["source"].as_str()) {
                ("move", Some(source)) => {
                    format!("{} → {destination}", file_name(source))
                }
                ("write", _) => format!("New file · {destination}"),
                _ => destination,
            }
        })
        .collect();
    ApprovalSummary {
        headline,
        items,
        more: operations.len().saturating_sub(MAX_ITEMS),
        note: "Nothing existing is overwritten, and these changes can be undone afterwards.".into(),
        accept: "Approve".into(),
        monospace: false,
    }
}

fn agent_start(runtime: &str, details: &Value) -> ApprovalSummary {
    let folder = details["folder"].as_str().unwrap_or("");
    let mut items = Vec::new();
    if !folder.is_empty() {
        items.push(format!("Works in {}", file_name(folder)));
    }
    items.push("Every command and file change is approved separately".into());
    ApprovalSummary {
        headline: format!("Let {runtime} work on this task"),
        items,
        more: 0,
        // The runtime's own caveat is written for people; keep it verbatim.
        note: details["notice"].as_str().unwrap_or("").into(),
        accept: "Start".into(),
        monospace: false,
    }
}

fn claude_action(tool: &str, details: &Value) -> ApprovalSummary {
    let input = &details["input"];
    let path = ["file_path", "path", "pattern"]
        .iter()
        .find_map(|key| input[*key].as_str());
    let (headline, items, monospace) = match tool {
        "Bash" => (
            "Run a command".to_string(),
            command_lines(input).unwrap_or_else(|| vec!["Command not readable".into()]),
            true,
        ),
        "Write" => (
            format!("Write {}", path.map(file_name).unwrap_or("a file".into())),
            path.into_iter().map(str::to_string).collect(),
            false,
        ),
        "Edit" => (
            format!("Edit {}", path.map(file_name).unwrap_or("a file".into())),
            path.into_iter().map(str::to_string).collect(),
            false,
        ),
        "Read" | "Glob" | "Grep" => (
            format!("Read {}", path.map(file_name).unwrap_or("files".into())),
            path.into_iter().map(str::to_string).collect(),
            false,
        ),
        "WebSearch" | "WebFetch" => (
            "Look something up on the web".to_string(),
            ["query", "url", "prompt"]
                .iter()
                .filter_map(|key| input[*key].as_str().map(str::to_string))
                .collect(),
            false,
        ),
        other => (format!("Use {other}"), leaf_lines(input, MAX_ITEMS), false),
    };
    ApprovalSummary {
        headline,
        items,
        more: 0,
        note: details["notice"].as_str().unwrap_or("").into(),
        accept: "Allow".into(),
        monospace,
    }
}

fn codex_action(details: &Value) -> ApprovalSummary {
    let request = if details["request"].is_null() {
        details
    } else {
        &details["request"]
    };
    if let Some(lines) = find_command(request) {
        return ApprovalSummary {
            headline: "Run a command".into(),
            items: lines,
            more: 0,
            note: "Commands act outside the file preview and cannot always be undone.".into(),
            accept: "Allow".into(),
            monospace: true,
        };
    }
    let paths = change_paths(&details["changes"]);
    if !paths.is_empty() {
        let root = common_parent(&paths.iter().map(String::as_str).collect::<Vec<_>>());
        return ApprovalSummary {
            headline: format!("Edit {}", count(paths.len(), "file")),
            items: paths
                .iter()
                .take(MAX_ITEMS)
                .map(|p| relative(p, root.as_deref()))
                .collect(),
            more: paths.len().saturating_sub(MAX_ITEMS),
            note: "The agent wrote these files itself; Ledgeling cannot undo them for you.".into(),
            accept: "Allow".into(),
            monospace: false,
        };
    }
    ApprovalSummary {
        headline: "The agent wants to make a change".into(),
        items: leaf_lines(request, MAX_ITEMS),
        more: 0,
        note: "Approve only if this matches what you asked for.".into(),
        accept: "Allow".into(),
        monospace: false,
    }
}

/// Last resort: a shape nobody here recognises. Showing the raw payload would
/// be showing the agent's internals, so scalar leaves are relabelled instead,
/// and the note says plainly that Ledgeling could not read the request.
fn unrecognised(title: &str, details: &Value) -> ApprovalSummary {
    ApprovalSummary {
        headline: if title.is_empty() {
            "The task needs your approval".into()
        } else {
            title.into()
        },
        items: leaf_lines(details, MAX_ITEMS),
        more: 0,
        note: "Ledgeling could not read this request in full. Approve only if you understand it."
            .into(),
        accept: "Approve".into(),
        monospace: false,
    }
}

/// What a tool call is doing, in the user's words. Tool names are Ledgeling's
/// internal vocabulary and its models'; progress lines are read by people.
pub fn tool_label(tool: &str) -> &'static str {
    match tool {
        "list_files" => "Looking through your files",
        "read_file" => "Reading a file",
        "find_files" => "Searching your files",
        "duplicates" => "Comparing files",
        "changes" => "Preparing changes for you to review",
        "search_web" => "Searching the web",
        "read_page" => "Reading a web page",
        _ => "Working",
    }
}

/// The same, for the item types a Codex turn reports.
pub fn agent_activity(kind: &str) -> &'static str {
    match kind {
        "commandExecution" => "Running a command",
        "fileChange" => "Preparing file changes",
        "agentMessage" | "assistantMessage" => "Writing a reply",
        "reasoning" => "Thinking",
        "webSearch" => "Searching the web",
        "todoList" | "plan" => "Planning the work",
        _ => "Working",
    }
}

// ------------------------------------------------------------------- helpers

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Deepest directory every path shares, so destinations can be shown relative
/// to the folder the user chose rather than as absolute paths.
fn common_parent(paths: &[&str]) -> Option<String> {
    let mut shared: Option<Vec<&str>> = None;
    for path in paths {
        let parts: Vec<&str> = Path::new(path)
            .parent()?
            .to_str()?
            .split('/')
            .filter(|p| !p.is_empty())
            .collect();
        shared = Some(match shared {
            None => parts,
            Some(current) => current
                .into_iter()
                .zip(parts)
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect(),
        });
    }
    shared
        .filter(|parts| !parts.is_empty())
        .map(|parts| format!("/{}", parts.join("/")))
}

/// The folder name every destination lands in, when there is exactly one.
fn shared_folder(destinations: &[&str]) -> Option<String> {
    let mut folders = destinations
        .iter()
        .filter_map(|d| Path::new(d).parent())
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    folders.sort();
    folders.dedup();
    match folders.as_slice() {
        [only] => Some(file_name(only)),
        _ => None,
    }
}

fn relative(path: &str, root: Option<&str>) -> String {
    match root {
        Some(root) => path
            .strip_prefix(root)
            .map(|rest| rest.trim_start_matches('/').to_string())
            .filter(|rest| !rest.is_empty())
            .unwrap_or_else(|| path.to_string()),
        None => path.to_string(),
    }
}

/// A command as the user would read it, from wherever in the payload it sits.
fn find_command(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Object(map) => {
            for key in ["command", "cmd"] {
                if let Some(found) = map.get(key).and_then(command_text) {
                    let mut lines = vec![found];
                    if let Some(cwd) = map.get("cwd").and_then(Value::as_str) {
                        lines.push(format!("in {cwd}"));
                    }
                    if let Some(reason) = map.get("reason").and_then(Value::as_str) {
                        lines.push(reason.into());
                    }
                    return Some(lines);
                }
            }
            map.values().find_map(find_command)
        }
        Value::Array(items) => items.iter().find_map(find_command),
        _ => None,
    }
}

fn command_lines(input: &Value) -> Option<Vec<String>> {
    let command = input["command"].as_str().map(str::to_string)?;
    let mut lines = vec![command];
    if let Some(description) = input["description"].as_str() {
        lines.push(description.into());
    }
    Some(lines)
}

fn command_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(clamp(text)),
        Value::Array(parts) => {
            let words: Vec<String> = parts
                .iter()
                .filter_map(|p| p.as_str().map(str::to_string))
                .collect();
            (!words.is_empty()).then(|| clamp(&words.join(" ")))
        }
        _ => None,
    }
}

/// Paths out of a Codex `changes` payload, which is either a map keyed by path
/// or a list of objects carrying one.
fn change_paths(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map.keys().cloned().collect(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                ["path", "file", "destination"]
                    .iter()
                    .find_map(|key| item[*key].as_str().map(str::to_string))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Scalar leaves as `Label — value`, used only when nothing else could be
/// recognised. Keys become sentence-case words so the output reads as English
/// rather than as the payload it came from.
fn leaf_lines(value: &Value, limit: usize) -> Vec<String> {
    let mut lines = Vec::new();
    walk(value, "", &mut lines, limit);
    lines
}

fn walk(value: &Value, label: &str, out: &mut Vec<String>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                walk(child, key, out, limit);
            }
        }
        Value::Array(items) => {
            for child in items {
                walk(child, label, out, limit);
            }
        }
        Value::Null => {}
        other => {
            let text = match other {
                Value::String(text) => clamp(text),
                other => other.to_string(),
            };
            if text.trim().is_empty() {
                return;
            }
            out.push(if label.is_empty() {
                text
            } else {
                format!("{} — {text}", humanize(label))
            });
        }
    }
}

fn humanize(key: &str) -> String {
    let spaced = key.replace(['_', '-'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

fn clamp(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_VALUE {
        return text.to_string();
    }
    text.chars().take(MAX_VALUE).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The one invariant every summary shares: nothing reaches a user that
    /// reads like the payload it came from.
    fn assert_human(summary: &ApprovalSummary) {
        let text = format!(
            "{} {} {}",
            summary.headline,
            summary.items.join(" "),
            summary.note
        );
        for internal in ["{\"", "\":", "[{", "}]", "null"] {
            assert!(
                !text.contains(internal),
                "summary leaked raw JSON: {text} (contains {internal})"
            );
        }
        assert!(!summary.headline.is_empty());
        assert!(!summary.accept.is_empty());
    }

    #[test]
    fn file_moves_read_as_a_sentence_and_hide_the_long_tail() {
        let operations: Vec<Value> = (0..9)
            .map(|i| {
                json!({"kind":"move","source":format!("/chosen/file{i}.txt"),
                       "destination":format!("/chosen/Documents/file{i}.txt"),"state":"planned"})
            })
            .collect();
        let summary = summarize("Review file changes", &json!({ "operations": operations }));
        assert_human(&summary);
        assert_eq!(summary.headline, "Move 9 files into Documents");
        assert_eq!(summary.items.len(), MAX_ITEMS);
        assert_eq!(summary.more, 3);
        assert_eq!(summary.items[0], "file0.txt → Documents/file0.txt");
        assert_eq!(summary.accept, "Approve");
    }

    #[test]
    fn a_single_written_file_says_so() {
        let summary = summarize(
            "Review file changes",
            &json!({"operations":[{"kind":"write","destination":"/chosen/notes.md","state":"planned"}]}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Create 1 new file");
        assert_eq!(summary.items, vec!["New file · notes.md"]);
    }

    #[test]
    fn agent_start_names_the_runtime_and_keeps_its_caveat() {
        let summary = summarize(
            "Start advanced agent",
            &json!({"runtime":"Codex","folder":"/Users/x/Trip","notice":"Commands do not have universal undo."}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Let Codex work on this task");
        assert_eq!(summary.accept, "Start");
        assert!(summary.items.contains(&"Works in Trip".to_string()));
        assert_eq!(summary.note, "Commands do not have universal undo.");
    }

    #[test]
    fn claude_tool_calls_become_plain_requests() {
        let summary = summarize(
            "Review Claude action",
            &json!({"tool":"Bash","input":{"command":"rm -rf build","description":"Clear the build"},
                    "cwd":"/p","notice":"Review this exact command."}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Run a command");
        assert_eq!(summary.items[0], "rm -rf build");
        assert!(summary.monospace);
        let summary = summarize(
            "Review Claude action",
            &json!({"tool":"Write","input":{"file_path":"/p/report.md","content":"…"},"notice":""}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Write report.md");
    }

    #[test]
    fn codex_requests_surface_the_command_wherever_it_sits() {
        let summary = summarize(
            "Review agent command",
            &json!({"request":{"threadId":"t","item":{"command":["git","status"],"cwd":"/p"}},"changes":null}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Run a command");
        assert_eq!(summary.items[0], "git status");
        assert_eq!(summary.accept, "Allow");
    }

    #[test]
    fn codex_file_changes_list_paths_not_payloads() {
        let summary = summarize(
            "Review agent file changes",
            &json!({"request":{"threadId":"t"},"changes":{"/p/src/main.rs":{"kind":"edit"}}}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Edit 1 file");
        assert_eq!(summary.items, vec!["main.rs"]);
    }

    #[test]
    fn an_unknown_shape_is_relabelled_never_dumped() {
        let summary = summarize(
            "Something new",
            &json!({"mystery_field":"do the thing","nested":{"another_key":42,"empty":null}}),
        );
        assert_human(&summary);
        assert_eq!(summary.headline, "Something new");
        assert!(summary
            .items
            .contains(&"Mystery field — do the thing".into()));
        assert!(summary.items.contains(&"Another key — 42".into()));
        assert!(summary.note.contains("could not read this request"));
    }

    #[test]
    fn oversized_values_are_clamped() {
        let summary = summarize("x", &json!({"blob":"a".repeat(5_000)}));
        assert!(summary.items[0].chars().count() < MAX_VALUE + 40);
    }
}
