//! GitHub access for git-viewme: sign-in, the saved client ID, token storage, requests, and the
//! domain types the interface receives. Nothing here depends on Tauri, so a hosted server can call
//! the same code.

mod auth;
mod board;
mod error;
mod files;
mod github;
mod issues;
mod lists;
mod pulls;
mod repos;
mod settings;
mod token;

pub use auth::DevicePrompt;
pub use board::{board_from_pages, card_move_request};
pub use error::{CoreError, error_for_graphql, error_for_status};
pub use files::{file_content_from_response, tree_from_response};
pub use github::GitHub;
pub use issues::{issue_changes_from_pages, issue_counts_from_response, issues_from_pages};
pub use lists::{IssueCache, ListCache, Listed, PullRequestCache};
pub use pulls::{pull_request_changes_from_pages, pull_requests_from_pages};
pub use repos::repos_from_pages;
pub use settings::{ClientId, client_id, save_client_id};
pub use token::{AccessToken, delete_token, is_signed_in, store_token};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Repo {
    pub r#ref: RepoRef,
    /// `None` for an empty repository.
    pub default_branch: Option<String>,
    pub html_url: String,
    pub private: bool,
    pub linked_projects: Vec<ProjectRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub owner: String,
    pub number: u32,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeEntryKind {
    File,
    Dir,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TreeEntry {
    pub path: String,
    pub kind: TreeEntryKind,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileContent {
    Text { text: String },
    Binary { size: u64 },
    TooLarge { size: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Board {
    /// The project's node ID, which a move names.
    pub project_id: String,
    /// The node ID of the `Status` single-select field. `None` when the project has none, and
    /// then no card can be moved.
    pub status_field_id: Option<String>,
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Column {
    pub name: String,
    /// The `Status` option the column shows. `None` for "No status".
    pub option_id: Option<String>,
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    /// The project item's node ID, unique on the board.
    pub item_id: String,
    pub title: String,
    pub number: Option<u32>,
    pub url: String,
    pub labels: Vec<String>,
}

/// Sets one project item's `Status` to an option. Nothing clears it, so no card moves to "No status".
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CardMove {
    pub project_id: String,
    pub item_id: String,
    pub field_id: String,
    pub option_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
}

impl IssueState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    /// The author's login.
    pub author: String,
    pub labels: Vec<String>,
    /// ISO 8601 in UTC, as GitHub sends it, so it sorts as text.
    pub updated_at: String,
    pub html_url: String,
}

/// The `state` GitHub filters a pull request list by, and the state each pull request reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    Open,
    Closed,
}

impl PullRequestState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

/// What the list shows for a pull request. A draft is open, and a merged pull request is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestStatus {
    Open,
    Draft,
    Merged,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    /// The author's login.
    pub author: String,
    pub labels: Vec<String>,
    /// The branch the changes come from.
    pub head: String,
    /// The branch they merge into.
    pub base: String,
    pub status: PullRequestStatus,
    /// ISO 8601 in UTC, as GitHub sends it, so it sorts as text.
    pub updated_at: String,
    pub html_url: String,
}
