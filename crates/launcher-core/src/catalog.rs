use std::fs::File;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384};
use url::Url;
use xxhash_rust::xxh3::Xxh3;

use crate::{Edition, Host};

const SHA256_DIGEST_PREFIX: &str = "sha256:";
const SHA384_SRI_PREFIX: &str = "sha384-";
const SHA384_BYTES: usize = 48;
const XXH3_128_HEX_DIGITS: usize = 32;
const READ_CHUNK: usize = 64 * 1024;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashAlgo {
    Sha256,
    Sha384,
    Xxh3_128,
}

/// `hex` holds lowercase hexadecimal digits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash {
    pub algo: HashAlgo,
    pub hex: String,
}

impl Hash {
    /// Parses the `sha256:<64 hex digits>` form that GitHub reports per release asset.
    #[must_use]
    pub fn parse_digest(digest: &str) -> Option<Self> {
        let hex = digest.strip_prefix(SHA256_DIGEST_PREFIX)?;
        let valid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        valid.then(|| Self {
            algo: HashAlgo::Sha256,
            hex: hex.to_ascii_lowercase(),
        })
    }

    /// Parses the `sha384-<base64>` Subresource Integrity form of the APM data.
    #[must_use]
    pub fn parse_sri(text: &str) -> Option<Self> {
        let encoded = text.strip_prefix(SHA384_SRI_PREFIX)?;
        let bytes = STANDARD.decode(encoded).ok()?;
        (bytes.len() == SHA384_BYTES).then(|| Self {
            algo: HashAlgo::Sha384,
            hex: to_hex(bytes.iter()),
        })
    }

    /// Parses the 32 hexadecimal digits of an XXH3-128 value.
    #[must_use]
    pub fn parse_xxh3_128(text: &str) -> Option<Self> {
        let valid =
            text.len() == XXH3_128_HEX_DIGITS && text.bytes().all(|b| b.is_ascii_hexdigit());
        valid.then(|| Self {
            algo: HashAlgo::Xxh3_128,
            hex: text.to_ascii_lowercase(),
        })
    }

    /// # Errors
    ///
    /// Returns the `std::io::Error` of opening or reading `path`.
    pub fn of_file(algo: HashAlgo, path: &Path) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        let mut buffer = vec![0u8; READ_CHUNK];
        let hex = match algo {
            HashAlgo::Sha256 => {
                let mut hasher = Sha256::new();
                read_chunks(&mut file, &mut buffer, |chunk| hasher.update(chunk))?;
                to_hex(hasher.finalize().iter())
            }
            HashAlgo::Sha384 => {
                let mut hasher = Sha384::new();
                read_chunks(&mut file, &mut buffer, |chunk| hasher.update(chunk))?;
                to_hex(hasher.finalize().iter())
            }
            HashAlgo::Xxh3_128 => {
                let mut hasher = Xxh3::new();
                read_chunks(&mut file, &mut buffer, |chunk| hasher.update(chunk))?;
                format!("{:032x}", hasher.digest128())
            }
        };
        Ok(Self { algo, hex })
    }
}

fn to_hex<'a>(bytes: impl Iterator<Item = &'a u8>) -> String {
    bytes.fold(String::new(), |mut hex, &byte| {
        hex.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
        hex
    })
}

