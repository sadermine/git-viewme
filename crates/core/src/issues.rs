use serde::Deserialize;

use crate::{CoreError, GitHub, Issue, IssueState, RepoRef};

const PER_PAGE: usize = 100;

impl GitHub {
    /// Every issue in the repository with the given state, most recently updated first. Pull
    /// requests are left out.
    pub async fn list_issues(
        &self,
        repo: &RepoRef,
        state: IssueState,
    ) -> Result<Vec<Issue>, CoreError> {
        let per_page = PER_PAGE.to_string();
        let mut nodes = Vec::new();
        for page in 1.. {
            let page = page.to_string();
            let body = self
                .get(
                    &["repos", &repo.owner, &repo.name, "issues"],
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
        Ok(issues_from_nodes(nodes))
    }
}

/// Turns the bodies of REST issue-list pages into issues, most recently updated first. GitHub's
/// issues endpoint returns pull requests too, and those are left out.
pub fn issues_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Issue>, CoreError> {
    let mut nodes = Vec::new();
    for page in pages {
        nodes.extend(parse_page(page)?);
    }
    Ok(issues_from_nodes(nodes))
}

fn parse_page(body: &str) -> Result<Vec<IssueNode>, CoreError> {
    serde_json::from_str(body)
        .map_err(|error| CoreError::other(format!("GitHub sent an unexpected issue list: {error}")))
}

fn issues_from_nodes(nodes: Vec<IssueNode>) -> Vec<Issue> {
    let mut issues: Vec<Issue> = nodes
        .into_iter()
        .filter(|node| node.pull_request.is_none())
        .map(|node| Issue {
            number: node.number,
            title: node.title,
            author: node.user.map_or_else(|| "ghost".into(), |user| user.login),
            labels: node.labels.into_iter().map(|label| label.name).collect(),
            updated_at: node.updated_at,
            html_url: node.html_url,
        })
        .collect();
    // Offset paging can return an issue on two pages when another moves above it mid-fetch, and
    // the view keys its rows by number.
    issues.sort_by_key(|issue| issue.number);
    issues.dedup_by_key(|issue| issue.number);
    issues.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.number.cmp(&a.number))
    });
    issues
}

#[derive(Deserialize)]
struct IssueNode {
    number: u32,
    title: String,
    user: Option<Login>,
    labels: Vec<Label>,
    updated_at: String,
    html_url: String,
    pull_request: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}
