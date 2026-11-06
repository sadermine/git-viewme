use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::CoreError;

const FILE: &str = "settings.json";

/// The client ID of the user's own GitHub OAuth app. It is public: the device flow sends it with
/// no secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ClientId(String);

impl ClientId {
    pub(crate) fn parse(raw: &str) -> Result<Self, CoreError> {
        Self::try_from(raw.to_owned()).map_err(CoreError::other)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ClientId {
    type Error = &'static str;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let value = raw.trim();
        let plausible = !value.is_empty()
            && value.len() <= 100
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if plausible {
            Ok(Self(value.to_owned()))
        } else {
            Err(
                "That is not a client ID. Copy the Client ID from your OAuth app's page on \
                 GitHub: letters and digits with no spaces.",
            )
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Settings {
    client_id: Option<ClientId>,
}

/// `%APPDATA%\git-viewme` on Windows.
fn settings_dir() -> Result<PathBuf, CoreError> {
    dirs::config_dir()
        .map(|dir| dir.join("git-viewme"))
        .ok_or_else(|| CoreError::other("This system has no folder for app settings."))
}

fn read(dir: &Path) -> Result<Settings, CoreError> {
    let path = dir.join(FILE);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| {
            CoreError::other(format!(
                "{} is not valid ({error}). Fix it or delete it.",
                path.display()
            ))
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(CoreError::other(format!(
            "Could not read {}: {error}",
            path.display()
        ))),
    }
}

/// Writes a temporary file and renames it over the old one, so a crash never leaves half a file.
fn write(dir: &Path, settings: &Settings) -> Result<(), CoreError> {
    let path = dir.join(FILE);
    let temp = dir.join(format!("{FILE}.tmp"));
    let text = serde_json::to_string_pretty(settings).expect("settings serialize to JSON");
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&temp, text))
        .and_then(|()| fs::rename(&temp, &path))
        .map_err(|error| CoreError::other(format!("Could not save {}: {error}", path.display())))
}

pub fn client_id() -> Result<Option<ClientId>, CoreError> {
    Ok(read(&settings_dir()?)?.client_id)
}

/// Settings holds the client ID alone, so this writes the whole file. That also replaces a file
/// that no longer parses, which would otherwise block the user from entering a new ID.
pub fn save_client_id(raw: &str) -> Result<ClientId, CoreError> {
    let client_id = ClientId::parse(raw)?;
    write(
        &settings_dir()?,
        &Settings {
            client_id: Some(client_id.clone()),
        },
    )?;
    Ok(client_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("git-viewme-settings-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn with_id(id: &str) -> Settings {
        Settings {
            client_id: Some(ClientId::parse(id).unwrap()),
        }
    }

    #[test]
    fn read_with_no_file_is_default() {
        let dir = temp_dir("missing");

        assert_eq!(read(&dir).unwrap(), Settings::default());
    }

    #[test]
    fn write_then_read_returns_the_client_id_and_creates_the_folder() {
        let dir = temp_dir("round-trip").join("nested");

        write(&dir, &with_id("Ov23liTestClient")).unwrap();

        assert_eq!(read(&dir).unwrap(), with_id("Ov23liTestClient"));
        assert!(!dir.join("settings.json.tmp").exists());
    }

    #[test]
    fn write_replaces_an_earlier_client_id() {
        let dir = temp_dir("replace");
        write(&dir, &with_id("Ov23liFirst")).unwrap();

        write(&dir, &with_id("Ov23liSecond")).unwrap();

        assert_eq!(read(&dir).unwrap(), with_id("Ov23liSecond"));
    }

    #[test]
    fn write_of_no_client_id_reads_back_as_none() {
        let dir = temp_dir("cleared");
        write(&dir, &with_id("Ov23liFirst")).unwrap();

        write(&dir, &Settings::default()).unwrap();

        assert_eq!(read(&dir).unwrap().client_id, None);
    }

    #[test]
    fn read_of_malformed_json_is_an_error_naming_the_file() {
        let dir = temp_dir("malformed");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), "{ not json").unwrap();

        let error = read(&dir).unwrap_err();

        assert!(
            matches!(&error, CoreError::Other { message } if message.contains("settings.json"))
        );
    }

    #[test]
    fn read_of_an_invalid_saved_client_id_is_an_error() {
        let dir = temp_dir("invalid-saved");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), r#"{"client_id":"two words"}"#).unwrap();

        assert!(read(&dir).is_err());
    }

    #[test]
    fn parse_trims_surrounding_whitespace() {
        assert_eq!(
            ClientId::parse("  Ov23liTestClient\n").unwrap().as_str(),
            "Ov23liTestClient"
        );
    }

    #[test]
    fn parse_rejects_empty_spaced_and_odd_input() {
        for raw in [
            "",
            "   ",
            "Ov23 li",
            "Ov23li/../x",
            "Ov23li\"",
            &"a".repeat(101),
        ] {
            assert!(
                matches!(ClientId::parse(raw), Err(CoreError::Other { .. })),
                "{raw:?} was accepted"
            );
        }
    }
}
