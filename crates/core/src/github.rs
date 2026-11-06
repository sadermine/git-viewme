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
        let mut url = Url::parse(API).expect("the API base URL parses");
        url.path_segments_mut()
            .expect("the API base URL has a path")
            .extend(segments);
        let request = self
            .http
            .get(url)
            .query(query)
            .header("Accept", "application/vnd.github+json");
        Ok(self.send(request).await?.text().await?)
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
