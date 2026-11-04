use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tracing::{debug, info};
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_64;

use crate::archive::{Archive, Created, Layout};
use crate::model::ensure_single_component;
use crate::stage::{self, Roots};
use crate::swap::Swap;
use crate::{
    Artifact, Error, Fetcher, FileCheck, Hash, Host, Instance, InstanceId, InstanceStore, Kind,
    Package, PackageSource, PluginRecord, Progress, ProgressSink, Recipe, StagedPlan, Step,
    find_executable, is_allowed_url,
};

const DOWNLOADS_DIR: &str = "cache/downloads";
const TMP_DIR: &str = "cache/tmp";
const STAGED_DIR: &str = "new";
const BACKUP_DIR: &str = "old";
const PART_SUFFIX: &str = ".part";

/// Working directory of one installation. Removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn create(root: &Path) -> Result<Self, Error> {
        let path = root.join(TMP_DIR).join(Uuid::new_v4().to_string());
        fs::create_dir_all(&path).map_err(|e| Error::io(&path, e))?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Archive and procedure of one staged installation.
struct Staging<'a> {
    version: &'a str,
    steps: &'a [Step],
    checks: &'a [FileCheck],
    archive: &'a Path,
    name: &'a str,
}

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

    pub const fn fetcher(&self) -> &F {
        &self.fetcher
    }

    /// Extracts each artifact into `<instance>/app/`.
    ///
    /// # Errors
    ///
    /// Returns the errors of `install_app_checked` with an empty `checks`.
    pub async fn install_app(
        &self,
        id: InstanceId,
        artifacts: &[Artifact],
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        self.install_app_checked(id, artifacts, &[], progress).await
    }

    /// Like `install_app`, then verifies `checks` against the extracted files.
    /// `AviUtl` archives lose the directory that holds every entry.
    /// `AviUtl2` gets `data/` beside the executable, which makes the instance self-contained.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, an artifact fails to download
    /// or to match its hash, an archive entry is rejected or cannot be written,
    /// or a file in `checks` is missing or differs in hash.
    pub async fn install_app_checked(
        &self,
        id: InstanceId,
        artifacts: &[Artifact],
        checks: &[FileCheck],
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let instance = self.store.load(id)?;
        let kind = instance.config.kind;
        let app_dir = instance.app_dir();
        info!(%id, artifacts = artifacts.len(), "install app");
        for artifact in artifacts {
            let download = self.download(artifact, progress).await?;
            let mut archive = Archive::open(&download.path, layout_of(kind))?;
            debug!(name = %artifact.name, files = archive.files().len(), "extract app archive");
            archive.extract(&app_dir)?;
        }
        verify_app(kind, &app_dir, checks)?;
        info!(%id, "app installed");
        Ok(())
    }

    /// Replaces the app files with the content of `artifacts`, then verifies `checks`.
    /// Every archive is extracted into a working directory first, so a failing download or a
    /// rejected entry leaves the app directory untouched. A failing copy or check restores the
    /// files that the copy replaced and removes the files that it created.
    /// When the new archive keeps its executable in another directory than the installed app,
    /// the files go to the directory of the installed executable.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, an artifact fails to download
    /// or to match its hash, an archive entry is rejected or cannot be written,
    /// a file cannot be replaced, or a file in `checks` is missing or differs in hash.
    pub async fn update_app_checked(
        &self,
        id: InstanceId,
        artifacts: &[Artifact],
        checks: &[FileCheck],
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let instance = self.store.load(id)?;
        let kind = instance.config.kind;
        let app_dir = instance.app_dir();
        let tmp = TempDir::create(&self.root)?;
        let staged = tmp.0.join(STAGED_DIR);
        fs::create_dir_all(&staged).map_err(|e| Error::io(&staged, e))?;
        info!(%id, artifacts = artifacts.len(), "update app");
        for artifact in artifacts {
            let download = self.download(artifact, progress).await?;
            let mut archive = Archive::open(&download.path, layout_of(kind))?;
            debug!(name = %artifact.name, files = archive.files().len(), "extract app archive");
            archive.extract(&staged)?;
        }
        let (source, dest) = match (exe_dir(kind, &staged), exe_dir(kind, &app_dir)) {
            (Some(new), Some(old)) if new != old => (staged.join(new), app_dir.join(old)),
            _ => (staged, app_dir.clone()),
        };
        let swap = Swap::apply(&source, &dest, &tmp.0.join(BACKUP_DIR))?;
        if let Err(error) = verify_app(kind, &app_dir, checks) {
            swap.roll_back();
            return Err(error);
        }
        info!(%id, "app updated");
        Ok(())
    }

    /// Replaces the record with the same `package.id`, then writes the new record to `instance.toml`.
    /// The archive is validated before any existing file is removed.
    /// When one directory holds every archive entry, the files are written without that directory.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, the kind has no plugin directory,
    /// the artifact fails to download or to match its hash, an archive entry is rejected,
    /// a file cannot be removed or written, or `instance.toml` cannot be saved.
    pub async fn install_package(
        &self,
        id: InstanceId,
        package: &Package,
        version: &str,
        artifact: &Artifact,
        progress: &dyn ProgressSink,
    ) -> Result<PluginRecord, Error> {
        let instance = self.store.load(id)?;
        let plugin_dir = plugin_dir(instance.config.kind, package)?;
        info!(%id, package = %package.id, version, "install package");
        let download = self.download(artifact, progress).await?;
        self.place_package(instance, &plugin_dir, package, version, &download.path)
    }

    /// Installs the archive at `archive` like `install_package`, without downloading or hashing.
    /// The caller vouches for the file: it comes from the user's own browser download.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, the kind has no plugin directory,
    /// an archive entry is rejected, a file cannot be removed or written,
    /// or `instance.toml` cannot be saved.
    pub fn install_package_file(
        &self,
        id: InstanceId,
        package: &Package,
        version: &str,
        archive: &Path,
    ) -> Result<PluginRecord, Error> {
        let instance = self.store.load(id)?;
        let plugin_dir = plugin_dir(instance.config.kind, package)?;
        info!(%id, package = %package.id, version, archive = %archive.display(), "install package file");
        self.place_package(instance, &plugin_dir, package, version, archive)
    }

    fn place_package(
        &self,
        mut instance: Instance,
        plugin_dir: &str,
        package: &Package,
        version: &str,
        archive: &Path,
    ) -> Result<PluginRecord, Error> {
        let id = instance.config.id;
        let mut archive = Archive::open(archive, Layout::TreeWithoutRoot)?;
        let app_dir = instance.app_dir();

        if let Some(position) = position_of(&instance.config.plugins, &package.id) {
            delete_files(&app_dir, &app_dir, &instance.config.plugins[position].files)?;
            instance.config.plugins.remove(position);
            self.store.save(&instance)?;
        }

        archive.extract(&app_dir.join(plugin_dir))?;
        let record = PluginRecord {
            id: package.id.clone(),
            version: version.to_owned(),
            source: package.source.label(),
            files: archive
                .files()
                .into_iter()
                .map(|file| format!("{plugin_dir}/{file}"))
                .collect(),
            paused: false,
        };
        instance.config.plugins.push(record.clone());
        self.store.save(&instance)?;
        info!(%id, package = %package.id, files = record.files.len(), "package installed");
        Ok(record)
    }

    /// Downloads `plan.artifact`, runs `plan.steps`, then verifies `plan.checks`.
    /// A failing step or check removes every file that the task wrote.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, `plan.artifact.name` is not a single
    /// path component, the artifact fails to download or to match its hash, the kind has no
    /// catalog by location, a step or a check in `plan` fails, or `instance.toml` cannot be saved.
    pub async fn install_staged(
        &self,
        id: InstanceId,
        package: &Package,
        plan: &StagedPlan,
        progress: &dyn ProgressSink,
    ) -> Result<PluginRecord, Error> {
        let instance = self.store.load(id)?;
        info!(%id, package = %package.id, version = %plan.version, "install staged package");
        ensure_single_component(&plan.artifact.name)?;
        let download = self.download(&plan.artifact, progress).await?;
        let staging = Staging {
            version: &plan.version,
            steps: &plan.steps,
            checks: &plan.checks,
            archive: &download.path,
            name: &plan.artifact.name,
        };
        self.place_staged(instance, package, &staging)
    }

    /// Installs the archive at `archive` with the steps and checks of `recipe`, without downloading
    /// or hashing the archive. The caller vouches for the file: it comes from the user's own
    /// browser download. The `checks` of the recipe still apply.
    ///
    /// # Errors
    ///
    /// Returns an `Error` when the instance cannot be loaded, the name of `archive` is not a single
    /// path component, the kind has no catalog by location, a step or a check fails,
    /// or `instance.toml` cannot be saved.
    pub fn install_recipe_file(
        &self,
        id: InstanceId,
        package: &Package,
        recipe: &Recipe,
        archive: &Path,
    ) -> Result<PluginRecord, Error> {
        let instance = self.store.load(id)?;
        let name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::UnsafePath(archive.display().to_string()))?;
        ensure_single_component(name)?;
        info!(%id, package = %package.id, version = %recipe.version, archive = %archive.display(), "install recipe file");
        let staging = Staging {
            version: &recipe.version,
            steps: &recipe.steps,
            checks: &recipe.checks,
            archive,
            name,
        };
        self.place_staged(instance, package, &staging)
    }

    fn place_staged(
        &self,
        mut instance: Instance,
        package: &Package,
        staging: &Staging<'_>,
    ) -> Result<PluginRecord, Error> {
        let id = instance.config.id;
        let app_dir = instance.app_dir();
        let tmp = TempDir::create(&self.root)?;
        let staged = tmp.0.join(staging.name);
        fs::copy(staging.archive, &staged).map_err(|e| Error::io(&staged, e))?;
        let roots = Roots::locate(instance.config.kind, &app_dir, Some(&tmp.0))?;

        if let Some(position) = position_of(&instance.config.plugins, &package.id) {
            let floor = removal_floor(&instance);
            delete_files(&app_dir, &floor, &instance.config.plugins[position].files)?;
            instance.config.plugins.remove(position);
            self.store.save(&instance)?;
        }

        let mut created = Created::default();
        let result = stage::run(&roots, &staged, staging.steps, &mut created)
            .and_then(|()| stage::verify(&roots, staging.checks));
        if let Err(error) = result {
            created.roll_back();
            return Err(error);
        }
        let record = PluginRecord {
            id: package.id.clone(),
            version: staging.version.to_owned(),
            source: package.source.label(),
            files: relative_files(&app_dir, &created.files)?,
            paused: false,
        };
        instance.config.plugins.push(record.clone());
        self.store.save(&instance)?;
        info!(%id, package = %package.id, files = record.files.len(), "staged package installed");
        Ok(record)
    }

    /// Removes every file listed in the record, then the record.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` when the instance does not exist,
    /// `Error::PluginNotFound` when `plugin_id` has no record,
    /// and `Error::Io` when a file cannot be removed or `instance.toml` cannot be saved.
    pub fn remove_package(&self, id: InstanceId, plugin_id: &str) -> Result<(), Error> {
        let mut instance = self.store.load(id)?;
        info!(%id, package = plugin_id, "remove package");
        let position = position_of(&instance.config.plugins, plugin_id)
            .ok_or_else(|| Error::PluginNotFound(plugin_id.to_owned()))?;
        delete_files(
            &instance.app_dir(),
            &removal_floor(&instance),
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
        if artifact.url.scheme() == "http" && artifact.hash.is_none() {
            return Err(Error::HashRequired(artifact.url.to_string()));
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

/// `AviUtl` archives lose the directory that holds every entry.
const fn layout_of(kind: Kind) -> Layout {
    if matches!(kind, Kind::Aviutl) {
        Layout::TreeWithoutRoot
    } else {
        Layout::Tree
    }
}

/// `AviUtl` and the other Windows-only kinds ship `.exe` files on every host.
fn executable_host(kind: Kind) -> Host {
    if kind.supported_on(Host::current()) {
        Host::current()
    } else {
        Host::Windows
    }
}

/// Directory of the executable below `dir`, relative to `dir`. `None` when there is no executable.
fn exe_dir(kind: Kind, dir: &Path) -> Option<PathBuf> {
    let exe = find_executable(dir, kind, executable_host(kind)).ok()?;
    exe.parent()?.strip_prefix(dir).ok().map(Path::to_path_buf)
}

/// Verifies `checks` against the app files. `AviUtl2` gets `data/` beside the executable,
/// which makes the instance self-contained.
fn verify_app(kind: Kind, app_dir: &Path, checks: &[FileCheck]) -> Result<(), Error> {
    if !matches!(kind, Kind::Aviutl | Kind::Aviutl2) {
        return Ok(());
    }
    let roots = Roots::locate(kind, app_dir, None)?;
    if kind == Kind::Aviutl2 {
        fs::create_dir_all(&roots.data).map_err(|e| Error::io(&roots.data, e))?;
    }
    stage::verify(&roots, checks)
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
        (Kind::Ymm4, PackageSource::Web(_)) => Ok(format!(
            "user/plugin/web-{:016x}",
            xxh3_64(package.id.as_bytes())
        )),
        _ => Err(Error::PluginUnsupported(kind)),
    }
}

fn position_of(plugins: &[PluginRecord], id: &str) -> Option<usize> {
    plugins.iter().position(|plugin| plugin.id == id)
}

/// Removes `files` (relative to `app_dir`), ignoring files that are already absent,
/// then removes the directories left empty below `floor`. `floor` itself stays.
fn delete_files(app_dir: &Path, floor: &Path, files: &[String]) -> Result<(), Error> {
    for file in files {
        let path = app_dir.join(file);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(&path, e)),
        }
        let mut parent = path.parent();
        while let Some(dir) = parent {
            if dir == floor || dir == app_dir || fs::remove_dir(dir).is_err() {
                break;
            }
            parent = dir.parent();
        }
    }
    Ok(())
}

/// `AviUtl2` reads `<exe dir>/data` only while that directory exists, so it must stay.
fn removal_floor(instance: &Instance) -> PathBuf {
    let app_dir = instance.app_dir();
    match instance.config.kind {
        Kind::Aviutl2 => {
            Roots::locate(Kind::Aviutl2, &app_dir, None).map_or(app_dir, |roots| roots.data)
        }
        _ => app_dir,
    }
}

/// Paths below `app_dir`, written with `/`.
fn relative_files(app_dir: &Path, files: &[PathBuf]) -> Result<Vec<String>, Error> {
    files
        .iter()
        .map(|file| {
            let rel = file
                .strip_prefix(app_dir)
                .map_err(|_| Error::UnsafePath(file.display().to_string()))?;
            Ok(rel
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/"))
        })
        .collect()
}
