use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use launcher_core::{Kind, Package};

/// A cache file younger than this is used without a request.
pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

const CATALOG_DIR: &str = "cache/catalog";

fn path_of(root: &Path, kind: Kind) -> PathBuf {
    let name = format!("{kind:?}").to_ascii_lowercase();
    root.join(CATALOG_DIR).join(format!("{name}.json"))
}

fn read(root: &Path, kind: Kind) -> Option<(Vec<Package>, SystemTime)> {
    let path = path_of(root, kind);
    let modified = fs::metadata(&path).ok()?.modified().ok()?;
    let packages = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
    Some((packages, modified))
}

/// Packages of a cache file that is younger than `CACHE_TTL`.
pub fn load_fresh(root: &Path, kind: Kind) -> Option<Vec<Package>> {
    let (packages, modified) = read(root, kind)?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    (age < CACHE_TTL).then_some(packages)
}

/// Packages of the cache file whatever its age.
pub fn load_any(root: &Path, kind: Kind) -> Option<Vec<Package>> {
    read(root, kind).map(|(packages, _)| packages)
}

/// Writes through a temporary file, so that a failed write keeps the previous cache file.
pub fn store(root: &Path, kind: Kind, packages: &[Package]) -> std::io::Result<()> {
    let path = path_of(root, kind);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text =
        serde_json::to_vec(packages).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)
}
