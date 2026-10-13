use super::{
    client::{friendly, json_of, repo_arg, run},
    models::{Board, BoardAction, Card, Choice, Label, ProjectSummary, Repo, SelectField},
};
use serde_json::Value;

pub fn graphql(query: &str, vars: &[(&str, &str)]) -> Result<Value, String> {
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

pub fn node_id(id: &str) -> Result<&str, String> {
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
