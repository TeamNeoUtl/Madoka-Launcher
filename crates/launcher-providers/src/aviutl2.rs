use launcher_core::{
    AppSelection, AppVersion, Artifact, FileCheck, Hash, Host, InstallPlan, Kind, License,
    Location, Origin, Package, PackageSource, Recipe, Root, Step,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;
use url::Url;

use crate::history::{self, parse_rows};
use crate::recipe::{direct_artifact, finish};
use crate::release::{latest_artifacts, parse_url};
use crate::{Http, Provider, ProviderError};

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json";
const CORE_ID: &str = "Kenkun.AviUtlExEdit2";
const CATALOG_PAGE: &str = "https://github.com/Neosku/aviutl2-catalog-data";
/// Link to `aviutl2beta<n>.zip` or `aviutl2_v<x.y.z>.zip`; group 2 is `beta<n>` or `v<x.y.z>`.
const HISTORY_ROW: &str =
    r#"["']([^"']*aviutl2_?((?:beta[0-9]+|v[0-9]+(?:\.[0-9]+)+)[a-z]?)\.zip)["']"#;

/// Placeholder of the index, the root it names and the directory below that root.
const PLACEHOLDERS: [(&str, Root, &str); 5] = [
    ("{tmp}", Root::Tmp, ""),
    ("{appDir}", Root::App, ""),
    ("{dataDir}", Root::Data, ""),
    ("{pluginsDir}", Root::Data, "Plugin"),
    ("{scriptsDir}", Root::Data, "Script"),
];

#[derive(Debug, Deserialize)]
struct RawEntry {
    id: String,
    name: String,
    r#type: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    author: String,
    #[serde(default, rename = "repoURL")]
    repo_url: String,
    #[serde(rename = "latest-version")]
    latest_version: String,
    installer: RawInstaller,
    #[serde(default)]
    version: Vec<RawVersion>,
    #[serde(default, rename = "niconiCommonsId")]
    niconi_commons_id: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    deprecation: Option<Value>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    licenses: Vec<RawLicense>,
    #[serde(default)]
    changelog: Option<Value>,
    #[serde(default)]
    images: Option<Value>,
}

/// License templates of the upstream catalog; `<years>` and `<holder>` are placeholders.
const LICENSE_TEMPLATES: [(&str, &str); 8] = [
    ("MIT", include_str!("../assets/licenses/MIT.txt")),
    (
        "Apache-2.0",
        include_str!("../assets/licenses/Apache-2.0.txt"),
    ),
    (
        "BSD-2-Clause",
        include_str!("../assets/licenses/BSD-2-Clause.txt"),
    ),
    (
        "BSD-3-Clause",
        include_str!("../assets/licenses/BSD-3-Clause.txt"),
    ),
    ("CC0-1.0", include_str!("../assets/licenses/CC0-1.0.txt")),
    ("GPL-2.0", include_str!("../assets/licenses/GPL-2.0.txt")),
    ("GPL-3.0", include_str!("../assets/licenses/GPL-3.0.txt")),
    (
        "Unlicense",
        include_str!("../assets/licenses/Unlicense.txt"),
    ),
];

/// A license as a bare name (flat schema) or as an object (`type`, `copyrights`, `licenseBody`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawLicense {
    Name(String),
    Detail(RawLicenseDetail),
}

