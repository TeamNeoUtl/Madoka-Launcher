use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tracing::{debug, info};
use uuid::Uuid;

use crate::archive::{Archive, Layout};
use crate::model::ensure_single_component;
use crate::{
    Artifact, Error, Fetcher, Hash, InstanceId, InstanceStore, Kind, Package, PackageSource,
    PluginRecord, Progress, ProgressSink, is_allowed_url,
};

const DOWNLOADS_DIR: &str = "cache/downloads";
const PART_SUFFIX: &str = ".part";

/// A downloaded file. Removed on drop when it is not stored in the cache.
struct Downloaded {
    path: PathBuf,
    temporary: bool,
}

impl Drop for Downloaded {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub struct Installer<F> {
    root: PathBuf,
    store: InstanceStore,
    fetcher: F,
}

impl<F: Fetcher> Installer<F> {
    pub fn new(root: impl Into<PathBuf>, fetcher: F) -> Self {
        let root = root.into();
        Self {
            store: InstanceStore::new(&root),
            root,
            fetcher,
        }
    }

    pub fn fetcher(&self) -> &F {
        &self.fetcher
    }

    /// Extracts each artifact into `<instance>/app/`.
    pub async fn install_app(
        &self,
        id: InstanceId,
        artifacts: &[Artifact],
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let instance = self.store.load(id)?;
        info!(%id, artifacts = artifacts.len(), "install app");
        for artifact in artifacts {
            let download = self.download(artifact, progress).await?;
            let mut archive = Archive::open(&download.path, Layout::Tree)?;
            debug!(name = %artifact.name, files = archive.files().len(), "extract app archive");
            archive.extract(&instance.app_dir())?;
        }
        info!(%id, "app installed");
        Ok(())
    }

    /// Replaces the record with the same `package.id`, then writes the new record to `instance.toml`.
    /// The archive is validated before any existing file is removed.
    pub async fn install_package(
        &self,
        id: InstanceId,
        package: &Package,
        version: &str,
        artifact: &Artifact,
        progress: &dyn ProgressSink,
    ) -> Result<PluginRecord, Error> {
        let mut instance = self.store.load(id)?;
        let plugin_dir = plugin_dir(instance.config.kind, package)?;
        info!(%id, package = %package.id, version, "install package");
        let download = self.download(artifact, progress).await?;
        let mut archive = Archive::open(&download.path, Layout::Flat)?;
        let app_dir = instance.app_dir();

        if let Some(position) = position_of(&instance.config.plugins, &package.id) {
            delete_files(&app_dir, &instance.config.plugins[position].files)?;
            instance.config.plugins.remove(position);
            self.store.save(&instance)?;
        }

        archive.extract(&app_dir.join(&plugin_dir))?;
        let record = PluginRecord {
            id: package.id.clone(),
            version: version.to_owned(),
            source: package.source.label(),
            files: archive
                .files()
                .into_iter()
                .map(|file| format!("{plugin_dir}/{file}"))
                .collect(),
        };
        instance.config.plugins.push(record.clone());
        self.store.save(&instance)?;
        info!(%id, package = %package.id, files = record.files.len(), "package installed");
        Ok(record)
    }

    /// Removes every file listed in the record, then the record.
    pub fn remove_package(&self, id: InstanceId, plugin_id: &str) -> Result<(), Error> {
        let mut instance = self.store.load(id)?;
        info!(%id, package = plugin_id, "remove package");
        let position = position_of(&instance.config.plugins, plugin_id)
            .ok_or_else(|| Error::PluginNotFound(plugin_id.to_owned()))?;
        delete_files(
            &instance.app_dir(),
            &instance.config.plugins[position].files,
        )?;
        instance.config.plugins.remove(position);
        self.store.save(&instance)
    }

    async fn download(
        &self,
        artifact: &Artifact,
        progress: &dyn ProgressSink,
    ) -> Result<Downloaded, Error> {
        debug!(name = %artifact.name, url = %artifact.url, hashed = artifact.hash.is_some(), "download requested");
        if !is_allowed_url(&artifact.url) {
            return Err(Error::HostNotAllowed(artifact.url.to_string()));
        }
        let dir = self.root.join(DOWNLOADS_DIR);
        fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        debug!(dir = %dir.display(), "download directory ready");

        let Some(expected) = &artifact.hash else {
            let path = dir.join(format!("{}{PART_SUFFIX}", Uuid::new_v4()));
            let download = Downloaded {
                path,
                temporary: true,
            };
            self.fetch_timed(artifact, &download.path, progress).await?;
            return Ok(download);
        };

        let cached = dir.join(&expected.hex);
        if verify(&cached, expected, &artifact.name).is_ok() {
            info!(name = %artifact.name, path = %cached.display(), "download cache hit");
            let size = fs::metadata(&cached).map_or(0, |meta| meta.len());
            progress.report(Progress {
                done: size,
                total: Some(size),
            });
            return Ok(Downloaded {
                path: cached,
                temporary: false,
            });
        }
        let part = Downloaded {
            path: dir.join(format!("{}{PART_SUFFIX}", expected.hex)),
            temporary: true,
        };
        self.fetch_timed(artifact, &part.path, progress).await?;
        verify(&part.path, expected, &artifact.name)?;
        debug!(name = %artifact.name, sha256 = %expected.hex, "hash verified");
        fs::rename(&part.path, &cached).map_err(|e| Error::io(&cached, e))?;
        Ok(Downloaded {
            path: cached,
            temporary: false,
        })
    }
}

impl<F: Fetcher> Installer<F> {
    async fn fetch_timed(
        &self,
        artifact: &Artifact,
        dest: &Path,
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let started = Instant::now();
        self.fetcher.fetch(&artifact.url, dest, progress).await?;
        info!(
            name = %artifact.name,
            bytes = fs::metadata(dest).map_or(0, |meta| meta.len()),
            elapsed_ms = started.elapsed().as_millis(),
            "downloaded"
        );
        Ok(())
    }
}

fn verify(path: &Path, expected: &Hash, name: &str) -> Result<(), Error> {
    let actual = Hash::of_file(expected.algo, path).map_err(|e| Error::io(path, e))?;
    if actual.hex == expected.hex {
        Ok(())
    } else {
        Err(Error::HashMismatch {
            name: name.to_owned(),
            expected: expected.hex.clone(),
            actual: actual.hex,
        })
    }
}

fn plugin_dir(kind: Kind, package: &Package) -> Result<String, Error> {
    match (kind, &package.source) {
        (Kind::Ymm4, PackageSource::GithubRepo { repo, .. }) => {
            ensure_single_component(repo)?;
            Ok(format!("user/plugin/{repo}"))
        }
        _ => Err(Error::PluginUnsupported(kind)),
    }
}

fn position_of(plugins: &[PluginRecord], id: &str) -> Option<usize> {
    plugins.iter().position(|plugin| plugin.id == id)
}

/// Removes `files` (relative to `app_dir`), ignoring files that are already absent,
/// then removes the directories left empty below `app_dir`.
fn delete_files(app_dir: &Path, files: &[String]) -> Result<(), Error> {
    for file in files {
        let path = app_dir.join(file);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(&path, e)),
        }
        let mut parent = path.parent();
        while let Some(dir) = parent {
            if dir == app_dir || fs::remove_dir(dir).is_err() {
                break;
            }
            parent = dir.parent();
        }
    }
    Ok(())
}
