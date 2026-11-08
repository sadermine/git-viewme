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
        let mut fetched = Vec::new();
        if cache.list(state).is_none() {
            let (issues, since) = self.fetch_issues(repo, state).await?;
            cache.replace(state, issues, since);
            fetched.push(state);
        }
        self.refresh_issues(repo, &mut cache, fetched).await?;
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
    /// `fetched` names the states just fetched in full. A full fetch misses issues that moved
    /// while it paged, so each one's count is compared only after a change request has merged
    /// them, and then recorded rather than checked.
    async fn refresh_issues(
        &self,
        repo: &RepoRef,
        cache: &mut IssueCache,
        mut fetched: Vec<IssueState>,
    ) -> Result<(), CoreError> {
        self.merge_issue_changes(repo, cache).await?;
        let mut counts = self.issue_counts(repo).await?;
        let differing: Vec<IssueState> = counts
            .iter()
            .filter(|(state, count)| {
                !fetched.contains(state) && cache.count_differs(*state, *count)
            })
            .map(|(state, _)| *state)
            .collect();
        if !differing.is_empty() {
            for state in differing {
                let (issues, since) = self.fetch_issues(repo, state).await?;
                cache.replace(state, issues, since);
                fetched.push(state);
            }
            self.merge_issue_changes(repo, cache).await?;
            counts = self.issue_counts(repo).await?;
        }
        for (state, count) in counts {
            if fetched.contains(&state) {
                cache.settle(state, count);
            }
        }
        Ok(())
    }

    async fn issue_counts(&self, repo: &RepoRef) -> Result<[(IssueState, usize); 2], CoreError> {
        let body = self
            .graphql(
                ISSUE_COUNTS,
                json!({ "owner": repo.owner, "name": repo.name }),
            )
            .await?;
        issue_counts_from_response(&body)
    }

    async fn merge_issue_changes(
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
            cache.merge(issue_changes_from_pages(
                pages.bodies.iter().map(String::as_str),
            )?);
            cache.answered(pages.bodies.len(), pages.since, pages.etag);
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
