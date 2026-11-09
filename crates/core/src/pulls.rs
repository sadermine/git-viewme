use serde::Deserialize;

use crate::lists::PullRequestCache;
use crate::{CoreError, GitHub, PullRequest, PullRequestState, PullRequestStatus, RepoRef};

impl GitHub {
    /// Every pull request in the repository with the given state, most recently updated first.
    /// The first call for a state fetches every page. Later calls read only the pull requests
    /// changed since, which costs nothing against the REST limit when none did.
    pub async fn list_pull_requests(
        &self,
        repo: &RepoRef,
        state: PullRequestState,
    ) -> Result<Vec<PullRequest>, CoreError> {
        let cache = self.pull_request_cache(repo);
        let mut cache = cache.lock().await;
        if cache.list(state).is_some() {
            self.refresh_pull_requests(repo, &mut cache).await?;
            return Ok(cache.list(state).unwrap_or_default());
        }
        // A list stored before a later step failed may have lost a row, and the cached path never
        // checks for that, so it goes and the next call fetches the state again.
        if let Err(error) = self.fetch_pull_requests(repo, state, &mut cache).await {
            cache.forget(state);
            return Err(error);
        }
        Ok(cache.list(state).unwrap_or_default())
    }

    /// Fetches `state` in full into the cache, and again while a pull request it listed has left
    /// the state since, at most `REFETCHES` more times.
    async fn fetch_pull_requests(
        &self,
        repo: &RepoRef,
        state: PullRequestState,
        cache: &mut PullRequestCache,
    ) -> Result<(), CoreError> {
        for _ in 0..=REFETCHES {
            let pages = self
                .get_pages(
                    &["repos", &repo.owner, &repo.name, "pulls"],
                    &[
                        ("state", state.as_str()),
                        ("sort", "updated"),
                        ("direction", "desc"),
                        ("per_page", "100"),
                    ],
                )
                .await?;
            let pulls = pull_requests_from_pages(pages.bodies.iter().map(String::as_str))?;
            let fetch = FullFetch {
                state,
                pages: pages.bodies.len(),
                since: pages.since.clone(),
                numbers: pulls.iter().map(|pull| pull.number).collect(),
            };
            cache.replace(state, pulls, pages.since);
            // Right after a full fetch this merges what moved while it paged, and leaves the ETag
            // that makes the first switch free.
            let changes = self.refresh_pull_requests(repo, cache).await?;
            if !pull_request_left_while_paging(&fetch, &changes) {
                break;
            }
        }
        Ok(())
    }

    /// Merges the pull requests changed since the last fetch, in every state. The pulls endpoint
    /// takes no `since`, so this reads the list by last update and stops at the first page that
    /// reaches back past it. Its URL never changes, so the first page's ETag earns the free 304.
    async fn refresh_pull_requests(
        &self,
        repo: &RepoRef,
        cache: &mut PullRequestCache,
    ) -> Result<Vec<(PullRequestState, PullRequest)>, CoreError> {
        let since = cache.since().unwrap_or_default().to_owned();
        let changes = self
            .get_changed_pages(
                &["repos", &repo.owner, &repo.name, "pulls"],
                &[
                    ("state", "all"),
                    ("sort", "updated"),
                    ("direction", "desc"),
                    ("per_page", "100"),
                ],
                cache.etag(),
                |body| {
                    Ok(parse_page(body)?
                        .iter()
                        .all(|node| node.updated_at >= since))
                },
            )
            .await?;
        let Some(pages) = changes else {
            return Ok(Vec::new());
        };
        let changes = pull_request_changes_from_pages(pages.bodies.iter().map(String::as_str))?;
        cache.merge(changes.clone());
        cache.advance(pages.since);
        cache.set_etag(pages.etag);
        Ok(changes)
    }
}

/// How many times a state is fetched in full again when a pull request left it while it paged.
const REFETCHES: usize = 3;

/// What a full fetch of one state read: how many pages, the GitHub time it started at, and the
/// pull requests it listed.
pub struct FullFetch {
    pub state: PullRequestState,
    pub pages: usize,
    pub since: String,
    pub numbers: Vec<u32>,
}

/// Whether the change answer right after a full fetch shows a pull request that fetch listed
/// leaving its state since the fetch started. The pulls endpoint pages by number, so when a pull
/// request on a page already read leaves the state, every row below it moves up one place and
/// the first row of the next page is never read. That row did not change, so no change answer
/// brings it back, and only a fresh full fetch does. One page has no next page to lose a row from.
pub fn pull_request_left_while_paging(
    fetch: &FullFetch,
    changes: &[(PullRequestState, PullRequest)],
) -> bool {
    fetch.pages > 1
        && changes.iter().any(|(state, pull)| {
            *state != fetch.state
                && pull.updated_at >= fetch.since
                && fetch.numbers.contains(&pull.number)
        })
}

/// Turns the bodies of REST pull-request-list pages into pull requests, most recently updated
/// first, each number once. When a pull request moves to the top while the pages load, the rows
/// below it shift down a place, so the last row of one page comes back on the next.
pub fn pull_requests_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<PullRequest>, CoreError> {
    Ok(pull_request_changes_from_pages(pages)?
        .into_iter()
        .map(|(_, pull)| pull)
        .collect())
}

/// Turns the bodies of a change request's pages, asked with `state=all`, into each pull request
/// with the state GitHub lists it under now. A merged pull request is closed.
pub fn pull_request_changes_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<(PullRequestState, PullRequest)>, CoreError> {
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

fn pull_requests_from_nodes(nodes: Vec<PullNode>) -> Vec<(PullRequestState, PullRequest)> {
    let mut pulls: Vec<(PullRequestState, PullRequest)> = nodes
        .into_iter()
        .map(|node| {
            let pull = PullRequest {
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
            };
            (node.state, pull)
        })
        .collect();
    pulls.sort_by(|(_, a), (_, b)| {
        a.number
            .cmp(&b.number)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    pulls.dedup_by_key(|(_, pull)| pull.number);
    pulls.sort_by(|(_, a), (_, b)| {
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
