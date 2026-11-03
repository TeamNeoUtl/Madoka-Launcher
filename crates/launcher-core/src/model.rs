use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(Uuid);

impl InstanceId {
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
    }
}

impl FromStr for InstanceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Aviutl,
    Aviutl2,
    Ymm4,
    Neoutl,
    Freya,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    #[default]
    Normal,
    Lite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRecord {
    pub id: String,
    pub version: String,
    pub source: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: InstanceId,
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub edition: Edition,
    pub app_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(default, rename = "plugin")]
    pub plugins: Vec<PluginRecord>,
}

impl InstanceConfig {
    /// # Errors
    ///
    /// Returns `Error::EmptyName` for a blank name, `Error::InvalidEdition` for `Edition::Lite`
    /// on a kind other than `Kind::Ymm4`, `Error::EmptyPluginId` for a blank plugin id,
    /// and `Error::UnsafePath` for a plugin file path that leaves the app directory.
    pub fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty() {
            return Err(Error::EmptyName);
        }
        if self.edition == Edition::Lite && self.kind != Kind::Ymm4 {
            return Err(Error::InvalidEdition {
                kind: self.kind,
                edition: self.edition,
            });
        }
        for plugin in &self.plugins {
            if plugin.id.trim().is_empty() {
                return Err(Error::EmptyPluginId);
            }
            for file in &plugin.files {
                ensure_relative_path(file)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInstance {
    pub name: String,
    pub kind: Kind,
    pub edition: Edition,
    pub app_version: String,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub dir: PathBuf,
    pub config: InstanceConfig,
}

impl Instance {
    pub const CONFIG_FILE: &'static str = "instance.toml";
    pub const APP_DIR: &'static str = "app";

    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.dir.join(Self::CONFIG_FILE)
    }

    #[must_use]
    pub fn app_dir(&self) -> PathBuf {
        self.dir.join(Self::APP_DIR)
    }
}

/// Accepts only paths made of normal components, so that joining the path
/// onto an instance directory stays inside that directory.
pub fn ensure_relative_path(path: &str) -> Result<(), Error> {
    let normalized = path.replace('\\', "/");
    let all_normal = !normalized.is_empty()
        && Path::new(&normalized)
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    let has_drive_prefix = normalized.as_bytes().get(1) == Some(&b':');
    if all_normal && !has_drive_prefix {
        Ok(())
    } else {
        Err(Error::UnsafePath(path.to_owned()))
    }
}

pub fn ensure_single_component(name: &str) -> Result<(), Error> {
    ensure_relative_path(name)?;
    if name.contains(['/', '\\']) {
        return Err(Error::UnsafePath(name.to_owned()));
    }
    Ok(())
}
