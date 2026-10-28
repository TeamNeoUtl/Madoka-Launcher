use launcher_core::{Host, Kind};

use crate::HttpError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("json parse error in {context}: {source}")]
    Json {
        context: &'static str,
        source: serde_json::Error,
    },
    #[error("catalog shape changed: {0}")]
    CatalogShapeChanged(String),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("kind {kind:?} is unsupported on {host:?}")]
    UnsupportedHost { kind: Kind, host: Host },
    #[error("version {version} has no matching asset")]
    NoMatchingAsset { version: String },
}
