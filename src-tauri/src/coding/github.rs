//! GitHub through the user's own `gh` CLI: its sign-in and its connection, so
//! Ledgeling opens no socket itself and sees exactly what the user can.
//!
//! The window never passes `gh` arguments through. Each operation below builds
//! its own argument list from typed, checked values, and text the user wrote
//! (titles aside) travels on stdin rather than the command line.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const PR_LIST_FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,reviewDecision,statusCheckRollup,additions,deletions,updatedAt,createdAt,labels,comments,url,assignees";
const PR_FIELDS: &str = "number,title,body,state,isDraft,author,createdAt,updatedAt,url,headRefName,baseRefName,headRefOid,isCrossRepository,headRepositoryOwner,mergeable,mergeStateStatus,reviewDecision,statusCheckRollup,additions,deletions,changedFiles,commits,comments,reviews,reviewRequests,latestReviews,labels,assignees,closingIssuesReferences,mergedAt,mergedBy,milestone,autoMergeRequest";
const ISSUE_LIST_FIELDS: &str =
    "number,title,state,stateReason,author,labels,assignees,comments,updatedAt,createdAt,url";
const ISSUE_FIELDS: &str = "number,title,body,state,stateReason,author,createdAt,updatedAt,closedAt,url,comments,labels,assignees,milestone,closedByPullRequestsReferences";

fn program() -> Result<PathBuf, String> {
    super::tools::find("gh").ok_or_else(|| {
        "The GitHub CLI (gh) is not installed. Install it from the GitHub view or Add-ons.".into()
    })
}

/// `gh` configured never to prompt, colour or check for updates, since nobody
/// can answer an invisible process.
fn command(dir: &Path) -> Result<Command, String> {
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
    if let Some(path) = super::pty::login_path() {
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
fn run_in(dir: &Path, args: &[String], input: Option<&str>) -> Result<String, String> {
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
fn run(args: &[String], input: Option<&str>) -> Result<String, String> {
    run_in(&std::env::temp_dir(), args, input)
}

fn json_of(text: &str) -> Result<Value, String> {
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

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

// ------------------------------------------------------------------ status

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub name_with_owner: String,
    pub url: String,
    pub default_branch: String,
    pub viewer_permission: String,
    pub is_fork: bool,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GhStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub signed_in: bool,
    pub account: Option<String>,
    pub scopes: Vec<String>,
    pub repo: Option<Repo>,
    pub problem: Option<String>,
}

/// The signed-in account and its token scopes from `gh auth status --json hosts`.
pub fn parse_auth(text: &str) -> Option<(String, Vec<String>)> {
    let value: Value = serde_json::from_str(text).ok()?;
    let accounts = value["hosts"]["github.com"].as_array()?;
    let account = accounts
        .iter()
        .find(|a| a["active"].as_bool() == Some(true) && a["state"] == "success")?;
    let scopes = account["scopes"]
        .as_str()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some((account["login"].as_str()?.to_string(), scopes))
}

pub fn parse_repo(value: &Value) -> Option<Repo> {
    Some(Repo {
        id: value["id"].as_str()?.into(),
        owner: value["owner"]["login"].as_str()?.into(),
        name: value["name"].as_str()?.into(),
        name_with_owner: value["nameWithOwner"].as_str()?.into(),
        url: value["url"].as_str()?.into(),
        default_branch: value["defaultBranchRef"]["name"]
            .as_str()
            .unwrap_or("main")
            .into(),
        viewer_permission: value["viewerPermission"].as_str().unwrap_or("READ").into(),
        is_fork: value["isFork"].as_bool().unwrap_or(false),
    })
}

/// Whether `gh` is installed and signed in, and which repository `dir` pushes to.
pub fn status(dir: &Path) -> GhStatus {
    let Ok(mut version) = command(dir) else {
        return GhStatus::default();
    };
    let version = version.arg("--version").output().ok().map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    });
    let mut status = GhStatus {
        installed: true,
        version,
        ..Default::default()
    };
    // `auth status` exits non-zero when any stored account is broken, so the
    // JSON is read either way.
    let auth = command(dir)
        .ok()
        .and_then(|mut c| {
            c.args([
                "auth",
                "status",
                "--hostname",
                "github.com",
                "--json",
                "hosts",
            ])
            .output()
            .ok()
        })
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    match parse_auth(&auth) {
        Some((login, scopes)) => {
            status.signed_in = true;
            status.account = Some(login);
            status.scopes = scopes;
        }
        None => return status,
    }
    match run_in(
        dir,
        &args(&[
            "repo",
            "view",
            "--json",
            "id,name,owner,nameWithOwner,url,defaultBranchRef,viewerPermission,isFork",
        ]),
        None,
    ) {
        Ok(text) => status.repo = json_of(&text).ok().as_ref().and_then(parse_repo),
        Err(e) => status.problem = Some(e),
    }
    status
}

// ------------------------------------------------------------------- lists

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Pr,
    Issue,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::Pr => "pr",
            Kind::Issue => "issue",
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    Open,
    Closed,
    Merged,
    All,
}

