use std::collections::HashMap;

use launcher_core::{
    AppSelection, AppVersion, Artifact, FileCheck, Hash, Host, InstallPlan, Kind, Location, Origin,
    Package, PackageSource, Recipe, Root, Step,
};
use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::recipe::{direct_artifact, finish};
use crate::release::{latest_artifacts, parse_url};
use crate::{Http, Provider, ProviderError};

const DATA_URL: &str = "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/";
const PATTERN_URL: &str =
    "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/util/archive-name-pattern.json";
const FALLBACK_PAGE: &str = "https://github.com/team-apm/apm-data";
const ZIP_EXTENSION: &str = ".zip";

#[derive(Debug, Deserialize)]
struct RawList {
    packages: Vec<RawDataFile>,
}

#[derive(Debug, Deserialize)]
struct RawDataFile {
    path: String,
}

#[derive(Debug, Deserialize)]
struct RawCore {
    aviutl: RawProgram,
    exedit: RawProgram,
}

#[derive(Debug, Deserialize)]
struct RawProgram {
    #[serde(rename = "latestVersion")]
    latest_version: String,
    #[serde(default)]
    releases: Vec<RawRelease>,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    version: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    integrity: Option<RawIntegrity>,
}

#[derive(Debug, Deserialize)]
struct RawIntegrity {
    archive: String,
    #[serde(default)]
    file: Vec<RawFileHash>,
}

#[derive(Debug, Deserialize)]
struct RawFileHash {
    target: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct RawPackages {
    packages: Vec<RawPackage>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    id: String,
    name: String,
    overview: String,
    developer: String,
    #[serde(rename = "pageURL")]
    page_url: String,
    #[serde(default, rename = "downloadURLs")]
    download_urls: Vec<String>,
    #[serde(rename = "latestVersion")]
    latest_version: String,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    releases: Vec<RawRelease>,
    #[serde(default, rename = "directURL")]
    direct_url: Option<String>,
    #[serde(default, rename = "isHidden")]
    is_hidden: bool,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    filename: String,
    #[serde(default, rename = "archivePath")]
    archive_path: Option<String>,
    #[serde(default, rename = "isDirectory")]
    is_directory: bool,
    #[serde(default, rename = "isUninstallOnly")]
    is_uninstall_only: bool,
    #[serde(default, rename = "isObsolete")]
    is_obsolete: bool,
}

fn shape_changed(message: impl Into<String>) -> ProviderError {
    ProviderError::CatalogShapeChanged(message.into())
}

fn app_check(target: &str, hash: &str) -> Option<FileCheck> {
    Some(FileCheck {
        location: Location {
            root: Root::App,
            rel: target.replace('\\', "/"),
        },
        hash: Hash::parse_sri(hash)?,
    })
}

fn checks_of(release: Option<&RawRelease>) -> Vec<FileCheck> {
    release
        .and_then(|release| release.integrity.as_ref())
        .map(|integrity| {
            integrity
                .file
                .iter()
                .filter_map(|file| app_check(&file.target, &file.hash))
                .collect()
        })
        .unwrap_or_default()
}

fn release_of<'a>(releases: &'a [RawRelease], version: &str) -> Option<&'a RawRelease> {
    releases.iter().find(|release| release.version == version)
}

/// Archive of the latest version of AviUtl or the extended editing plugin.
fn program_artifact(program: &RawProgram) -> Result<(Artifact, Vec<FileCheck>), ProviderError> {
    let release = release_of(&program.releases, &program.latest_version)
        .ok_or_else(|| shape_changed("latest release is missing"))?;
    let text = release
        .url
        .as_deref()
        .ok_or_else(|| shape_changed("release has no url"))?;
    let url = parse_url(text)?;
    let mut artifact = direct_artifact(&url);
    artifact.hash = release
        .integrity
        .as_ref()
        .and_then(|integrity| Hash::parse_sri(&integrity.archive));
    Ok((artifact, checks_of(Some(release))))
}

/// AviUtl and the extended editing plugin of their latest versions form one app version.
pub fn parse_aviutl_app(bytes: &[u8]) -> Result<AppVersion, ProviderError> {
    let core: RawCore = serde_json::from_slice(bytes).map_err(|e| shape_changed(e.to_string()))?;
    let (aviutl, mut checks) = program_artifact(&core.aviutl)?;
    let (exedit, exedit_checks) = program_artifact(&core.exedit)?;
    checks.extend(exedit_checks);
    Ok(AppVersion {
        version: format!(
            "{} / exedit {}",
            core.aviutl.latest_version, core.exedit.latest_version
        ),
        prerelease: false,
        published_at: None,
        artifacts: vec![aviutl, exedit],
        checks,
    })
}

/// `https://github.com/<owner>/<repo>/releases...` gives `(owner, repo)`.
fn github_release_repo(text: &str) -> Option<(String, String)> {
    let url = Url::parse(text).ok()?;
    if url.host_str() != Some("github.com") {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [owner, repo, "releases", ..] => Some(((*owner).to_owned(), (*repo).to_owned())),
        _ => None,
    }
}

