use serde::Deserialize;

use crate::{CoreError, GitHub, PullRequest, PullRequestState, PullRequestStatus, RepoRef};

const PER_PAGE: usize = 100;

impl GitHub {
    /// Every pull request in the repository with the given state, most recently updated first.
    pub async fn list_pull_requests(
        &self,
        repo: &RepoRef,
        state: PullRequestState,
    ) -> Result<Vec<PullRequest>, CoreError> {
        let per_page = PER_PAGE.to_string();
        let mut nodes = Vec::new();
        for page in 1.. {
            let page = page.to_string();
            let body = self
                .get(
                    &["repos", &repo.owner, &repo.name, "pulls"],
                    &[
                        ("state", state.as_str()),
                        ("sort", "updated"),
                        ("direction", "desc"),
                        ("per_page", &per_page),
                        ("page", &page),
                    ],
                )
                .await?;
            let page_nodes = parse_page(&body)?;
            let last = page_nodes.len() < PER_PAGE;
            nodes.extend(page_nodes);
            if last {
                break;
            }
        }
        Ok(pull_requests_from_nodes(nodes))
    }
}

/// Turns the bodies of REST pull-request-list pages into pull requests, most recently updated
/// first. A pull request updated while the pages load can appear on two pages, and only its
/// latest copy is kept.
pub fn pull_requests_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<PullRequest>, CoreError> {
    let mut nodes = Vec::new();
    for page in pages {
        nodes.extend(parse_page(page)?);
    }
    Ok(pull_requests_from_nodes(nodes))
}

fn parse_page(body: &str) -> Result<Vec<PullNode>, CoreError> {
    serde_json::from_str(body).map_err(|error| {
        CoreError::other(format!(
            "GitHub sent an unexpected pull request list: {error}"
        ))
    })
}

fn pull_requests_from_nodes(nodes: Vec<PullNode>) -> Vec<PullRequest> {
    let mut pulls: Vec<PullRequest> = nodes
        .into_iter()
        .map(|node| PullRequest {
            number: node.number,
            title: node.title,
            author: node.user.map_or_else(|| "ghost".into(), |user| user.login),
            labels: node.labels.into_iter().map(|label| label.name).collect(),
            head: node.head.r#ref,
            base: node.base.r#ref,
            status: match (node.state, node.merged_at, node.draft) {
                (PullRequestState::Closed, Some(_), _) => PullRequestStatus::Merged,
                (PullRequestState::Closed, None, _) => PullRequestStatus::Closed,
                (PullRequestState::Open, _, true) => PullRequestStatus::Draft,
                (PullRequestState::Open, _, false) => PullRequestStatus::Open,
            },
            updated_at: node.updated_at,
            html_url: node.html_url,
        })
        .collect();
    pulls.sort_by(|a, b| {
        a.number
            .cmp(&b.number)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    pulls.dedup_by_key(|pull| pull.number);
    pulls.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.number.cmp(&a.number))
    });
    pulls
}

#[derive(Deserialize)]
struct PullNode {
    number: u32,
    title: String,
    user: Option<Login>,
    labels: Vec<Label>,
    head: Branch,
    base: Branch,
    state: PullRequestState,
    #[serde(default)]
    draft: bool,
    merged_at: Option<String>,
    updated_at: String,
    html_url: String,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Deserialize)]
struct Branch {
    r#ref: String,
}