fn repo_arg(repo: &str) -> Result<String, String> {
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

pub fn list(repo: &str, kind: Kind, filter: Filter, search: &str) -> Result<Value, String> {
    let state = match (kind, filter) {
        (Kind::Issue, Filter::Merged) => "closed",
        (_, Filter::Open) => "open",
        (_, Filter::Closed) => "closed",
        (_, Filter::Merged) => "merged",
        (_, Filter::All) => "all",
    };
    let fields = match kind {
        Kind::Pr => PR_LIST_FIELDS,
        Kind::Issue => ISSUE_LIST_FIELDS,
    };
    let mut list = vec![
        kind.noun().to_string(),
        "list".into(),
        repo_arg(repo)?,
        format!("--state={state}"),
        "--limit=100".into(),
        format!("--json={fields}"),
    ];
    if !search.trim().is_empty() {
        list.push(format!("--search={}", search.trim()));
    }
    json_of(&run(&list, None)?)
}

pub fn view(repo: &str, kind: Kind, number: u64) -> Result<Value, String> {
    let fields = match kind {
        Kind::Pr => PR_FIELDS,
        Kind::Issue => ISSUE_FIELDS,
    };
    json_of(&run(
        &[
            kind.noun().into(),
            "view".into(),
            number.to_string(),
            repo_arg(repo)?,
            format!("--json={fields}"),
        ],
        None,
    )?)
}

pub fn diff(repo: &str, number: u64) -> Result<String, String> {
    run(
        &[
            "pr".into(),
            "diff".into(),
            number.to_string(),
            repo_arg(repo)?,
            "--color=never".into(),
        ],
        None,
    )
}

/// The newest pull request whose head is `branch`, open ones first.
pub fn branch_pr(repo: &str, branch: &str) -> Result<Option<Value>, String> {
    if branch.starts_with('-') || branch.is_empty() {
        return Ok(None);
    }
    let found = json_of(&run(
        &[
            "pr".into(),
            "list".into(),
            repo_arg(repo)?,
            format!("--head={branch}"),
            "--state=all".into(),
            "--limit=5".into(),
            format!("--json={PR_LIST_FIELDS}"),
        ],
        None,
    )?)?;
    let mut prs = found.as_array().cloned().unwrap_or_default();
    prs.sort_by_key(|p| {
        (
            p["state"] != "OPEN",
            std::cmp::Reverse(p["number"].as_u64()),
        )
    });
    Ok(prs.into_iter().next())
}

/// Labels and people who can be assigned, for the pickers.
pub fn meta(repo: &str) -> Result<Value, String> {
    let labels = json_of(&run(
        &[
            "label".into(),
            "list".into(),
            repo_arg(repo)?,
            "--limit=200".into(),
            "--json=name,color,description".into(),
        ],
        None,
    )?)?;
    let people = run(
        &[
            "api".into(),
            format!("repos/{repo}/assignees?per_page=100"),
            "--jq=[.[].login]".into(),
        ],
        None,
    )
    .ok()
    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    .unwrap_or_else(|| json!([]));
    Ok(json!({ "labels": labels, "assignees": people }))
}

// ----------------------------------------------------------------- actions

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEvent {
    Approve,
    RequestChanges,
    Comment,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    Completed,
    NotPlanned,
}

/// Everything the window can change on GitHub. Each variant maps to one
/// fixed `gh` invocation; nothing else can be run.
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Action {
    Comment {
        kind: Kind,
        number: u64,
        body: String,
    },
    Close {
        kind: Kind,
        number: u64,
        reason: Option<CloseReason>,
    },
    Reopen {
        kind: Kind,
        number: u64,
    },
    Edit {
        kind: Kind,
        number: u64,
        title: Option<String>,
        body: Option<String>,
    },
    Labels {
        kind: Kind,
        number: u64,
        add: Vec<String>,
        remove: Vec<String>,
    },
    Assignees {
        kind: Kind,
        number: u64,
        add: Vec<String>,
        remove: Vec<String>,
    },
    Merge {
        number: u64,
        method: MergeMethod,
        #[serde(rename = "deleteBranch")]
        delete_branch: bool,
        auto: bool,
    },
    Review {
        number: u64,
        event: ReviewEvent,
        body: String,
    },
    Ready {
        number: u64,
        ready: bool,
    },
    /// Re-runs the failed jobs of one Actions workflow run.
    RerunFailed {
        run: u64,
    },
    CreateIssue {
        title: String,
        body: String,
        labels: Vec<String>,
        assignees: Vec<String>,
    },
}

fn title_arg(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Give it a title.".into());
    }
    Ok(format!("--title={}", title.replace('\n', " ")))
}

