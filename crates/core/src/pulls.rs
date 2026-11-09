use serde::Deserialize;
use serde_json::json;

use crate::lists::PullRequestCache;
use crate::{CoreError, GitHub, PullRequest, PullRequestState, PullRequestStatus, RepoRef};

/// One page of a state's pull requests, newest update first. GitHub's cursor holds the last
/// row's `updatedAt` and ID, so a pull request that leaves the state or moves to the top while
/// the pages load moves no other row past a page boundary. The REST list pages by position, and
/// a merge on a page already read there makes the next page's first row go unread.
const QUERY: &str = r"
query($owner: String!, $name: String!, $states: [PullRequestState!], $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: $states, orderBy: { field: UPDATED_AT, direction: DESC }, first: 100, after: $after) {
      pageInfo { hasNextPage endCursor }
      nodes {
        number
        title
        author { login }
        labels(first: 100) { nodes { name } }
        headRefName
        baseRefName
        state
        isDraft
        updatedAt
        url
      }
    }
  }
}
";

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
        if cache.list(state).is_none() {
            let states = match state {
                PullRequestState::Open => json!(["OPEN"]),
                PullRequestState::Closed => json!(["CLOSED", "MERGED"]),
            };
            let pages = self
                .graphql_pages(
                    QUERY,
                    json!({ "owner": repo.owner, "name": repo.name, "states": states }),
                    "/data/repository/pullRequests/pageInfo",
                )
                .await?;
            let pulls = pull_requests_from_graphql_pages(pages.bodies.iter().map(String::as_str))?;
            cache.replace(state, pulls, pages.since);
        }
        // Right after a full fetch this merges what was updated while it paged, and leaves the
        // ETag that makes the first switch free.
        self.refresh_pull_requests(repo, &mut cache).await?;
        Ok(cache.list(state).unwrap_or_default())
    }

    /// Merges the pull requests changed since the last fetch, in every state. The pulls endpoint
    /// takes no `since`, so this reads the list by last update and stops at the first page that
    /// reaches back past it. Its URL never changes, so the first page's ETag earns the free 304.
    async fn refresh_pull_requests(
        &self,
        repo: &RepoRef,
        cache: &mut PullRequestCache,
    ) -> Result<(), CoreError> {
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
        if let Some(pages) = changes {
            cache.merge(pull_request_changes_from_pages(
                pages.bodies.iter().map(String::as_str),
            )?);
            cache.advance(pages.since);
            cache.set_etag(pages.etag);
        }
        Ok(())
    }
}

/// Turns the bodies of the pull request GraphQL query's pages into pull requests, most recently
/// updated first, each number once.
pub fn pull_requests_from_graphql_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<PullRequest>, CoreError> {
    let mut pulls = Vec::new();
    for page in pages {
        let page: GraphqlPage = serde_json::from_str(page).map_err(|error| {
            CoreError::other(format!(
                "GitHub sent an unexpected pull request list: {error}"
            ))
        })?;
        pulls.extend(
            page.data
                .repository
                .pull_requests
                .nodes
                .into_iter()
                .map(|node| {
                    let status = match (node.state, node.is_draft) {
                        (GraphqlState::Merged, _) => PullRequestStatus::Merged,
                        (GraphqlState::Closed, _) => PullRequestStatus::Closed,
                        (GraphqlState::Open, true) => PullRequestStatus::Draft,
                        (GraphqlState::Open, false) => PullRequestStatus::Open,
                    };
                    let pull = PullRequest {
                        number: node.number,
                        title: node.title,
                        author: node
                            .author
                            .map_or_else(|| "ghost".into(), |user| user.login),
                        labels: node
                            .labels
                            .nodes
                            .into_iter()
                            .map(|label| label.name)
                            .collect(),
                        head: node.head_ref_name,
                        base: node.base_ref_name,
                        status,
                        updated_at: node.updated_at,
                        html_url: node.url,
                    };
                    ((), pull)
                }),
        );
    }
    Ok(newest_first(pulls)
        .into_iter()
        .map(|(_, pull)| pull)
        .collect())
}

/// Turns the bodies of a REST change request's pages, asked with `state=all`, into each pull
/// request with the state GitHub lists it under now. A merged pull request is closed.
pub fn pull_request_changes_from_pages<'a>(
    pages: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<(PullRequestState, PullRequest)>, CoreError> {
    let mut pulls = Vec::new();
    for page in pages {
        pulls.extend(parse_page(page)?.into_iter().map(|node| {
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
        }));
    }
    Ok(newest_first(pulls))
}

fn parse_page(body: &str) -> Result<Vec<PullNode>, CoreError> {
    serde_json::from_str(body).map_err(|error| {
        CoreError::other(format!(
            "GitHub sent an unexpected pull request list: {error}"
        ))
    })
}

/// A pull request updated while the pages load can come back on two of them, and the view keys
/// its rows by number. The newer copy is the one kept.
fn newest_first<S>(mut pulls: Vec<(S, PullRequest)>) -> Vec<(S, PullRequest)> {
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
struct GraphqlPage {
    data: GraphqlData,
}

#[derive(Deserialize)]
struct GraphqlData {
    repository: GraphqlRepository,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlRepository {
    pull_requests: GraphqlConnection,
}

#[derive(Deserialize)]
struct GraphqlConnection {
    nodes: Vec<GraphqlPull>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlPull {
    number: u32,
    title: String,
    author: Option<Login>,
    labels: GraphqlLabels,
    head_ref_name: String,
    base_ref_name: String,
    state: GraphqlState,
    is_draft: bool,
    updated_at: String,
    url: String,
}

#[derive(Deserialize)]
struct GraphqlLabels {
    nodes: Vec<Label>,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GraphqlState {
    Open,
    Closed,
    Merged,
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
