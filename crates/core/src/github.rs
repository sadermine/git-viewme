use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{RequestBuilder, Response, StatusCode, Url};
use serde_json::{Value, json};

use crate::lists::{IssueCache, ListCache, PullRequestCache};
use crate::token::{authorize, delete_token};
use crate::{CoreError, RepoRef, error_for_graphql, error_for_status};

const API: &str = "https://api.github.com";

/// The one HTTP client every GitHub request goes through. It reads the token from the credential
/// store for each request and deletes it when GitHub answers 401. It also holds the issue and
/// pull request lists fetched so far, so a list already seen is only asked for its changes.
pub struct GitHub {
    pub(crate) http: reqwest::Client,
    /// Moves on at every sign-in, sign-out and 401, and the cached lists are dropped when it
    /// does, so one account never sees another account's lists.
    account: AtomicU64,
    issues: Mutex<Lists<IssueCache>>,
    pulls: Mutex<Lists<PullRequestCache>>,
}

/// One kind of cached list for every repository, for one account. Each repository's cache has its
/// own lock, held across its fetch, so two tabs never fetch the same list at once while other
/// repositories load freely.
struct Lists<C> {
    account: u64,
    repos: HashMap<RepoRef, Arc<tokio::sync::Mutex<C>>>,
}

impl<C> Default for Lists<C> {
    fn default() -> Self {
        Self {
            account: 0,
            repos: HashMap::new(),
        }
    }
}

/// The pages of a list, with what a later conditional request needs.
pub(crate) struct Pages {
    pub(crate) bodies: Vec<String>,
    /// The first page's `ETag`. GraphQL sends none.
    pub(crate) etag: Option<String>,
    /// One minute before the first page's `Date`, as ISO 8601: GitHub's own clock, never the
    /// local one, with a margin, because asking for changes from too early only repeats some.
    pub(crate) since: String,
}

