use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Error;

const CONFIG_FILE: &str = "config.toml";
const TEMP_EXTENSION: &str = "toml.tmp";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    /// Name of an `iced::Theme`, as printed by its `Display` implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Family name of the UI font. `None` selects the iced default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

impl Config {
    /// Returns the default value when `<root>/config.toml` does not exist.
    pub fn load(root: &Path) -> Result<Self, Error> {
        let path = root.join(CONFIG_FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(Error::io(&path, e)),
        };
        toml::from_str(&text).map_err(|source| Error::Parse { path, source })
    }

    /// Writes `<root>/config.toml` through a temporary file.
    pub fn save(&self, root: &Path) -> Result<(), Error> {
        let text = toml::to_string_pretty(self)?;
        fs::create_dir_all(root).map_err(|e| Error::io(root, e))?;
        let path = root.join(CONFIG_FILE);
        let temp = path.with_extension(TEMP_EXTENSION);
        fs::write(&temp, text).map_err(|e| Error::io(&temp, e))?;
        fs::rename(&temp, &path).map_err(|e| Error::io(&path, e))
    }
}
