use std::sync::LazyLock;

use launcher_core::{
    AppSelection, AppVersion, Artifact, Edition, Host, InstallPlan, Kind, Package, PackageSource,
};
use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::release::{Release, app_versions_from_json, parse_url};
use crate::{Http, HttpError, Provider, ProviderError};

const REPO: &str = "manju-summoner/YukkuriMovieMaker4";
const PLUGIN_INDEX_URL: &str = "https://manjubox.net/ymm4plugins.yml";
const LITE_SUFFIX: &str = "_Lite.zip";
const PLUGIN_EXTENSION: &str = ".ymme";
const GITHUB_HOST: &str = "github.com";

static APP_ASSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^YukkuriMovieMaker_v[0-9.]+(_Lite)?\.zip$").expect("static regex")
});

pub struct Ymm4Provider<H> {
    http: H,
}

impl<H: Http> Ymm4Provider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }
}

fn edition_of(asset_name: &str) -> Edition {
    if asset_name.ends_with(LITE_SUFFIX) {
        Edition::Lite
    } else {
        Edition::Normal
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntry {
    r#type: String,
    date: String,
    name: String,
    url: String,
    author: String,
    #[allow(dead_code)]
    author_url: String,
    description: String,
    #[serde(default = "enabled_by_default")]
    is_enabled: bool,
}

const fn enabled_by_default() -> bool {
    true
}

fn github_repo(url: &Url) -> Option<(String, String)> {
    if url.host_str() != Some(GITHUB_HOST) {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [owner, repo] => Some(((*owner).to_owned(), (*repo).to_owned())),
        _ => None,
    }
}

fn shape_changed(message: impl Into<String>) -> ProviderError {
    ProviderError::CatalogShapeChanged(message.into())
}

/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `bytes` is not the expected YAML,
/// holds no entry, or an entry has an invalid URL.
pub fn parse_ymm4_catalog(bytes: &[u8]) -> Result<Vec<Package>, ProviderError> {
    let entries: Vec<RawEntry> =
        serde_yaml_ng::from_slice(bytes).map_err(|e| shape_changed(e.to_string()))?;
    if entries.is_empty() {
        return Err(shape_changed("no entries"));
    }
    let mut packages = Vec::new();
    for entry in entries.into_iter().filter(|entry| entry.is_enabled) {
        let page_url = Url::parse(&entry.url)
            .map_err(|_| shape_changed(format!("invalid url: {}", entry.url)))?;
        let (id, source) = match github_repo(&page_url) {
            Some((owner, repo)) => (
                format!("github:{owner}/{repo}"),
                PackageSource::GithubRepo { owner, repo },
            ),
            None => (page_url.to_string(), PackageSource::Web(page_url.clone())),
        };
        packages.push(Package {
            id,
            name: entry.name,
            summary: entry.description,
            author: entry.author,
            categories: entry
                .r#type
                .split(',')
                .map(str::trim)
                .filter(|category| !category.is_empty())
                .map(str::to_owned)
                .collect(),
            updated: Some(entry.date),
            page_url,
            source,
        });
    }
    if packages.is_empty() {
        return Err(shape_changed("no enabled entries"));
    }
    Ok(packages)
}

impl<H: Http> Provider for Ymm4Provider<H> {
    fn kind(&self) -> Kind {
        Kind::Ymm4
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        let url = parse_url(&format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=30"
        ))?;
        let bytes = self.http.get(&url).await?;
        app_versions_from_json(&bytes, |name| APP_ASSET.is_match(name))
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        let url = parse_url(PLUGIN_INDEX_URL)?;
        let bytes = self.http.get(&url).await?;
        parse_ymm4_catalog(&bytes)
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        if selection.host != Host::Windows {
            return Err(ProviderError::UnsupportedHost {
                kind: Kind::Ymm4,
                host: selection.host,
            });
        }
        version
            .artifacts
            .iter()
            .find(|artifact| edition_of(&artifact.name) == selection.edition)
            .cloned()
            .map(|artifact| vec![artifact])
            .ok_or_else(|| ProviderError::NoMatchingAsset {
                version: version.version.clone(),
            })
    }

    async fn plan_install_package(&self, package: &Package) -> Result<InstallPlan, ProviderError> {
        let browser = || InstallPlan::OpenInBrowser(package.page_url.clone());
        let PackageSource::GithubRepo { owner, repo } = &package.source else {
            return Ok(browser());
        };
        let url = parse_url(&format!(
            "https://api.github.com/repos/{owner}/{repo}/releases/latest"
        ))?;
        let bytes = match self.http.get(&url).await {
            Ok(bytes) => bytes,
            Err(HttpError::Status { status: 404, .. }) => return Ok(browser()),
            Err(error) => return Err(error.into()),
        };
        let mut plugins: Vec<Artifact> = Release::from_json(&bytes)?
            .into_artifacts()?
            .into_iter()
            .filter(|artifact| {
                artifact
                    .name
                    .to_ascii_lowercase()
                    .ends_with(PLUGIN_EXTENSION)
            })
            .collect();
        Ok(match plugins.len() {
            1 => InstallPlan::Download(std::mem::take(&mut plugins)),
            _ => browser(),
        })
    }
}
