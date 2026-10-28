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
}