impl Default for GitHub {
    fn default() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("git-viewme/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("the HTTP client builds with static settings");
        Self {
            http,
            account: AtomicU64::new(0),
            issues: Mutex::default(),
            pulls: Mutex::default(),
        }
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl GitHub {
    /// Deletes the stored token and drops the cached lists.
    pub fn sign_out(&self) -> Result<(), CoreError> {
        self.account_changed();
        delete_token()
    }

    pub(crate) fn account_changed(&self) {
        self.account.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn issue_cache(&self, repo: &RepoRef) -> Arc<tokio::sync::Mutex<IssueCache>> {
        self.cache(&self.issues, repo)
    }

    pub(crate) fn pull_request_cache(
        &self,
        repo: &RepoRef,
    ) -> Arc<tokio::sync::Mutex<PullRequestCache>> {
        self.cache(&self.pulls, repo)
    }

    /// A fetch still running when the account changes writes to a cache already dropped here.
    fn cache<S, T>(
        &self,
        lists: &Mutex<Lists<ListCache<S, T>>>,
        repo: &RepoRef,
    ) -> Arc<tokio::sync::Mutex<ListCache<S, T>>> {
        let mut lists = lists.lock().unwrap_or_else(PoisonError::into_inner);
        let account = self.account.load(Ordering::SeqCst);
        if lists.account != account {
            lists.repos.clear();
            lists.account = account;
        }
        Arc::clone(lists.repos.entry(repo.clone()).or_default())
    }

    async fn send(&self, request: RequestBuilder) -> Result<Response, CoreError> {
        let request = request
            .header("X-GitHub-Api-Version", "2022-11-28")
            .build()?;
        let response = self.http.execute(authorize(request)?).await?;
        let status = response.status();
        if is_answer(status) {
            return Ok(response);
        }
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        let error = error_for_status(status.as_u16(), &headers, now(), &body);
        if error == CoreError::Unauthorized {
            self.account_changed();
            delete_token()?;
        }
        Err(error)
    }

    pub(crate) async fn get(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<String, CoreError> {
        Ok(self
            .send_get(api_url(segments, query))
            .await?
            .text()
            .await?)
    }

    /// Fetches every page of a REST list by following the `next` link in each `Link` header.
    /// GitHub refuses a `page` number past 99 on large lists, so offset paging cannot reach the
    /// end of them.
    pub(crate) async fn get_pages(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<Pages, CoreError> {
        self.get_changed_pages(segments, query, None, |_| Ok(true))
            .await?
            .ok_or_else(|| CoreError::other("GitHub answered 304 to a request with no ETag"))
    }

    /// Like `get_pages`, but sends `etag` as `If-None-Match` and returns `None` when GitHub
    /// answers `304 Not Modified`, which does not count against the REST rate limit. `more` reads
    /// each page and says whether to fetch the next.
    pub(crate) async fn get_changed_pages(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
        etag: Option<&str>,
        mut more: impl FnMut(&str) -> Result<bool, CoreError>,
    ) -> Result<Option<Pages>, CoreError> {
        let mut request = self.get_request(api_url(segments, query));
        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }
        let mut response = self.send(request).await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        let etag = header(&response, "etag")?;
        let date = header(&response, "date")?
            .ok_or_else(|| CoreError::other("GitHub sent no Date header"))?;
        let since = since_from_date(&date)?;
        let mut bodies = Vec::new();
        loop {
            let next = match header(&response, "link")? {
                Some(value) => next_link(&value)?,
                None => None,
            };
            let body = response.text().await?;
            let go_on = more(&body)?;
            bodies.push(body);
            match next {
                Some(next) if go_on => response = self.send_get(next).await?,
                _ => {
                    return Ok(Some(Pages {
                        bodies,
                        etag,
                        since,
                    }));
                }
            }
        }
    }

    async fn send_get(&self, url: Url) -> Result<Response, CoreError> {
        self.send(self.get_request(url)).await
    }

    fn get_request(&self, url: Url) -> RequestBuilder {
        self.http
            .get(url)
            .header("Accept", "application/vnd.github+json")
    }

    /// Runs a GraphQL query or mutation and returns the response body. GitHub reports most
    /// GraphQL failures with status 200 and an `errors` list, so those are mapped here.
    pub(crate) async fn graphql(&self, query: &str, variables: Value) -> Result<String, CoreError> {
        Ok(self.graphql_answer(query, variables).await?.0)
    }

    /// Like `graphql`, with the response's `Date` header.
    async fn graphql_answer(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<(String, Option<String>), CoreError> {
        let request = self
            .http
            .post(format!("{API}/graphql"))
            .json(&json!({ "query": query, "variables": variables }));
        let response = self.send(request).await?;
        let reset_at = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|value| value.to_str().ok()?.parse().ok());
        let date = header(&response, "date")?;
        let body = response.text().await?;
        error_for_graphql(&body, reset_at, now())?;
        Ok((body, date))
    }

    /// Fetches every page of a paged GraphQL connection. `page_info` points at the connection's
    /// `pageInfo` inside `data`.
    pub(crate) async fn graphql_pages(
        &self,
        query: &str,
        mut variables: Value,
        page_info: &str,
    ) -> Result<Pages, CoreError> {
        let mut bodies = Vec::new();
        let mut since = String::new();
        loop {
            let (body, date) = self.graphql_answer(query, variables.clone()).await?;
            if bodies.is_empty() {
                let date = date.ok_or_else(|| CoreError::other("GitHub sent no Date header"))?;
                since = since_from_date(&date)?;
            }
            let info: Value = serde_json::from_str(&body).map_err(|error| {
                CoreError::other(format!("GitHub sent an unreadable response: {error}"))
            })?;
            let info = info.pointer(page_info).cloned().unwrap_or(Value::Null);
            bodies.push(body);
            match (info["hasNextPage"].as_bool(), info["endCursor"].as_str()) {
                (Some(true), Some(cursor)) => variables["after"] = json!(cursor),
                _ => {
                    return Ok(Pages {
                        bodies,
                        etag: None,
                        since,
                    });
                }
            }
        }
    }
}

fn header(response: &Response, name: &str) -> Result<Option<String>, CoreError> {
    response
        .headers()
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| CoreError::other(format!("GitHub sent an unreadable {name} header")))
        })
        .transpose()
}

/// Whether `send` hands the response back rather than turning it into an error. A `304` answers
/// a conditional request: the list has not changed since the ETag it sent.
fn is_answer(status: StatusCode) -> bool {
    status.is_success() || status == StatusCode::NOT_MODIFIED
}

/// One minute before an HTTP `Date`, as the ISO 8601 that `since` takes.
fn since_from_date(date: &str) -> Result<String, CoreError> {
    let time = httpdate::parse_http_date(date)
        .map_err(|_| CoreError::other(format!("GitHub sent an unreadable Date header: {date}")))?;
    let time = time
        .checked_sub(Duration::from_secs(60))
        .unwrap_or(UNIX_EPOCH);
    Ok(humantime::format_rfc3339_seconds(time).to_string())
}

fn api_url(segments: &[&str], query: &[(&str, &str)]) -> Url {
    let mut url = Url::parse(API).expect("the API base URL parses");
    url.path_segments_mut()
        .expect("the API base URL has a path")
        .extend(segments);
    url.query_pairs_mut().extend_pairs(query);
    url
}

/// The `rel="next"` URL of a `Link` header. A next link that leaves the API is refused rather than
/// followed.
fn next_link(header: &str) -> Result<Option<Url>, CoreError> {
    let Some(target) = header.split(',').find_map(|link| {
        let (target, params) = link.split_once(';')?;
        params
            .split(';')
            .any(|param| param.trim() == r#"rel="next""#)
            .then(|| target.trim().trim_start_matches('<').trim_end_matches('>'))
    }) else {
        return Ok(None);
    };
    let api = Url::parse(API).expect("the API base URL parses");
    match Url::parse(target) {
        Ok(url) if url.origin() == api.origin() => Ok(Some(url)),
        _ => Err(CoreError::other(format!(
            "GitHub sent a next page link outside {API}: {target}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{GitHub, is_answer, next_link, since_from_date};
    use crate::{IssueState, RepoRef};

    #[test]
    fn cached_lists_are_dropped_when_the_account_changes() {
        let github = GitHub::default();
        let repo = RepoRef {
            owner: "sadermine".into(),
            name: "git-viewme".into(),
        };
        let held = github.issue_cache(&repo);
        held.try_lock()
            .unwrap()
            .replace(IssueState::Open, vec![], "2026-11-08T12:00:00Z".into());
        assert!(
            github
                .issue_cache(&repo)
                .try_lock()
                .unwrap()
                .list(IssueState::Open)
                .is_some()
        );

        github.account_changed();

        assert!(
            github
                .issue_cache(&repo)
                .try_lock()
                .unwrap()
                .list(IssueState::Open)
                .is_none()
        );
    }

    #[test]
    fn is_answer_takes_304_not_modified_as_an_answer_not_an_error() {
        assert!(is_answer(StatusCode::NOT_MODIFIED));
        assert!(is_answer(StatusCode::OK));
        for status in [
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ] {
            assert!(!is_answer(status), "{status} answered");
        }
    }

    #[test]
    fn since_from_date_is_one_minute_before_github_date() {
        assert_eq!(
            since_from_date("Sun, 08 Nov 2026 13:20:05 GMT").unwrap(),
            "2026-11-08T13:19:05Z"
        );
    }

    #[test]
    fn since_from_date_refuses_an_unreadable_date() {
        assert!(since_from_date("yesterday").is_err());
    }

    #[test]
    fn next_link_follows_the_cursor_in_a_link_header() {
        let header = r#"<https://api.github.com/repositories/1/issues?per_page=100&after=Y3Vyc29y>; rel="next", <https://api.github.com/repositories/1/issues?per_page=100&before=Y3Vyc29y>; rel="prev""#;
        let next = next_link(header).unwrap().unwrap();
        assert_eq!(
            next.as_str(),
            "https://api.github.com/repositories/1/issues?per_page=100&after=Y3Vyc29y"
        );
    }

    #[test]
    fn next_link_is_none_on_the_last_page() {
        let header = r#"<https://api.github.com/repositories/1/issues?page=1>; rel="first", <https://api.github.com/repositories/1/issues?before=Y3Vyc29y>; rel="prev""#;
        assert_eq!(next_link(header).unwrap(), None);
    }

    #[test]
    fn next_link_refuses_a_link_outside_the_api() {
        for target in [
            "https://evil.example/issues?after=x",
            "http://api.github.com/issues?after=x",
            "https://api.github.com.evil.example/issues?after=x",
            "https://api.github.com:8443/issues?after=x",
        ] {
            let header = format!(r#"<{target}>; rel="next""#);
            assert!(next_link(&header).is_err(), "followed {target}");
        }
    }
}