fn recipe(package: &RawPackage) -> Option<Recipe> {
    let origin = match &package.direct_url {
        Some(url) => Origin::Direct(Url::parse(url).ok()?),
        None => {
            let (owner, repo) = package
                .download_urls
                .iter()
                .find_map(|url| github_release_repo(url))?;
            Origin::GithubRelease {
                owner,
                repo,
                pattern: None,
            }
        }
    };
    let release = release_of(&package.releases, &package.latest_version);
    let mut steps = vec![Step::Extract];
    steps.extend(
        package
            .files
            .iter()
            .filter(|file| !file.is_uninstall_only && !file.is_obsolete)
            .map(|file| Step::Place {
                name: file.filename.clone(),
                archive_dir: file.archive_path.clone().filter(|dir| !dir.is_empty()),
                directory: file.is_directory,
            }),
    );
    if steps.len() == 1 {
        return None;
    }
    Some(Recipe {
        version: package.latest_version.clone(),
        origin,
        hash: release
            .and_then(|release| release.integrity.as_ref())
            .and_then(|integrity| Hash::parse_sri(&integrity.archive)),
        steps,
        checks: checks_of(release),
    })
}

/// Packages of one developer file. Hidden packages are left out.
pub fn parse_apm_packages(bytes: &[u8]) -> Result<Vec<Package>, ProviderError> {
    let raw: RawPackages =
        serde_json::from_slice(bytes).map_err(|e| shape_changed(e.to_string()))?;
    let fallback = parse_url(FALLBACK_PAGE)?;
    Ok(raw
        .packages
        .into_iter()
        .filter(|package| !package.is_hidden)
        .map(|package| {
            let page_url = Url::parse(&package.page_url).unwrap_or_else(|_| fallback.clone());
            let source = recipe(&package).map_or_else(
                || PackageSource::Web(page_url.clone()),
                PackageSource::Recipe,
            );
            Package {
                id: package.id,
                name: package.name,
                summary: package.overview,
                author: package.developer,
                categories: Vec::new(),
                updated: None,
                page_url,
                source,
            }
        })
        .collect())
}

pub struct AviutlProvider<H> {
    http: H,
}

impl<H: Http> AviutlProvider<H> {
    pub fn new(http: H) -> Self {
        Self { http }
    }

    async fn data(&self, path: &str) -> Result<Vec<u8>, ProviderError> {
        Ok(self
            .http
            .get(&parse_url(&format!("{DATA_URL}{path}"))?)
            .await?)
    }

    /// Asset of the latest release. One `.zip` asset is taken as it is.
    /// Several are narrowed by the pattern that `archive-name-pattern.json` lists for the package.
    async fn github_artifact(
        &self,
        package: &Package,
        owner: &str,
        repo: &str,
    ) -> Result<Option<Artifact>, ProviderError> {
        let Some(artifacts) = latest_artifacts(&self.http, owner, repo).await? else {
            return Ok(None);
        };
        let mut zips: Vec<Artifact> = artifacts
            .into_iter()
            .filter(|artifact| artifact.name.to_ascii_lowercase().ends_with(ZIP_EXTENSION))
            .collect();
        if zips.len() <= 1 {
            return Ok(zips.pop());
        }
        let bytes = self.http.get(&parse_url(PATTERN_URL)?).await?;
        let patterns: HashMap<String, String> = serde_json::from_slice(&bytes)
            .map_err(|e| shape_changed(format!("archive-name-pattern.json: {e}")))?;
        let Some(pattern) = patterns.get(&package.id) else {
            return Ok(None);
        };
        let regex = Regex::new(&format!("^{pattern}\\.zip$"))
            .map_err(|e| shape_changed(format!("invalid asset pattern of {}: {e}", package.id)))?;
        let mut matched: Vec<Artifact> = zips
            .into_iter()
            .filter(|artifact| regex.is_match(&artifact.name))
            .collect();
        Ok((matched.len() == 1).then(|| matched.remove(0)))
    }
}

impl<H: Http> Provider for AviutlProvider<H> {
    fn kind(&self) -> Kind {
        Kind::Aviutl
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        Ok(vec![parse_aviutl_app(&self.data("core.json").await?)?])
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        let list: RawList = serde_json::from_slice(&self.data("list.json").await?)
            .map_err(|e| shape_changed(e.to_string()))?;
        let mut packages = Vec::new();
        for file in &list.packages {
            packages.extend(parse_apm_packages(&self.data(&file.path).await?)?);
        }
        if packages.is_empty() {
            return Err(shape_changed("no packages"));
        }
        Ok(packages)
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        if selection.host != Host::Windows {
            return Err(ProviderError::UnsupportedHost {
                kind: Kind::Aviutl,
                host: selection.host,
            });
        }
        Ok(version.artifacts.clone())
    }

    async fn plan_install_package(&self, package: &Package) -> Result<InstallPlan, ProviderError> {
        let browser = || InstallPlan::OpenInBrowser(package.page_url.clone());
        let PackageSource::Recipe(recipe) = &package.source else {
            return Ok(browser());
        };
        let artifact = match &recipe.origin {
            Origin::Direct(url) => Some(direct_artifact(url)),
            Origin::GithubRelease { owner, repo, .. } => {
                self.github_artifact(package, owner, repo).await?
            }
        };
        Ok(artifact.map_or_else(browser, |artifact| finish(package, recipe, artifact)))
    }
}
