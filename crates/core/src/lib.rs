//! GitHub access for git-viewme: sign-in, token storage, requests, and the domain types the
//! interface receives. Nothing here depends on Tauri, so a hosted server can call the same code.

mod auth;
mod board;
mod error;
mod files;
mod github;
mod repos;
mod token;

pub use auth::{CLIENT_ID, DevicePrompt};
pub use board::board_from_pages;
pub use error::{CoreError, error_for_status};
pub use files::{file_content_from_response, tree_from_response};
pub use github::GitHub;
pub use repos::repos_from_pages;
pub use token::{AccessToken, delete_token, is_signed_in, store_token};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Column {
    pub name: String,
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    pub title: String,
    pub number: Option<u32>,
    pub url: String,
    pub labels: Vec<String>,
}
