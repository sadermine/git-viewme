use serde::Deserialize;
use serde_json::json;

use crate::lists::IssueCache;
use crate::{CoreError, GitHub, Issue, IssueState, RepoRef};

const ISSUE_COUNTS: &str = "query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    open: issues(states: OPEN) { totalCount }
    closed: issues(states: CLOSED) { totalCount }
  }
}";

impl GitHub {
    /// Every issue in the repository with the given state, most recently updated first. Pull
    /// requests are left out. The first call for a state fetches every page. Later calls ask only
    /// for the issues changed since, which costs nothing against the REST limit when none did.
    pub async fn list_issues(
        &self,
        repo: &RepoRef,
        state: IssueState,
    ) -> Result<Vec<Issue>, CoreError> {
        let cache = self.issue_cache(repo);
        let mut cache = cache.lock().await;
        if cache.list(state).is_some() {
            self.refresh_issues(repo, &mut cache).await?;
        } else {
            let (issues, since) = self.fetch_issues(repo, state).await?;
            cache.replace(state, issues, since);
        }
        Ok(cache.list(state).unwrap_or_default())
    }

    async fn fetch_issues(
        &self,
        repo: &RepoRef,
        state: IssueState,
    ) -> Result<(Vec<Issue>, String), CoreError> {
        let pages = self
            .get_pages(
                &["repos", &repo.owner, &repo.name, "issues"],
                &[
                    ("state", state.as_str()),
                    ("sort", "updated"),
                    ("direction", "desc"),
                    ("per_page", "100"),
                ],
            )
            .await?;
        let issues = issues_from_pages(pages.bodies.iter().map(String::as_str))?;
        Ok((issues, pages.since))
    }

    /// Merges the issues changed since the last fetch, in every state, so one closed since leaves
    /// Open and joins Closed. A deleted or transferred issue never shows up as changed, so the
    /// counts are checked after and a state whose count differs is fetched again in full.
    async fn refresh_issues(
        &self,
        repo: &RepoRef,
        cache: &mut IssueCache,
    ) -> Result<(), CoreError> {
        let since = cache.since().unwrap_or_default().to_owned();
        let changes = self
            .get_changed_pages(
                &["repos", &repo.owner, &repo.name, "issues"],
                &[
                    ("state", "all"),
                    ("since", &since),
                    ("sort", "updated"),
                    ("direction", "desc"),
                    ("per_page", "100"),
                ],
                cache.etag(),
                |_| Ok(true),
            )
            .await?;
        if let Some(pages) = changes {
            let changes = issue_changes_from_pages(pages.bodies.iter().map(String::as_str))?;
            // `since` moves on only when something changed, so an unchanged refresh sends the
            // same URL again and its ETag earns the free 304.
            if cache.merge(changes) {
                cache.advance(pages.since);
            } else {
                cache.set_etag(pages.etag);
            }
        }
        let body = self
            .graphql(
                ISSUE_COUNTS,
                json!({ "owner": repo.owner, "name": repo.name }),
            )
            .await?;
        for (state, count) in issue_counts_from_response(&body)? {
            if cache.count_differs(state, count) {
                let (issues, since) = self.fetch_issues(repo, state).await?;
                cache.refetched(state, issues, since, count);
            }
        }
        Ok(())
    }
}

/// Turns the bodies of REST issue-list pages into issues, most recently updated first. GitHub's
/// issues endpoint returns pull requests too, and those are left out.
pub fn issues_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Issue>, CoreError> {
    Ok(issues_from_nodes(nodes_from_pages(pages)?)
        .into_iter()
        .map(|(_, issue)| issue)
        .collect())
}

/// Turns the bodies of a change request's pages, asked with `state=all`, into each changed issue
/// with its state now. Pull requests are left out.
pub fn issue_changes_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<(IssueState, Issue)>, CoreError> {
    Ok(issues_from_nodes(nodes_from_pages(pages)?))
}

/// The open and closed issue counts in a GraphQL answer to `ISSUE_COUNTS`. Pull requests are not
/// issues there, as in the REST lists once they are left out.
pub fn issue_counts_from_response(body: &str) -> Result<[(IssueState, usize); 2], CoreError> {
    let unexpected = || CoreError::other("GitHub sent unexpected issue counts");
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| unexpected())?;
    let count = |state: IssueState| {
        value
            .pointer(&format!("/data/repository/{}/totalCount", state.as_str()))
            .and_then(serde_json::Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(unexpected)
    };
    Ok([
        (IssueState::Open, count(IssueState::Open)?),
        (IssueState::Closed, count(IssueState::Closed)?),
    ])
}

fn nodes_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<IssueNode>, CoreError> {
    let mut nodes = Vec::new();
    for page in pages {
        nodes.extend(parse_page(page)?);
    }
    Ok(nodes)
}

fn parse_page(body: &str) -> Result<Vec<IssueNode>, CoreError> {
    serde_json::from_str(body)
        .map_err(|error| CoreError::other(format!("GitHub sent an unexpected issue list: {error}")))
}

fn issues_from_nodes(nodes: Vec<IssueNode>) -> Vec<(IssueState, Issue)> {
    let mut issues: Vec<(IssueState, Issue)> = nodes
        .into_iter()
        .filter(|node| node.pull_request.is_none())
        .map(|node| {
            let issue = Issue {
                number: node.number,
                title: node.title,
                author: node.user.map_or_else(|| "ghost".into(), |user| user.login),
                labels: node.labels.into_iter().map(|label| label.name).collect(),
                updated_at: node.updated_at,
                html_url: node.html_url,
            };
            (node.state, issue)
        })
        .collect();
    // An issue updated while the pages are fetched can come back on two of them, and the view
    // keys its rows by number. The newer copy is the one kept.
    issues.sort_by(|(_, a), (_, b)| {
        a.number
            .cmp(&b.number)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    issues.dedup_by_key(|(_, issue)| issue.number);
    issues.sort_by(|(_, a), (_, b)| {
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
    state: IssueState,
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
