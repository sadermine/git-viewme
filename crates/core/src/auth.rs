use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::github::now;
use crate::token::{AccessToken, store_token};
use crate::{CoreError, GitHub};

/// The client ID of the sadermine OAuth app. It is public: the device flow sends it with no secret.
pub const CLIENT_ID: &str = "REPLACE_WITH_OAUTH_APP_CLIENT_ID";
const PLACEHOLDER_CLIENT_ID: &str = "REPLACE_WITH_OAUTH_APP_CLIENT_ID";
const SCOPES: &str = "repo read:project";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// What the user needs to finish signing in on GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

impl GitHub {
    /// Signs in with GitHub's device flow and stores the token. `on_prompt` receives the code to
    /// show; this then polls GitHub until the user approves, denies, or the code expires.
    pub async fn sign_in(&self, on_prompt: impl FnOnce(DevicePrompt)) -> Result<(), CoreError> {
        if CLIENT_ID == PLACEHOLDER_CLIENT_ID {
            return Err(CoreError::other(
                "This build has no GitHub OAuth client ID yet, so it cannot sign in. Register the \
                 OAuth app with device flow enabled and set CLIENT_ID in crates/core/src/auth.rs.",
            ));
        }
        let response = self
            .http
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[("client_id", CLIENT_ID), ("scope", SCOPES)])
            .send()
            .await?;
        let status = response.status();
        let code: DeviceCode = match response.json::<DeviceCodeReply>().await {
            Ok(DeviceCodeReply::Code(code)) if status.is_success() => code,
            Ok(DeviceCodeReply::Error(reply)) => return Err(sign_in_error(&reply)),
            _ => {
                return Err(CoreError::other(format!(
                    "GitHub refused to start sign-in ({status}). Check the client ID and that the \
                     OAuth app has device flow enabled."
                )));
            }
        };
        on_prompt(DevicePrompt {
            user_code: code.user_code,
            verification_uri: code.verification_uri,
            expires_in: code.expires_in,
        });

        let deadline = now() + code.expires_in;
        let mut interval = code.interval;
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if now() >= deadline {
                return Err(expired());
            }
            let reply = self
                .http
                .post(TOKEN_URL)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("device_code", code.device_code.as_str()),
                    ("grant_type", DEVICE_GRANT),
                ])
                .send()
                .await?
                .json::<TokenReply>()
                .await?;
            match reply {
                TokenReply::Token { access_token } => {
                    return store_token(&AccessToken::new(access_token));
                }
                TokenReply::Error(reply) => match reply.error.as_str() {
                    "authorization_pending" => {}
                    // GitHub adds 5 seconds and says so in `interval`; keep to it or the next poll is refused too.
                    "slow_down" => interval = reply.interval.unwrap_or(interval + 5),
                    _ => return Err(sign_in_error(&reply)),
                },
            }
        }
    }
}

fn expired() -> CoreError {
    CoreError::other("The sign-in code expired before it was entered. Sign in again.")
}

fn sign_in_error(reply: &ErrorReply) -> CoreError {
    match reply.error.as_str() {
        "expired_token" => expired(),
        "access_denied" => CoreError::other("Sign-in was cancelled on GitHub."),
        _ => CoreError::other(format!(
            "GitHub refused sign-in: {}",
            reply.error_description.as_deref().unwrap_or(&reply.error)
        )),
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DeviceCodeReply {
    Code(DeviceCode),
    Error(ErrorReply),
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TokenReply {
    Token { access_token: String },
    Error(ErrorReply),
}

#[derive(Deserialize)]
struct ErrorReply {
    error: String,
    error_description: Option<String>,
    interval: Option<u64>,
}
