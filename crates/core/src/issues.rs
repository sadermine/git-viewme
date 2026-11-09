use serde::Deserialize;

use crate::lists::IssueCache;
use crate::{CoreError, GitHub, Issue, IssueState, RepoRef};

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
        if cache.list(state).is_none() {
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
            cache.replace(state, issues, pages.since);
        }
        // Right after a full fetch this merges what moved while it paged, and leaves the ETag
        // that makes the first switch free.
        self.merge_issue_changes(repo, &mut cache).await?;
        Ok(cache.list(state).unwrap_or_default())
    }

    /// Merges the issues changed since the last fetch, in every state, so one closed since leaves
    /// Open and joins Closed. A deleted or transferred issue is never reported as changed, so it
    /// stays listed until the app restarts (#35).
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