fn names(flag: &str, values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| format!("--{flag}={}", v.replace(',', "")))
        .collect()
}

/// The `gh` argument list and stdin for one action.
pub fn plan(repo: &str, action: &Action) -> Result<(Vec<String>, Option<String>), String> {
    let repo = repo_arg(repo)?;
    let body_from_stdin = || "--body-file=-".to_string();
    Ok(match action {
        Action::Comment { kind, number, body } => {
            if body.trim().is_empty() {
                return Err("Write something first.".into());
            }
            (
                vec![
                    kind.noun().into(),
                    "comment".into(),
                    number.to_string(),
                    repo,
                    body_from_stdin(),
                ],
                Some(body.clone()),
            )
        }
        Action::Close {
            kind,
            number,
            reason,
        } => {
            let mut list = vec![kind.noun().into(), "close".into(), number.to_string(), repo];
            if *kind == Kind::Issue {
                list.push(match reason {
                    Some(CloseReason::NotPlanned) => "--reason=not planned".into(),
                    _ => "--reason=completed".into(),
                });
            }
            (list, None)
        }
        Action::Reopen { kind, number } => (
            vec![
                kind.noun().into(),
                "reopen".into(),
                number.to_string(),
                repo,
            ],
            None,
        ),
        Action::Edit {
            kind,
            number,
            title,
            body,
        } => {
            let mut list = vec![kind.noun().into(), "edit".into(), number.to_string(), repo];
            if let Some(title) = title {
                list.push(title_arg(title)?);
            }
            if body.is_some() {
                list.push(body_from_stdin());
            }
            (list, body.clone())
        }
        Action::Labels {
            kind,
            number,
            add,
            remove,
        } => {
            let mut list = vec![kind.noun().into(), "edit".into(), number.to_string(), repo];
            list.extend(names("add-label", add));
            list.extend(names("remove-label", remove));
            (list, None)
        }
        Action::Assignees {
            kind,
            number,
            add,
            remove,
        } => {
            let mut list = vec![kind.noun().into(), "edit".into(), number.to_string(), repo];
            list.extend(names("add-assignee", add));
            list.extend(names("remove-assignee", remove));
            (list, None)
        }
        Action::Merge {
            number,
            method,
            delete_branch,
            auto,
        } => {
            let mut list = vec!["pr".into(), "merge".into(), number.to_string(), repo];
            list.push(
                match method {
                    MergeMethod::Merge => "--merge",
                    MergeMethod::Squash => "--squash",
                    MergeMethod::Rebase => "--rebase",
                }
                .into(),
            );
            if *delete_branch {
                list.push("--delete-branch".into());
            }
            if *auto {
                list.push("--auto".into());
            }
            (list, None)
        }
        Action::Review {
            number,
            event,
            body,
        } => {
            let flag = match event {
                ReviewEvent::Approve => "--approve",
                ReviewEvent::RequestChanges => "--request-changes",
                ReviewEvent::Comment => "--comment",
            };
            if *event != ReviewEvent::Approve && body.trim().is_empty() {
                return Err("Say what should change.".into());
            }
            let mut list = vec![
                "pr".into(),
                "review".into(),
                number.to_string(),
                repo,
                flag.into(),
            ];
            let input = (!body.trim().is_empty()).then(|| {
                list.push(body_from_stdin());
                body.clone()
            });
            (list, input)
        }
        Action::Ready { number, ready } => {
            let mut list = vec!["pr".into(), "ready".into(), number.to_string(), repo];
            if !ready {
                list.push("--undo".into());
            }
            (list, None)
        }
        Action::RerunFailed { run } => (
            vec![
                "run".into(),
                "rerun".into(),
                run.to_string(),
                repo,
                "--failed".into(),
            ],
            None,
        ),
        Action::CreateIssue {
            title,
            body,
            labels,
            assignees,
        } => {
            let mut list = vec![
                "issue".into(),
                "create".into(),
                repo,
                title_arg(title)?,
                body_from_stdin(),
            ];
            list.extend(names("label", labels));
            list.extend(names("assignee", assignees));
            (list, Some(body.clone()))
        }
    })
}

