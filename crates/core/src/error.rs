use reqwest::header::HeaderMap;
use serde::Serialize;

/// Every failure the interface can show. Views choose their message and their Retry from the
/// variant, never from the text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreError {
    /// No stored token, or GitHub rejected it. The stored token is deleted when this is returned.
    Unauthorized,
    /// `reset_at` is in seconds since the Unix epoch.
    RateLimited {
        reset_at: u64,
    },
    Network,
    NotFound,
    Other {
        message: String,
    },
}

impl CoreError {
    pub(crate) fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}

impl From<reqwest::Error> for CoreError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_connect() || error.is_timeout() || error.is_request() {
            Self::Network
        } else {
            Self::other(error.to_string())
        }
    }
}

/// Maps a GitHub REST or GraphQL response with a non-success status to the error the interface
/// shows. `now` is seconds since the Unix epoch, used when GitHub answers with `retry-after`.
pub fn error_for_status(status: u16, headers: &HeaderMap, now: u64, body: &str) -> CoreError {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
    };
    match status {
        401 => CoreError::Unauthorized,
        403 | 429 if header("x-ratelimit-remaining") == Some(0) => CoreError::RateLimited {
            reset_at: header("x-ratelimit-reset").unwrap_or(now + 60),
        },
        // GitHub's secondary rate limit sends retry-after and leaves x-ratelimit-remaining above 0.
        403 | 429 if header("retry-after").is_some() => CoreError::RateLimited {
            reset_at: now + header("retry-after").unwrap_or(60),
        },
        404 => CoreError::NotFound,
        _ => CoreError::other(format!("GitHub answered {status}: {}", message_of(body))),
    }
}

fn message_of(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| json.get("message")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(200).collect())
}
