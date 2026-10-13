use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PR_LIST_FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,reviewDecision,statusCheckRollup,additions,deletions,updatedAt,createdAt,labels,comments,url,assignees";
pub const PR_FIELDS: &str = "number,title,body,state,isDraft,author,createdAt,updatedAt,url,headRefName,baseRefName,headRefOid,isCrossRepository,headRepositoryOwner,mergeable,mergeStateStatus,reviewDecision,statusCheckRollup,additions,deletions,changedFiles,commits,comments,reviews,reviewRequests,latestReviews,labels,assignees,closingIssuesReferences,mergedAt,mergedBy,milestone,autoMergeRequest";
pub const ISSUE_LIST_FIELDS: &str =
    "number,title,state,stateReason,author,labels,assignees,comments,updatedAt,createdAt,url";
pub const ISSUE_FIELDS: &str = "number,title,body,state,stateReason,author,createdAt,updatedAt,closedAt,url,comments,labels,assignees,milestone,closedByPullRequestsReferences";

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

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Pr,
    Issue,
}

impl Kind {
    pub fn noun(self) -> &'static str {
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
