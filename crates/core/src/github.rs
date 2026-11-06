use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::{RequestBuilder, Response, Url};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::token::{delete_token, load_token};
use crate::{CoreError, error_for_status};

const API: &str = "https://api.github.com";

/// The one HTTP client every GitHub request goes through. It reads the token from the credential
/// store for each request and deletes it when GitHub answers 401.
pub struct GitHub {
    pub(crate) http: reqwest::Client,
}

impl Default for GitHub {
    fn default() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("git-viewme/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("the HTTP client builds with static settings");
        Self { http }
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl GitHub {
    async fn send(&self, request: RequestBuilder) -> Result<Response, CoreError> {
        let token = load_token()?.ok_or(CoreError::Unauthorized)?;
        let response = request
            .bearer_auth(token.secret())
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        let error = error_for_status(status.as_u16(), &headers, now(), &body);
        if error == CoreError::Unauthorized {
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
    ) -> Result<Vec<String>, CoreError> {
        let mut url = api_url(segments, query);
        let mut pages = Vec::new();
        loop {
            let response = self.send_get(url).await?;
            let next = match response.headers().get("link") {
                Some(value) => next_link(value.to_str().unwrap_or_default())?,
                None => None,
            };
            pages.push(response.text().await?);
            match next {
                Some(next) => url = next,
                None => return Ok(pages),
            }
        }
    }

    async fn send_get(&self, url: Url) -> Result<Response, CoreError> {
        let request = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json");
        self.send(request).await
    }

    /// Runs a GraphQL query and returns the response body. GitHub reports most GraphQL failures
    /// with status 200 and an `errors` list, so those are mapped here.
    pub(crate) async fn graphql(&self, query: &str, variables: Value) -> Result<String, CoreError> {
        let request = self
            .http
            .post(format!("{API}/graphql"))
            .json(&json!({ "query": query, "variables": variables }));
        let response = self.send(request).await?;
        let reset_at = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|value| value.to_str().ok()?.parse().ok());
        let body = response.text().await?;
        let errors = serde_json::from_str::<GraphQlErrors>(&body)
            .map_err(|error| {
                CoreError::other(format!("GitHub sent an unreadable response: {error}"))
            })?
            .errors;
        match errors.first() {
            None => Ok(body),
            Some(first) => Err(match first.kind.as_deref() {
                Some("NOT_FOUND") => CoreError::NotFound,
                Some("RATE_LIMITED") => CoreError::RateLimited {
                    reset_at: reset_at.unwrap_or(now() + 60),
                },
                _ => CoreError::other(first.message.clone()),
            }),
        }
    }

    /// Fetches every page of a paged GraphQL connection. `page_info` points at the connection's
    /// `pageInfo` inside `data`.
    pub(crate) async fn graphql_pages(
        &self,
        query: &str,
        mut variables: Value,
        page_info: &str,
    ) -> Result<Vec<String>, CoreError> {
        let mut pages = Vec::new();
        loop {
            let body = self.graphql(query, variables.clone()).await?;
            let info: Value = serde_json::from_str(&body).map_err(|error| {
                CoreError::other(format!("GitHub sent an unreadable response: {error}"))
            })?;
            let info = info.pointer(page_info).cloned().unwrap_or(Value::Null);
            pages.push(body);
            match (info["hasNextPage"].as_bool(), info["endCursor"].as_str()) {
                (Some(true), Some(cursor)) => variables["after"] = json!(cursor),
                _ => return Ok(pages),
            }
        }
    }
}

fn api_url(segments: &[&str], query: &[(&str, &str)]) -> Url {
    let mut url = Url::parse(API).expect("the API base URL parses");
    url.path_segments_mut()
        .expect("the API base URL has a path")
        .extend(segments);
    url.query_pairs_mut().extend_pairs(query);
    url
}

/// The `rel="next"` URL of a `Link` header. `send` attaches the token to whatever URL it is given,
/// so a next link that leaves the API is refused rather than followed.
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

#[derive(Deserialize)]
struct GraphQlErrors {
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::next_link;

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
