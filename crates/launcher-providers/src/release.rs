use crate::{Http, HttpError};
use launcher_core::{AppVersion, Artifact, Hash};
use serde::Deserialize;
use url::Url;

use crate::ProviderError;

#[derive(Debug, Deserialize)]
pub struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    size: Option<u64>,
    digest: Option<String>,
    browser_download_url: String,
}

impl Asset {
    fn into_artifact(self) -> Result<Artifact, ProviderError> {
        let url = Url::parse(&self.browser_download_url)
            .map_err(|_| ProviderError::InvalidUrl(self.browser_download_url.clone()))?;
        Ok(Artifact {
            hash: self.digest.as_deref().and_then(Hash::parse_digest),
            name: self.name,
            url,
            size: self.size,
        })
    }
}

impl Release {
    pub(crate) fn list_from_json(bytes: &[u8]) -> Result<Vec<Self>, ProviderError> {
        serde_json::from_slice(bytes).map_err(|source| ProviderError::Json {
            context: "release list",
            source,
        })
    }

    pub(crate) fn from_json(bytes: &[u8]) -> Result<Self, ProviderError> {
        serde_json::from_slice(bytes).map_err(|source| ProviderError::Json {
            context: "release",
            source,
        })
    }

    pub(crate) fn into_artifacts(self) -> Result<Vec<Artifact>, ProviderError> {
        self.assets.into_iter().map(Asset::into_artifact).collect()
    }

    /// Keeps published releases that have at least one asset accepted by `accepts`.
    pub(crate) fn into_app_version(
        self,
        accepts: impl Fn(&str) -> bool,
    ) -> Result<Option<AppVersion>, ProviderError> {
        if self.draft {
            return Ok(None);
        }
        let version = self.tag_name.clone();
        let prerelease = self.prerelease;
        let published_at = self.published_at.clone();
        let artifacts: Vec<Artifact> = self
            .into_artifacts()?
            .into_iter()
            .filter(|artifact| accepts(&artifact.name))
            .collect();
        Ok((!artifacts.is_empty()).then_some(AppVersion {
            version,
            prerelease,
            published_at,
            artifacts,
            checks: Vec::new(),
        }))
    }
}

pub fn app_versions_from_json(
    bytes: &[u8],
    accepts: impl Fn(&str) -> bool,
) -> Result<Vec<AppVersion>, ProviderError> {
    let mut versions = Vec::new();
    for release in Release::list_from_json(bytes)? {
        versions.extend(release.into_app_version(&accepts)?);
    }
    Ok(versions)
}

pub fn parse_url(text: &str) -> Result<Url, ProviderError> {
    Url::parse(text).map_err(|_| ProviderError::InvalidUrl(text.to_owned()))
}

/// Artifacts of the latest release of `owner/repo`. `None` when the repository has no release.
pub async fn latest_artifacts<H: Http>(
    http: &H,
    owner: &str,
    repo: &str,
) -> Result<Option<Vec<Artifact>>, ProviderError> {
    let url = parse_url(&format!(
        "https://api.github.com/repos/{owner}/{repo}/releases/latest"
    ))?;
    match http.get(&url).await {
        Ok(bytes) => Ok(Some(Release::from_json(&bytes)?.into_artifacts()?)),
        Err(HttpError::Status { status: 404, .. }) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
