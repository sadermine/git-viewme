use std::sync::LazyLock;

use keyring::{Entry, Error as KeyringError};
use reqwest::header::{AUTHORIZATION, HeaderValue};
use reqwest::{Client, Request, Response};

use crate::CoreError;

const SERVICE: &str = "git-viewme";
const ACCOUNT: &str = "github";
const API_HOST: &str = "api.github.com";

/// A GitHub OAuth token. It has no `Serialize` and its `Debug` hides the value, so no Tauri
/// command can return it and no log line can print it. Its value and `load_token` are private to
/// this module, so the only way another file can use the token is `send_authorized`.
pub(crate) struct AccessToken(String);

impl AccessToken {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessToken(..)")
    }
}

fn entry() -> Result<Entry, CoreError> {
    Entry::new(SERVICE, ACCOUNT).map_err(store_error)
}

fn store_error(error: KeyringError) -> CoreError {
    CoreError::other(format!("The credential store failed: {error}"))
}

pub(crate) fn store_token(token: &AccessToken) -> Result<(), CoreError> {
    entry()?.set_password(&token.0).map_err(store_error)
}

/// The client that sends the token. It lives here so that no caller can hand `send_authorized` a
/// client that resolves the API host to another address or accepts any certificate.
static HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(concat!("git-viewme/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("the HTTP client builds with static settings")
});

/// A handle to the client `send_authorized` uses, for building requests and for requests that
/// carry no token.
pub(crate) fn http() -> Client {
    HTTP.clone()
}

/// Sends a request for GitHub's API with the stored token. It returns only the response, so no
/// caller can read the token back from the request's headers. A request for any other host is
/// refused before the store is read.
pub(crate) async fn send_authorized(request: Request) -> Result<Response, CoreError> {
    Ok(HTTP.execute(authorize(request)?).await?)
}

fn authorize(request: Request) -> Result<Request, CoreError> {
    check_api_url(&request)?;
    let token = load_token()?.ok_or(CoreError::Unauthorized)?;
    with_token(request, &token)
}

fn check_api_url(request: &Request) -> Result<(), CoreError> {
    let url = request.url();
    if url.scheme() == "https" && url.host_str() == Some(API_HOST) && url.port().is_none() {
        Ok(())
    } else {
        Err(CoreError::other(format!(
            "Refused to send the token to {}",
            url.origin().ascii_serialization()
        )))
    }
}

fn with_token(mut request: Request, token: &AccessToken) -> Result<Request, CoreError> {
    let mut value = HeaderValue::from_str(&format!("Bearer {}", token.0))
        .map_err(|_| CoreError::other("The stored token is not a valid header value"))?;
    value.set_sensitive(true);
    request.headers_mut().insert(AUTHORIZATION, value);
    Ok(request)
}

fn load_token() -> Result<Option<AccessToken>, CoreError> {
    match entry()?.get_password() {
        Ok(value) => Ok(Some(AccessToken(value))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(store_error(error)),
    }
}

pub fn is_signed_in() -> Result<bool, CoreError> {
    load_token().map(|token| token.is_some())
}

/// Succeeds when no token is stored, so signing out twice is not an error.
pub fn delete_token() -> Result<(), CoreError> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(store_error(error)),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Method;

    use super::*;

    fn request(url: &str) -> Request {
        Client::new()
            .request(Method::GET, url)
            .build()
            .expect("the test URL parses")
    }

    #[test]
    fn authorize_refuses_a_host_other_than_the_api_before_reading_the_store() {
        for url in [
            "https://github.com/login/oauth/access_token",
            "http://api.github.com/user",
            "https://api.github.com.evil.example/user",
            "https://api.github.com:8443/user",
        ] {
            let error = authorize(request(url)).expect_err(url);
            assert!(
                matches!(&error, CoreError::Other { message } if message.starts_with("Refused to send the token to")),
                "{url} gave {error:?}"
            );
        }
    }

    #[test]
    fn with_token_sets_a_sensitive_bearer_header() {
        let token = AccessToken::new("gho_test".to_owned());
        let request = with_token(request("https://api.github.com/user"), &token).unwrap();
        let value = &request.headers()[AUTHORIZATION];
        assert_eq!(value, "Bearer gho_test");
        assert!(value.is_sensitive());
    }
}
