use keyring::{Entry, Error as KeyringError};

use crate::CoreError;

const SERVICE: &str = "git-viewme";
const ACCOUNT: &str = "github";

/// A GitHub OAuth token. It has no `Serialize` and its `Debug` hides the value, so no Tauri
/// command can return it and no log line can print it.
pub struct AccessToken(String);

impl AccessToken {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn secret(&self) -> &str {
        &self.0
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

pub fn store_token(token: &AccessToken) -> Result<(), CoreError> {
    entry()?.set_password(token.secret()).map_err(store_error)
}

pub(crate) fn load_token() -> Result<Option<AccessToken>, CoreError> {
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