pub fn act(repo: &str, action: &Action) -> Result<String, String> {
    let (list, input) = plan(repo, action)?;
    run(&list, input.as_deref()).map(|out| out.trim().to_string())
}

/// Opens a pull request for `branch`, which must already be pushed.
pub fn create_pr(
    repo: &str,
    branch: &str,
    base: &str,
    title: &str,
    body: &str,
    draft: bool,
) -> Result<String, String> {
    if branch.starts_with('-') || base.starts_with('-') || base.is_empty() {
        return Err("Those branch names are not valid.".into());
    }
    let mut list = vec![
        "pr".into(),
        "create".into(),
        repo_arg(repo)?,
        format!("--head={branch}"),
        format!("--base={base}"),
        title_arg(title)?,
        "--body-file=-".into(),
    ];
    if draft {
        list.push("--draft".into());
    }
    run(&list, Some(body)).map(|url| url.trim().to_string())
}

/// Checks out pull request `number` in `dir`, which is a fresh detached worktree.
pub fn checkout(dir: &Path, repo: &str, number: u64, branch: &str) -> Result<(), String> {
    run_in(
        dir,
        &[
            "pr".into(),
            "checkout".into(),
            number.to_string(),
            repo_arg(repo)?,
            format!("--branch={branch}"),
        ],
        None,
    )
    .map(|_| ())
}

// ------------------------------------------------------------------ boards

fn graphql(query: &str, vars: &[(&str, &str)]) -> Result<Value, String> {
    let mut list = vec![
        "api".to_string(),
        "graphql".into(),
        format!("--raw-field=query={query}"),
    ];
    for (key, value) in vars {
        list.push(format!("--raw-field={key}={value}"));
    }
    let value = json_of(&run(&list, None)?)?;
    if let Some(errors) = value["errors"].as_array().filter(|e| !e.is_empty()) {
        let message = errors
            .iter()
            .filter_map(|e| e["message"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(friendly(&message));
    }
    Ok(value["data"].clone())
}

const PROJECT_SUMMARY: &str =
    "id number title url closed shortDescription updatedAt items { totalCount } owner { ... on Organization { login } ... on User { login } }";

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub owner: String,
    pub description: String,
    pub items: u64,
    pub linked: bool,
}

fn summaries(nodes: &Value, linked: bool) -> Vec<ProjectSummary> {
    nodes
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["closed"].as_bool() != Some(true))
        .filter_map(|n| {
            Some(ProjectSummary {
                id: n["id"].as_str()?.into(),
                number: n["number"].as_u64()?,
                title: n["title"].as_str()?.into(),
                url: n["url"].as_str().unwrap_or_default().into(),
                owner: n["owner"]["login"].as_str().unwrap_or_default().into(),
                description: n["shortDescription"].as_str().unwrap_or_default().into(),
                items: n["items"]["totalCount"].as_u64().unwrap_or(0),
                linked,
            })
        })
        .collect()
}

/// Boards linked to the repository first, then the owner's other open boards.
pub fn parse_projects(data: &Value) -> Vec<ProjectSummary> {
    let mut out = summaries(&data["repository"]["projectsV2"]["nodes"], true);
    for project in summaries(&data["repositoryOwner"]["projectsV2"]["nodes"], false) {
        if !out.iter().any(|p| p.id == project.id) {
            out.push(project);
        }
    }
    out
}

