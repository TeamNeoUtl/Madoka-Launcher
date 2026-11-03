use std::collections::{HashMap, HashSet};

use launcher_core::{
    AppSelection, AppVersion, Artifact, FileCheck, Hash, Host, InstallPlan, Kind, Location, Origin,
    Package, PackageSource, Recipe, Root, Step,
};
use regex::Regex;
use serde::Deserialize;
use tracing::warn;
use url::Url;

use crate::history::{self, parse_rows};
use crate::recipe::{direct_artifact, finish};
use crate::release::{latest_artifacts, parse_url};
use crate::{Http, Provider, ProviderError, Versions};

const DATA_URL: &str = "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/";
const PATTERN_URL: &str =
    "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/util/archive-name-pattern.json";
const FALLBACK_PAGE: &str = "https://github.com/team-apm/apm-data";
const ZIP_EXTENSION: &str = ".zip";
/// Link to `<name><digits><letter?>.zip`, followed by the next `version<number>` text.
/// `aviutl2*.zip` and the other name do not match.
const HISTORY_ROW_AVIUTL: &str = r#"(?s)(?i:href)\s*=\s*["']([^"']*aviutl\d+[a-z]?\.zip)["'].*?(?i:version)\s*([0-9]+\.[0-9]+[a-z]?)"#;
const HISTORY_ROW_EXEDIT: &str = r#"(?s)(?i:href)\s*=\s*["']([^"']*exedit\d+[a-z]?\.zip)["'].*?(?i:version)\s*([0-9]+\.[0-9]+[a-z]?)"#;

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
    #[serde(default)]
    releases: Vec<RawRelease>,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    version: String,
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

fn archive_hash(release: Option<&RawRelease>) -> Option<Hash> {
    release
        .and_then(|release| release.integrity.as_ref())
        .and_then(|integrity| Hash::parse_sri(&integrity.archive))
}

/// Versions of one program, in the order of the history page: one entry per version that `row`
/// matches. The first entry of a version wins. `core` supplies the archive hash and the file
/// hashes of the versions that it lists. A version that only `core` lists is left out.
fn program_versions(
    program: Option<&RawProgram>,
    page: &[u8],
    row: &str,
) -> Result<Vec<AppVersion>, ProviderError> {
    let rows = parse_rows(page, row)?;
    if rows.is_empty() {
        return Err(shape_changed("history page lists no archive"));
    }
    let mut seen = HashSet::new();
    Ok(rows
        .into_iter()
        .filter(|(version, _)| seen.insert(version.clone()))
        .map(|(version, url)| {
            let release = program.and_then(|program| release_of(&program.releases, &version));
            let mut artifact = direct_artifact(&url);
            artifact.hash = archive_hash(release);
            AppVersion {
                version,
                prerelease: false,
                published_at: None,
                artifacts: vec![artifact],
                checks: checks_of(release),
            }
        })
        .collect())
}

fn parse_core(core: &[u8]) -> Result<RawCore, ProviderError> {
    serde_json::from_slice(core).map_err(|e| shape_changed(e.to_string()))
}

/// `None` when `core` is absent or not the expected JSON document.
fn usable_core(core: Option<&[u8]>) -> Option<RawCore> {
    parse_core(core?)
        .inspect_err(|error| warn!(%error, "apm core is unusable"))
        .ok()
}

/// `AviUtl` versions of the history page, newest first.
/// `core` adds hashes. Without it, or with an unusable one, the archives carry no hash.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `page` lists no `AviUtl` archive,
/// and `ProviderError::InvalidUrl` for an invalid URL.
pub fn parse_aviutl_apps(
    core: Option<&[u8]>,
    page: &[u8],
) -> Result<Vec<AppVersion>, ProviderError> {
    program_versions(
        usable_core(core).as_ref().map(|core| &core.aviutl),
        page,
        HISTORY_ROW_AVIUTL,
    )
}

/// Extended editing plugin versions of the history page, in the order of `parse_aviutl_apps`.
/// The archive holds the files that belong directly beside `aviutl.exe`.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `page` lists no extended editing archive,
/// and `ProviderError::InvalidUrl` for an invalid URL.
pub fn parse_aviutl_addons(
    core: Option<&[u8]>,
    page: &[u8],
) -> Result<Vec<AppVersion>, ProviderError> {
    program_versions(
        usable_core(core).as_ref().map(|core| &core.exedit),
        page,
        HISTORY_ROW_EXEDIT,
    )
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
    let origin = if let Some(url) = &package.direct_url {
        Origin::Direct(Url::parse(url).ok()?)
    } else {
        let (owner, repo) = package
            .download_urls
            .iter()
            .find_map(|url| github_release_repo(url))?;
        Origin::GithubRelease {
            owner,
            repo,
            pattern: None,
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
        hash: archive_hash(release),
        steps,
        checks: checks_of(release),
    })
}

/// Packages of one developer file. Hidden packages are left out.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `bytes` is not the expected JSON document
/// or an entry lacks a required field, and `ProviderError::InvalidUrl` for an invalid URL.
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
    pub const fn new(http: H) -> Self {
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
        Ok(self.versions().await?.app)
    }

    async fn versions(&self) -> Result<Versions, ProviderError> {
        let page = history::fetch_page(&self.http).await?;
        let core = self
            .data("core.json")
            .await
            .inspect_err(|error| warn!(%error, "apm core unavailable"))
            .ok();
        Ok(Versions {
            app: parse_aviutl_apps(core.as_deref(), &page)?,
            addon: parse_aviutl_addons(core.as_deref(), &page)?,
        })
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
