pub mod board;
pub mod client;
pub mod models;

pub use board::*;
pub use client::*;
pub use models::*;

use serde_json::{json, Value};
use std::path::Path;

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