#[derive(Debug, Deserialize)]
struct RawLicenseDetail {
    r#type: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    copyrights: Vec<RawCopyright>,
    #[serde(default, rename = "licenseBody")]
    license_body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCopyright {
    years: String,
    holder: String,
}

fn license_of(raw: RawLicense) -> License {
    let detail = match raw {
        RawLicense::Name(name) => {
            return License {
                body: name.clone(),
                name,
            };
        }
        RawLicense::Detail(detail) => detail,
    };
    let name = detail.name.unwrap_or_else(|| detail.r#type.clone());
    let custom = detail.license_body.filter(|body| !body.trim().is_empty());
    let body = custom
        .or_else(|| license_template(&detail.r#type, &detail.copyrights))
        .unwrap_or_else(|| name.clone());
    License { name, body }
}

/// Template of `kind` with the first copyright entry filled in. `None` for an unknown `kind`.
fn license_template(kind: &str, copyrights: &[RawCopyright]) -> Option<String> {
    let (_, template) = LICENSE_TEMPLATES.iter().find(|(name, _)| *name == kind)?;
    let (years, holder) = copyrights
        .first()
        .map_or(("", ""), |c| (c.years.trim(), c.holder.trim()));
    let text = Regex::new("(?i)<years>")
        .ok()?
        .replace_all(template, regex::NoExpand(years))
        .into_owned();
    let text = Regex::new("(?i)<holder>")
        .ok()?
        .replace_all(&text, regex::NoExpand(holder))
        .into_owned();
    Some(text.trim().to_owned())
}

#[derive(Debug, Deserialize)]
struct RawInstaller {
    source: Value,
    #[serde(default)]
    install: Vec<RawAction>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
enum RawAction {
    #[serde(rename = "download")]
    Download {},
    #[serde(rename = "extract")]
    Extract {},
    #[serde(rename = "extractSfx")]
    ExtractSfx {},
    #[serde(rename = "runAuoSetup")]
    RunAuoSetup { path: String },
    #[serde(rename = "copy")]
    Copy { from: String, to: String },
    #[serde(rename = "delete")]
    Delete {},
    #[serde(other)]
    Unsupported,
}

#[derive(Debug, Deserialize)]
struct RawVersion {
    version: String,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    file: Vec<RawFile>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    path: String,
    #[serde(default, rename = "XXH3_128")]
    xxh3_128: String,
}

#[derive(Debug, Deserialize)]
struct RawGithub {
    owner: String,
    repo: String,
    pattern: String,
}

fn shape_changed(message: impl Into<String>) -> ProviderError {
    ProviderError::CatalogShapeChanged(message.into())
}

fn parse_entries(bytes: &[u8]) -> Result<Vec<RawEntry>, ProviderError> {
    let entries: Vec<RawEntry> =
        serde_json::from_slice(bytes).map_err(|e| shape_changed(e.to_string()))?;
    if entries.is_empty() {
        return Err(shape_changed("no entries"));
    }
    Ok(entries)
}

/// Resolves `{tmp}/x`, `{pluginsDir}/x` and the like. `None` for any other text.
fn location(text: &str) -> Option<Location> {
    let (placeholder, root, base) = PLACEHOLDERS
        .iter()
        .find(|(placeholder, _, _)| text.starts_with(placeholder))?;
    let rest = text[placeholder.len()..].replace('\\', "/");
    if !(rest.is_empty() || rest.starts_with('/')) || rest.contains('{') {
        return None;
    }
    let rest = rest.trim_matches('/');
    let rel = match (base.is_empty(), rest.is_empty()) {
        (true, _) => rest.to_owned(),
        (false, true) => (*base).to_owned(),
        (false, false) => format!("{base}/{rest}"),
    };
    Some(Location { root: *root, rel })
}

fn origin(source: &Value) -> Option<Origin> {
    if let Some(url) = source.get("direct").and_then(Value::as_str) {
        return Url::parse(url).ok().map(Origin::Direct);
    }
    let github: RawGithub = serde_json::from_value(source.get("github")?.clone()).ok()?;
    Some(Origin::GithubRelease {
        owner: github.owner,
        repo: github.repo,
        pattern: Some(github.pattern),
    })
}

fn checks(entry: &RawEntry) -> Vec<FileCheck> {
    checks_of(entry, &entry.latest_version)
}

fn checks_of(entry: &RawEntry, version: &str) -> Vec<FileCheck> {
    let Some(version) = entry.version.iter().find(|raw| raw.version == version) else {
        return Vec::new();
    };
    version
        .file
        .iter()
        .filter_map(|file| {
            Some(FileCheck {
                location: location(&file.path)?,
                hash: Hash::parse_xxh3_128(&file.xxh3_128)?,
            })
        })
        .collect()
}

/// `None` when an action cannot run without the editor setup, a program or a self-extractor.
fn recipe(entry: &RawEntry) -> Option<Recipe> {
    let origin = origin(&entry.installer.source)?;
    let mut actions = entry.installer.install.iter();
    if !matches!(actions.next(), Some(RawAction::Download {})) {
        return None;
    }
    let mut steps = Vec::new();
    for action in actions {
        match action {
            RawAction::Extract {} => steps.push(Step::Extract),
            RawAction::ExtractSfx {} => steps.push(Step::ExtractSfx),
            RawAction::RunAuoSetup { path } => steps.push(Step::RunAuoSetup {
                path: location(path)?,
            }),
            RawAction::Copy { from, to } => steps.push(Step::Copy {
                from: location(from)?,
                to: location(to)?,
            }),
            // A new instance holds no file that an install-time delete targets.
            RawAction::Delete {} => {}
            RawAction::Download {} | RawAction::Unsupported => return None,
        }
    }
    if !steps.iter().any(|step| matches!(step, Step::Copy { .. })) {
        return None;
    }
    Some(Recipe {
        version: entry.latest_version.clone(),
        origin,
        hash: None,
        steps,
        checks: checks(entry),
    })
}

/// Entries other than the editor itself. Entries that `recipe` rejects open in the browser.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `bytes` is not the expected document
/// or holds no package entry, and `ProviderError::InvalidUrl` for an invalid page URL.
pub fn parse_aviutl2_catalog(bytes: &[u8]) -> Result<Vec<Package>, ProviderError> {
    let fallback =
        Url::parse(CATALOG_PAGE).map_err(|_| ProviderError::InvalidUrl(CATALOG_PAGE.to_owned()))?;
    let packages: Vec<Package> = parse_entries(bytes)?
        .into_iter()
        .filter(|entry| entry.id != CORE_ID)
        .map(|entry| package_of(entry, &fallback))
        .collect();
    if packages.is_empty() {
        return Err(shape_changed("no package entries"));
    }
    Ok(packages)
}

/// Builds the package of one catalog entry. An entry that `recipe` rejects opens in the browser.
fn package_of(entry: RawEntry, fallback: &Url) -> Package {
    let page_url = Url::parse(&entry.repo_url).unwrap_or_else(|_| fallback.clone());
    let source = recipe(&entry).map_or_else(
        || PackageSource::Web(page_url.clone()),
        PackageSource::Recipe,
    );
    let deprecated = entry.deprecation.is_some();
    let deprecation_message = entry
        .deprecation
        .as_ref()
        .and_then(|deprecation| deprecation.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let asset_base = Url::parse(INDEX_URL).ok();
    let screenshots = entry
        .images
        .as_ref()
        .and_then(|img| img.get("detailImages"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|path| {
                    asset_base
                        .as_ref()
                        .and_then(|base| base.join(path).ok())
                        .map_or_else(|| path.to_owned(), String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    let changelog = entry
        .changelog
        .as_ref()
        .and_then(|c| c.get("markdownSource").or_else(|| c.get("text")))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let mut release_dates: Vec<String> = entry
        .version
        .iter()
        .filter_map(|version| version.release_date.clone())
        .collect();
    release_dates.sort_unstable_by(|a, b| b.cmp(a));
    release_dates.dedup();

    Package {
        categories: vec![entry.r#type],
        id: entry.id,
        name: entry.name,
        summary: entry.summary,
        author: entry.author,
        updated: None,
        page_url,
        source,
        niconi_commons_id: entry.niconi_commons_id,
        tags: entry.tags,
        deprecated,
        deprecation_message,
        dependencies: entry.dependencies,
        licenses: entry.licenses.into_iter().map(license_of).collect(),
        changelog,
        screenshots,
        release_dates,
    }
}

/// Parses one catalog entry, a JSON object of the `index.json` shape, into a package.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `text` is not such an entry
/// and `ProviderError::InvalidUrl` for an invalid page URL.
pub fn parse_aviutl2_manifest(text: &str) -> Result<Package, ProviderError> {
    let fallback =
        Url::parse(CATALOG_PAGE).map_err(|_| ProviderError::InvalidUrl(CATALOG_PAGE.to_owned()))?;
    let entry: RawEntry = serde_json::from_str(text).map_err(|e| shape_changed(e.to_string()))?;
    Ok(package_of(entry, &fallback))
}

/// The portable ZIP lies beside the setup program: `aviutl2_<version>.zip`.
/// The entry lists the hash of `aviutl2.exe`, which the ZIP holds as well.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `bytes` is not the expected document,
/// the editor entry is missing, or the entry has no direct URL,
/// and `ProviderError::InvalidUrl` when the ZIP URL cannot be joined.
pub fn parse_aviutl2_app(bytes: &[u8]) -> Result<AppVersion, ProviderError> {
    let entries = parse_entries(bytes)?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == CORE_ID)
        .ok_or_else(|| shape_changed("editor entry is missing"))?;
    let setup = entry
        .installer
        .source
        .get("direct")
        .and_then(Value::as_str)
        .and_then(|url| Url::parse(url).ok())
        .ok_or_else(|| shape_changed("editor entry has no direct url"))?;
    let name = format!("aviutl2_{}.zip", entry.latest_version);
    let url = setup
        .join(&name)
        .map_err(|_| ProviderError::InvalidUrl(name.clone()))?;
    let published_at = entry
        .version
        .iter()
        .find(|version| version.version == entry.latest_version)
        .and_then(|version| version.release_date.clone());
    Ok(AppVersion {
        version: entry.latest_version.clone(),
        prerelease: false,
        published_at,
        artifacts: vec![Artifact {
            name,
            url,
            size: None,
            hash: None,
        }],
        checks: checks(entry)
            .into_iter()
            .filter(|check| check.location.root == Root::App)
            .collect(),
    })
}

/// Sort key of a version label: `v` versions before `beta` versions, then numbers, then suffix.
fn version_key(label: &str) -> (bool, Vec<u32>, String) {
    let release = label.starts_with('v');
    let body = label.trim_start_matches("beta").trim_start_matches('v');
    let suffix: String = body
        .chars()
        .rev()
        .take_while(char::is_ascii_lowercase)
        .collect();
    let numbers = body
        .trim_end_matches(|c: char| c.is_ascii_lowercase())
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    (release, numbers, suffix)
}

/// One app version per portable ZIP of the history page, except the latest version of `index`,
/// newest first. The hash of `aviutl2.exe` comes from `index` when it lists the version.
///
/// # Errors
///
/// Returns `ProviderError::CatalogShapeChanged` when `index` is not the expected document,
/// the editor entry is missing, or `page` lists no ZIP, and `ProviderError::InvalidUrl`
/// for an invalid URL.
pub fn parse_aviutl2_history(index: &[u8], page: &[u8]) -> Result<Vec<AppVersion>, ProviderError> {
    let entries = parse_entries(index)?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == CORE_ID)
        .ok_or_else(|| shape_changed("editor entry is missing"))?;
    let mut rows = parse_rows(page, HISTORY_ROW)?;
    if rows.is_empty() {
        return Err(shape_changed("history page lists no AviUtl2 archive"));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(version_key(&row.0)));
    Ok(rows
        .into_iter()
        .filter(|(version, _)| *version != entry.latest_version)
        .map(|(version, url)| {
            let checks = checks_of(entry, &version)
                .into_iter()
                .filter(|check| check.location.root == Root::App)
                .collect();
            AppVersion {
                artifacts: vec![direct_artifact(&url)],
                published_at: entry
                    .version
                    .iter()
                    .find(|raw| raw.version == version)
                    .and_then(|raw| raw.release_date.clone()),
                version,
                prerelease: false,
                checks,
            }
        })
        .collect())
}

pub struct Aviutl2Provider<H> {
    http: H,
}

impl<H: Http> Aviutl2Provider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }

    async fn index(&self) -> Result<Vec<u8>, ProviderError> {
        Ok(self.http.get(&parse_url(INDEX_URL)?).await?)
    }
}

impl<H: Http> Provider for Aviutl2Provider<H> {
    fn kind(&self) -> Kind {
        Kind::Aviutl2
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        let index = self.index().await?;
        let mut versions = vec![parse_aviutl2_app(&index)?];
        match history::fetch_page(&self.http)
            .await
            .and_then(|page| parse_aviutl2_history(&index, &page))
        {
            Ok(older) => versions.extend(older),
            Err(error) => warn!(%error, "aviutl2 history unavailable"),
        }
        Ok(versions)
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        parse_aviutl2_catalog(&self.index().await?)
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        if selection.host != Host::Windows {
            return Err(ProviderError::UnsupportedHost {
                kind: Kind::Aviutl2,
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
            Origin::Direct(url) => direct_artifact(url),
            Origin::GithubRelease {
                owner,
                repo,
                pattern,
            } => {
                let Some(artifacts) = latest_artifacts(&self.http, owner, repo).await? else {
                    return Ok(browser());
                };
                let pattern = Regex::new(pattern.as_deref().unwrap_or_default()).map_err(|e| {
                    shape_changed(format!("invalid asset pattern of {}: {e}", package.id))
                })?;
                let Some(artifact) = artifacts
                    .into_iter()
                    .find(|artifact| pattern.is_match(&artifact.name))
                else {
                    return Ok(browser());
                };
                artifact
            }
        };
        Ok(finish(package, recipe, artifact))
    }
}