fn read_chunks<R>(
    file: &mut File,
    buffer: &mut [u8],
    mut consume: impl FnMut(&[u8]) -> R,
) -> std::io::Result<()> {
    loop {
        let read = file.read(buffer)?;
        if read == 0 {
            return Ok(());
        }
        consume(&buffer[..read]);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub name: String,
    pub url: Url,
    pub size: Option<u64>,
    pub hash: Option<Hash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVersion {
    pub version: String,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub artifacts: Vec<Artifact>,
    /// Hashes of files that must exist under the app directory after the extraction.
    pub checks: Vec<FileCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSelection {
    pub host: Host,
    pub edition: Edition,
}

/// Base directory of a `Location`.
/// `App` is the directory of the editor executable, `Data` is `<App>/data`,
/// `Tmp` is the working directory of one installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Root {
    Tmp,
    App,
    Data,
}

/// `rel` uses `/` and has no `.` or `..` component. It is empty for the root itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub root: Root,
    pub rel: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    /// Extracts the downloaded zip into `Root::Tmp`.
    Extract,
    /// Copies a file into the directory `to`, or the content of a directory into `to`.
    Copy { from: Location, to: Location },
    /// Copies an extracted file or directory to `<App>/<name>`.
    /// The source is `<archive_dir>/<last component of name>` below `Root::Tmp`.
    /// Without `archive_dir`, or when that path is absent, the source is the unique
    /// entry whose path ends with `name`, then the unique entry with the same last component.
    Place {
        name: String,
        archive_dir: Option<String>,
        directory: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCheck {
    pub location: Location,
    pub hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    Direct(Url),
    GithubRelease {
        owner: String,
        repo: String,
        /// Regular expression for the asset name. Without it the single `.zip` asset is used.
        pattern: Option<String>,
    },
}

impl Origin {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Direct(url) => url.to_string(),
            Self::GithubRelease { owner, repo, .. } => format!("github:{owner}/{repo}"),
        }
    }
}

/// Installation procedure that a catalog entry describes. The download URL is resolved from `origin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub version: String,
    pub origin: Origin,
    /// Hash of the downloaded archive.
    pub hash: Option<Hash>,
    pub steps: Vec<Step>,
    /// Hashes of the files that exist after the steps ran.
    pub checks: Vec<FileCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    /// `.ymme` asset of the latest release.
    GithubRepo {
        owner: String,
        repo: String,
    },
    /// Page that the user opens in the browser.
    Web(Url),
    Recipe(Recipe),
}

impl PackageSource {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::GithubRepo { owner, repo } => format!("github:{owner}/{repo}"),
            Self::Web(url) => url.to_string(),
            Self::Recipe(recipe) => recipe.origin.label(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub categories: Vec<String>,
    pub updated: Option<String>,
    pub page_url: Url,
    pub source: PackageSource,
    #[serde(default)]
    pub niconi_commons_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub licenses: Vec<String>,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

impl Package {
    #[must_use]
    pub fn simple(
        id: impl Into<String>,
        name: impl Into<String>,
        page_url: Url,
        source: PackageSource,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: String::new(),
            author: String::new(),
            categories: Vec::new(),
            updated: None,
            page_url,
            source,
            niconi_commons_id: None,
            tags: Vec::new(),
            deprecated: false,
            dependencies: Vec::new(),
            licenses: Vec::new(),
            changelog: None,
            screenshots: Vec::new(),
        }
    }

    /// Version string that `PluginRecord::version` holds after the installation.
    /// `None` when the catalog does not tell it.
    #[must_use]
    pub fn latest_version(&self) -> Option<&str> {
        match &self.source {
            PackageSource::Recipe(recipe) => Some(&recipe.version),
            PackageSource::GithubRepo { .. } => self.updated.as_deref(),
            PackageSource::Web(_) => None,
        }
    }
}

/// Download of one archive followed by `steps` and `checks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPlan {
    pub artifact: Artifact,
    pub version: String,
    pub steps: Vec<Step>,
    pub checks: Vec<FileCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPlan {
    Download(Vec<Artifact>),
    Staged(StagedPlan),
    OpenInBrowser(Url),
}

impl InstallPlan {
    /// `false` when the installation writes files that no hash covers.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        match self {
            Self::Download(artifacts) => artifacts.iter().all(|artifact| artifact.hash.is_some()),
            Self::Staged(plan) => plan.artifact.hash.is_some() || !plan.checks.is_empty(),
            Self::OpenInBrowser(_) => true,
        }
    }
}
