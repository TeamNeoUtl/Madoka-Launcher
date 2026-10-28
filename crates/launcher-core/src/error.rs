use std::path::PathBuf;

use crate::{Edition, Host, InstanceId, Kind};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse error at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("instance {0} not found")]
    NotFound(InstanceId),
    #[error("kind {kind:?} is unsupported on {host:?}")]
    UnsupportedKind { kind: Kind, host: Host },
    #[error("edition {edition:?} is invalid for kind {kind:?}")]
    InvalidEdition { kind: Kind, edition: Edition },
    #[error("instance name is empty")]
    EmptyName,
    #[error("plugin id is empty")]
    EmptyPluginId,
    #[error("unsafe relative path: {0}")]
    UnsafePath(String),
    #[error("host not allowed: {0}")]
    HostNotAllowed(String),
    #[error("hash mismatch for {name}: expected {expected}, actual {actual}")]
    HashMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("fetch failed for {url}: {message}")]
    Fetch { url: String, message: String },
    #[error("zip error at {path}: {message}")]
    Zip { path: PathBuf, message: String },
    #[error("symlink entry in archive: {0}")]
    SymlinkEntry(String),
    #[error("plugin {0} is not installed")]
    PluginNotFound(String),
    #[error("plugin installation is unsupported for kind {0:?}")]
    PluginUnsupported(Kind),
    #[error("directory id {dir} differs from file id {file}")]
    IdMismatch { dir: InstanceId, file: InstanceId },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn zip(path: impl Into<PathBuf>, source: impl std::fmt::Display) -> Self {
        Self::Zip {
            path: path.into(),
            message: source.to_string(),
        }
    }
}
