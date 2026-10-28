use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use url::Url;

use crate::{Edition, Host};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgo {
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hash {
    pub algo: HashAlgo,
    pub hex: String,
}

impl Hash {
    /// Parses the `sha256:<64 hex digits>` form that GitHub reports per release asset.
    pub fn parse_digest(digest: &str) -> Option<Self> {
        let hex = digest.strip_prefix("sha256:")?;
        let valid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        valid.then(|| Self {
            algo: HashAlgo::Sha256,
            hex: hex.to_ascii_lowercase(),
        })
    }
}

impl Hash {
    pub fn of_file(algo: HashAlgo, path: &Path) -> std::io::Result<Self> {
        let HashAlgo::Sha256 = algo;
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let hex = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self { algo, hex })
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSelection {
    pub host: Host,
    pub edition: Edition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    GithubRepo { owner: String, repo: String },
    Web(Url),
}

impl PackageSource {
    pub fn label(&self) -> String {
        match self {
            Self::GithubRepo { owner, repo } => format!("github:{owner}/{repo}"),
            Self::Web(url) => url.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub categories: Vec<String>,
    pub updated: Option<String>,
    pub page_url: Url,
    pub source: PackageSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPlan {
    Download(Vec<Artifact>),
    OpenInBrowser(Url),
}