pub fn projects(repo: &Repo) -> Result<Vec<ProjectSummary>, String> {
    let query = format!(
        "query($owner:String!,$name:String!){{ repository(owner:$owner,name:$name){{ projectsV2(first:50,orderBy:{{field:UPDATED_AT,direction:DESC}}){{ nodes{{ {PROJECT_SUMMARY} }} }} }} repositoryOwner(login:$owner){{ ... on ProjectV2Owner {{ projectsV2(first:50,orderBy:{{field:UPDATED_AT,direction:DESC}}){{ nodes{{ {PROJECT_SUMMARY} }} }} }} }} }}"
    );
    Ok(parse_projects(&graphql(
        &query,
        &[("owner", &repo.owner), ("name", &repo.name)],
    )?))
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectField {
    pub id: String,
    pub name: String,
    pub options: Vec<Choice>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub color: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    /// `issue`, `pr` or `draft`; `redacted` when the viewer cannot see it.
    pub kind: String,
    pub number: Option<u64>,
    pub title: String,
    pub state: Option<String>,
    pub url: Option<String>,
    pub repo: Option<String>,
    pub draft_pr: bool,
    pub body: Option<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<Label>,
    /// Single-select field id → option id.
    pub values: HashMap<String, String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub title: String,
    pub url: String,
    pub fields: Vec<SelectField>,
    pub cards: Vec<Card>,
    pub truncated: bool,
}

const BOARD_QUERY: &str = "query($id:ID!,$after:String){ node(id:$id){ ... on ProjectV2 { title url fields(first:50){ nodes{ ... on ProjectV2SingleSelectField { id name options { id name color description } } } } items(first:100,after:$after){ pageInfo{ hasNextPage endCursor } nodes{ id type isArchived fieldValues(first:30){ nodes{ ... on ProjectV2ItemFieldSingleSelectValue { optionId field { ... on ProjectV2SingleSelectField { id } } } } } content{ __typename ... on DraftIssue { title body assignees(first:10){ nodes{ login } } } ... on Issue { number title state url repository{ nameWithOwner } assignees(first:10){ nodes{ login } } labels(first:10){ nodes{ name color } } } ... on PullRequest { number title state isDraft url repository{ nameWithOwner } assignees(first:10){ nodes{ login } } labels(first:10){ nodes{ name color } } } } } } } } }";

fn logins(value: &Value) -> Vec<String> {
    value["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["login"].as_str().map(str::to_string))
        .collect()
}

pub fn parse_card(item: &Value) -> Option<Card> {
    if item["isArchived"].as_bool() == Some(true) {
        return None;
    }
    let content = &item["content"];
    let kind = match content["__typename"].as_str() {
        Some("Issue") => "issue",
        Some("PullRequest") => "pr",
        Some("DraftIssue") => "draft",
        _ => "redacted",
    };
    let values = item["fieldValues"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some((
                v["field"]["id"].as_str()?.to_string(),
                v["optionId"].as_str()?.to_string(),
            ))
        })
        .collect();
    Some(Card {
        id: item["id"].as_str()?.into(),
        kind: kind.into(),
        number: content["number"].as_u64(),
        title: content["title"]
            .as_str()
            .unwrap_or("You do not have access to this item")
            .into(),
        state: content["state"].as_str().map(str::to_string),
        url: content["url"].as_str().map(str::to_string),
        repo: content["repository"]["nameWithOwner"]
            .as_str()
            .map(str::to_string),
        draft_pr: content["isDraft"].as_bool().unwrap_or(false),
        body: content["body"].as_str().map(str::to_string),
        assignees: logins(&content["assignees"]),
        labels: content["labels"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| {
                Some(Label {
                    name: l["name"].as_str()?.into(),
                    color: l["color"].as_str().unwrap_or("888888").into(),
                })
            })
            .collect(),
        values,
    })
}

pub fn parse_fields(node: &Value) -> Vec<SelectField> {
    node["fields"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            Some(SelectField {
                id: f["id"].as_str()?.into(),
                name: f["name"].as_str()?.into(),
                options: f["options"]
                    .as_array()?
                    .iter()
                    .filter_map(|o| {
                        Some(Choice {
                            id: o["id"].as_str()?.into(),
                            name: o["name"].as_str()?.into(),
                            color: o["color"].as_str().unwrap_or("GRAY").into(),
                            description: o["description"].as_str().unwrap_or_default().into(),
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Boards past this many cards show the first ones and say so.
const MAX_PAGES: usize = 5;

pub fn board(id: &str) -> Result<Board, String> {
    let mut after = String::new();
    let mut board: Option<Board> = None;
    for _ in 0..MAX_PAGES {
        let mut vars = vec![("id", id)];
        if !after.is_empty() {
            vars.push(("after", after.as_str()));
        }
        let data = graphql(BOARD_QUERY, &vars)?;
        let node = &data["node"];
        if node.is_null() {
            return Err("That board no longer exists, or you cannot see it.".into());
        }
        let current = board.get_or_insert_with(|| Board {
            id: id.into(),
            title: node["title"].as_str().unwrap_or_default().into(),
            url: node["url"].as_str().unwrap_or_default().into(),
            fields: parse_fields(node),
            cards: Vec::new(),
            truncated: false,
        });
        let items = &node["items"];
        current.cards.extend(
            items["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(parse_card),
        );
        let more = items["pageInfo"]["hasNextPage"].as_bool() == Some(true);
        current.truncated = more;
        match items["pageInfo"]["endCursor"].as_str() {
            Some(cursor) if more => after = cursor.to_string(),
            _ => break,
        }
    }
    board.ok_or_else(|| "That board could not be loaded.".into())
}

/// Everything the window can change on a board.
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum BoardAction {
    Move {
        item: String,
        field: String,
        option: Option<String>,
    },
    AddDraft {
        title: String,
        body: String,
    },
    AddExisting {
        kind: Kind,
        number: u64,
    },
    Archive {
        item: String,
    },
    Link,
    Unlink,
}

fn node_id(id: &str) -> Result<&str, String> {
    if !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '='))
    {
        Ok(id)
    } else {
        Err("That board item is not valid.".into())
    }
}

/// Applies one change and returns the id of the card it touched, if any.
pub fn board_act(
    project: &str,
    repo: &Repo,
    action: &BoardAction,
) -> Result<Option<String>, String> {
    let project = node_id(project)?;
    match action {
        BoardAction::Move {
            item,
            field,
            option: Some(option),
        } => {
            graphql(
                "mutation($p:ID!,$i:ID!,$f:ID!,$o:String!){ updateProjectV2ItemFieldValue(input:{projectId:$p,itemId:$i,fieldId:$f,value:{singleSelectOptionId:$o}}){ projectV2Item{ id } } }",
                &[("p", project), ("i", node_id(item)?), ("f", node_id(field)?), ("o", node_id(option)?)],
            )?;
            Ok(Some(item.clone()))
        }
        BoardAction::Move {
            item,
            field,
            option: None,
        } => {
            graphql(
                "mutation($p:ID!,$i:ID!,$f:ID!){ clearProjectV2ItemFieldValue(input:{projectId:$p,itemId:$i,fieldId:$f}){ projectV2Item{ id } } }",
                &[("p", project), ("i", node_id(item)?), ("f", node_id(field)?)],
            )?;
            Ok(Some(item.clone()))
        }
        BoardAction::AddDraft { title, body } => {
            if title.trim().is_empty() {
                return Err("Give the card a title.".into());
            }
            let data = graphql(
                "mutation($p:ID!,$t:String!,$b:String){ addProjectV2DraftIssue(input:{projectId:$p,title:$t,body:$b}){ projectItem{ id } } }",
                &[("p", project), ("t", title.trim()), ("b", body)],
            )?;
            Ok(data["addProjectV2DraftIssue"]["projectItem"]["id"]
                .as_str()
                .map(str::to_string))
        }
        BoardAction::AddExisting { kind, number } => {
            let content = json_of(&run(
                &[
                    kind.noun().into(),
                    "view".into(),
                    number.to_string(),
                    repo_arg(&repo.name_with_owner)?,
                    "--json=id".into(),
                ],
                None,
            )?)?;
            let content = content["id"].as_str().ok_or("That number was not found.")?;
            let data = graphql(
                "mutation($p:ID!,$c:ID!){ addProjectV2ItemById(input:{projectId:$p,contentId:$c}){ item{ id } } }",
                &[("p", project), ("c", node_id(content)?)],
            )?;
            Ok(data["addProjectV2ItemById"]["item"]["id"]
                .as_str()
                .map(str::to_string))
        }
        BoardAction::Archive { item } => {
            graphql(
                "mutation($p:ID!,$i:ID!){ archiveProjectV2Item(input:{projectId:$p,itemId:$i}){ item{ id } } }",
                &[("p", project), ("i", node_id(item)?)],
            )?;
            Ok(None)
        }
        BoardAction::Link | BoardAction::Unlink => {
            let verb = if matches!(action, BoardAction::Link) {
                "linkProjectV2ToRepository"
            } else {
                "unlinkProjectV2FromRepository"
            };
            graphql(
                &format!("mutation($p:ID!,$r:ID!){{ {verb}(input:{{projectId:$p,repositoryId:$r}}){{ clientMutationId }} }}"),
                &[("p", project), ("r", node_id(&repo.id)?)],
            )?;
            Ok(None)
        }
    }
}

// ------------------------------------------------------------ new PR draft

/// What a new pull request from the current branch would say, from its commits.
#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrDraft {
    pub branch: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub commits: usize,
    pub pushed: bool,
    pub remote: String,
}

/// Title from a single commit's subject, or the branch name; body from the
/// commit messages. `log` is `%s%x1f%b%x1e` records, newest first.
pub fn draft_from_log(branch: &str, log: &str) -> (String, String) {
    let commits: Vec<(&str, &str)> = log
        .split('\x1e')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .filter_map(|r| r.split_once('\x1f'))
        .collect();
    let without_trailers = |body: &str| {
        body.lines()
            .filter(|l| !l.starts_with("Co-Authored-By:") && !l.starts_with("Signed-off-by:"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    };
    match commits.as_slice() {
        [(subject, body)] => (subject.trim().to_string(), without_trailers(body)),
        [] => (humanize(branch), String::new()),
        many => (
            humanize(branch),
            many.iter()
                .rev()
                .map(|(s, _)| format!("- {}", s.trim()))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

fn humanize(branch: &str) -> String {
    let last = branch.rsplit('/').next().unwrap_or(branch);
    let words = last.replace(['-', '_'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => branch.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status_gives_the_active_account_and_its_scopes() {
        let text = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"octo","scopes":"gist, project, repo"}]}}"#;
        let (login, scopes) = parse_auth(text).unwrap();
        assert_eq!(login, "octo");
        assert_eq!(scopes, ["gist", "project", "repo"]);
        assert!(parse_auth(r#"{"hosts":{}}"#).is_none());
        assert!(parse_auth(
            r#"{"hosts":{"github.com":[{"state":"error","active":true,"login":"x"}]}}"#
        )
        .is_none());
    }

    #[test]
    fn repository_names_cannot_smuggle_flags() {
        assert_eq!(
            repo_arg("octo/hello.world").unwrap(),
            "--repo=octo/hello.world"
        );
        assert!(repo_arg("--hostname=evil/x").is_err());
        assert!(repo_arg("octo").is_err());
        assert!(repo_arg("a/b/c").is_err());
        assert!(repo_arg("octo/x y").is_err());
    }

    #[test]
    fn written_text_travels_on_stdin_and_titles_stay_one_argument() {
        let comment = Action::Comment {
            kind: Kind::Issue,
            number: 7,
            body: "--delete-branch\nhi".into(),
        };
        let (list, input) = plan("octo/app", &comment).unwrap();
        assert_eq!(
            list,
            ["issue", "comment", "7", "--repo=octo/app", "--body-file=-"]
        );
        assert_eq!(input.as_deref(), Some("--delete-branch\nhi"));

        let create = Action::CreateIssue {
            title: "-x\nsecond".into(),
            body: String::new(),
            labels: vec!["bug".into(), " ".into()],
            assignees: vec!["@me".into()],
        };
        let (list, _) = plan("octo/app", &create).unwrap();
        assert!(list.contains(&"--title=-x second".to_string()));
        assert!(list.contains(&"--label=bug".to_string()));
        assert!(list.contains(&"--assignee=@me".to_string()));
        assert_eq!(list.iter().filter(|a| a.starts_with("--label")).count(), 1);
    }

    #[test]
    fn actions_deserialize_from_the_window_and_map_to_fixed_commands() {
        let merge: Action = serde_json::from_value(json!({
            "action": "merge", "number": 3, "method": "squash", "deleteBranch": true, "auto": false
        }))
        .unwrap();
        let (list, _) = plan("o/r", &merge).unwrap();
        assert_eq!(
            list,
            [
                "pr",
                "merge",
                "3",
                "--repo=o/r",
                "--squash",
                "--delete-branch"
            ]
        );

        let close: Action = serde_json::from_value(json!({
            "action": "close", "kind": "issue", "number": 9, "reason": "not_planned"
        }))
        .unwrap();
        assert!(plan("o/r", &close)
            .unwrap()
            .0
            .contains(&"--reason=not planned".to_string()));

        let review: Action = serde_json::from_value(json!({
            "action": "review", "number": 2, "event": "request_changes", "body": ""
        }))
        .unwrap();
        assert!(
            plan("o/r", &review).is_err(),
            "asking for changes needs words"
        );

        let rerun: Action =
            serde_json::from_value(json!({ "action": "rerunFailed", "run": 42 })).unwrap();
        assert_eq!(
            plan("o/r", &rerun).unwrap().0,
            ["run", "rerun", "42", "--repo=o/r", "--failed"]
        );

        let unknown = serde_json::from_value::<Action>(json!({ "action": "api", "path": "x" }));
        assert!(unknown.is_err());
    }

    #[test]
    fn projects_list_linked_boards_first_without_duplicates_or_closed_ones() {
        let node = |id: &str, closed: bool| json!({ "id": id, "number": 1, "title": id, "url": "", "closed": closed, "items": { "totalCount": 2 }, "owner": { "login": "octo" } });
        let data = json!({
            "repository": { "projectsV2": { "nodes": [node("A", false)] } },
            "repositoryOwner": { "projectsV2": { "nodes": [node("B", false), node("A", false), node("C", true)] } },
        });
        let found = parse_projects(&data);
        assert_eq!(
            found
                .iter()
                .map(|p| (p.id.as_str(), p.linked))
                .collect::<Vec<_>>(),
            [("A", true), ("B", false)]
        );
    }

    #[test]
    fn board_cards_read_every_kind_of_item() {
        let issue = json!({
            "id": "PVTI_1", "type": "ISSUE", "isArchived": false,
            "fieldValues": { "nodes": [ {}, { "optionId": "opt1", "field": { "id": "F1" } } ] },
            "content": { "__typename": "Issue", "number": 4, "title": "Fix it", "state": "OPEN", "url": "u",
                "repository": { "nameWithOwner": "o/r" }, "assignees": { "nodes": [{ "login": "a" }] },
                "labels": { "nodes": [{ "name": "bug", "color": "d73a4a" }] } }
        });
        let card = parse_card(&issue).unwrap();
        assert_eq!(card.kind, "issue");
        assert_eq!(card.number, Some(4));
        assert_eq!(card.values.get("F1").map(String::as_str), Some("opt1"));
        assert_eq!(card.labels[0].name, "bug");

        let draft = json!({ "id": "PVTI_2", "isArchived": false, "fieldValues": { "nodes": [] },
            "content": { "__typename": "DraftIssue", "title": "Idea", "body": "b", "assignees": { "nodes": [] } } });
        assert_eq!(parse_card(&draft).unwrap().kind, "draft");

        let hidden = json!({ "id": "PVTI_3", "isArchived": false, "fieldValues": { "nodes": [] }, "content": null });
        assert_eq!(parse_card(&hidden).unwrap().kind, "redacted");

        let archived = json!({ "id": "PVTI_4", "isArchived": true, "content": null });
        assert!(parse_card(&archived).is_none());
    }

    #[test]
    fn only_single_select_fields_become_board_columns() {
        let node = json!({ "fields": { "nodes": [
            {},
            { "id": "F1", "name": "Status", "options": [{ "id": "o1", "name": "Todo", "color": "GREEN", "description": "" }] },
        ] } });
        let fields = parse_fields(&node);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].options[0].name, "Todo");
    }

    #[test]
    fn board_ids_are_checked_before_they_reach_a_query() {
        assert!(node_id("PVTI_lADOEdg6ps4Bi-HF").is_ok());
        assert!(node_id("x\" } mutation {").is_err());
        assert!(node_id("").is_err());
    }

    #[test]
    fn a_new_pull_request_is_drafted_from_its_commits() {
        let one = "Add the thing\x1fBecause reasons.\n\nCo-Authored-By: Someone <a@b>\x1e";
        assert_eq!(
            draft_from_log("feat/x", one),
            ("Add the thing".into(), "Because reasons.".into())
        );
        let two = "Second\x1f\x1eFirst\x1f\x1e";
        assert_eq!(
            draft_from_log("feat/github-view", two),
            ("Github view".into(), "- First\n- Second".into())
        );
        assert_eq!(draft_from_log("fix_it", "").0, "Fix it");
    }

    #[test]
    fn gh_messages_say_what_to_do_next() {
        assert!(
            friendly("To get started with GitHub CLI, please run:  gh auth login")
                .contains("Sign in")
        );
        assert!(
            friendly("GraphQL: Your token has not been granted the required scopes")
                .contains("board access")
        );
        assert!(friendly(
            "none of the git remotes configured for this repository point to a known GitHub host"
        )
        .contains("no GitHub remote"));
    }
}
