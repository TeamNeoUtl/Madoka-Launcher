use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use crate::Error;

const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub github_token: Option<String>,
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
}
