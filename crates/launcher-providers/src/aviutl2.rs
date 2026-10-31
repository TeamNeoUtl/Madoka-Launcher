use launcher_core::{
    AppSelection, AppVersion, Artifact, FileCheck, Hash, Host, InstallPlan, Kind, Location, Origin,
    Package, PackageSource, Recipe, Root, Step,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::recipe::{direct_artifact, finish};
use crate::release::{latest_artifacts, parse_url};
use crate::{Http, Provider, ProviderError};

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json";
const CORE_ID: &str = "Kenkun.AviUtlExEdit2";
const CATALOG_PAGE: &str = "https://github.com/Neosku/aviutl2-catalog-data";

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
    let Some(version) = entry
        .version
        .iter()
        .find(|version| version.version == entry.latest_version)
    else {
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
pub fn parse_aviutl2_catalog(bytes: &[u8]) -> Result<Vec<Package>, ProviderError> {
    let fallback =
        Url::parse(CATALOG_PAGE).map_err(|_| ProviderError::InvalidUrl(CATALOG_PAGE.to_owned()))?;
    let packages: Vec<Package> = parse_entries(bytes)?
        .into_iter()
        .filter(|entry| entry.id != CORE_ID)
        .map(|entry| {
            let page_url = Url::parse(&entry.repo_url).unwrap_or_else(|_| fallback.clone());
            let source = recipe(&entry).map_or_else(
                || PackageSource::Web(page_url.clone()),
                PackageSource::Recipe,
            );
            Package {
                categories: vec![entry.r#type],
                id: entry.id,
                name: entry.name,
                summary: entry.summary,
                author: entry.author,
                updated: None,
                page_url,
                source,
            }
        })
        .collect();
    if packages.is_empty() {
        return Err(shape_changed("no package entries"));
    }
    Ok(packages)
}

/// The portable ZIP lies beside the setup program: `aviutl2_<version>.zip`.
/// The entry lists the hash of `aviutl2.exe`, which the ZIP holds as well.
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

pub struct Aviutl2Provider<H> {
    http: H,
}

impl<H: Http> Aviutl2Provider<H> {
    pub fn new(http: H) -> Self {
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
        Ok(vec![parse_aviutl2_app(&self.index().await?)?])
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
