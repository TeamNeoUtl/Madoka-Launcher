use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, PoisonError, RwLock};

use launcher_core::{
    AppSelection, AppVersion, Config, Edition, Error, Fetcher, Host, InstallPlan, Installer,
    Instance, InstanceId, InstanceLocks, InstanceStore, Kind, Launched, NewInstance, PROBE_WINDOW,
    Package, TaskBoard, TaskHandle, open_directory,
};
use launcher_providers::{
    AnyProvider, Dns, Provider, ProviderError, ReqwestFetcher, ReqwestHttp, load_any, load_fresh,
    store_catalog,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use url::Url;

const CREATE_STEPS: u16 = 2;
const DELETE_STEPS: u16 = 1;
const LAUNCH_STEPS: u16 = 2;
const WRITE_STEPS: u16 = 1;
const BROWSER_EXE: &str = if cfg!(windows) {
    "Motoyori-Browser.exe"
} else {
    "Motoyori-Browser"
};
const BROWSER_DOWNLOAD_PREFIX: &str = "download ";
const BROWSED_DIR: &str = "downloads";
const PART_SUFFIX: &str = ".part";
const DEFAULT_FILE_NAME: &str = "download";
const UNKNOWN_VERSION: &str = "-";
const PLUGIN_ARCHIVE_EXTENSION: &str = "ymme";
const FORBIDDEN_FILE_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{0}")]
    Core(Box<Error>),
    #[error("{0}")]
    Provider(Box<ProviderError>),
    #[error("worker failed: {0}")]
    Worker(String),
}

impl From<Error> for OpError {
    fn from(error: Error) -> Self {
        Self::Core(Box::new(error))
    }
}

impl From<ProviderError> for OpError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(Box::new(error))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub instances: Vec<Instance>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CreateRequest {
    pub name: String,
    pub kind: Kind,
    pub edition: Edition,
    pub group: Option<String>,
    pub version: AppVersion,
}

/// A download that started in the browser window.
#[derive(Debug, Clone)]
pub struct Browsed {
    pub url: Url,
    pub name: Option<String>,
}

impl Browsed {
    /// File name that the browser reported, else the last segment of the URL path, else `download`.
    fn file_name(&self) -> String {
        let from_url = || {
            let segment = self.url.path_segments()?.next_back()?;
            Some(
                percent_encoding::percent_decode_str(segment)
                    .decode_utf8_lossy()
                    .into_owned(),
            )
        };
        let raw = self.name.clone().or_else(from_url).unwrap_or_default();
        let name: String = Path::new(&raw)
            .file_name()
            .map(|name| name.to_string_lossy().replace(FORBIDDEN_FILE_CHARS, "_"))
            .unwrap_or_default();
        if name.is_empty() || name == ".." {
            DEFAULT_FILE_NAME.to_owned()
        } else {
            name
        }
    }

    pub fn label(&self) -> String {
        self.file_name()
    }
}

/// Everything a task needs. Clones share the task board, the locks and the installer.
#[derive(Clone)]
pub struct Services {
    pub host: Host,
    pub board: TaskBoard,
    root: PathBuf,
    theme: Option<String>,
    font: Option<String>,
    dns_servers: Vec<IpAddr>,
    dns: Dns,
    browsed: Arc<ReqwestFetcher>,
    token: Option<String>,
    user_agent: Arc<str>,
    store: InstanceStore,
    installer: Arc<Installer<ReqwestFetcher>>,
    http: Arc<RwLock<ReqwestHttp>>,
    locks: InstanceLocks,
}

impl Services {
    pub fn new(root: PathBuf) -> Result<Self, OpError> {
        let config = Config::load(&root)?;
        let user_agent = format!("Motoyori-Launcher/{}", env!("CARGO_PKG_VERSION"));
        let token = config.github_token.filter(|token| !token.trim().is_empty());
        let config_has_token = token.is_some();
        let theme = config.theme;
        let font = config.font;
        let dns_servers = config.dns;
        let dns = Dns::new(&dns_servers);
        let http = ReqwestHttp::with_dns(&user_agent, token.clone(), dns.clone())
            .map_err(ProviderError::from)?;
        let fetcher = ReqwestFetcher::with_dns(&user_agent, dns.clone())?;
        let browsed = Arc::new(ReqwestFetcher::unrestricted(&user_agent, dns.clone())?);
        info!(
            root = %root.display(),
            host = ?Host::current(),
            github_token = config_has_token,
            "services ready"
        );
        Ok(Self {
            host: Host::current(),
            board: TaskBoard::default(),
            theme,
            font,
            dns_servers,
            dns,
            browsed,
            token,
            user_agent: user_agent.into(),
            store: InstanceStore::new(&root),
            root: root.clone(),
            installer: Arc::new(Installer::new(root, fetcher)),
            http: Arc::new(RwLock::new(http)),
            locks: InstanceLocks::default(),
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// GitHub token that `config.toml` held at start.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn http(&self) -> ReqwestHttp {
        self.http
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Name of the theme stored in `config.toml`.
    pub fn theme_name(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    /// DNS servers stored in `config.toml`. Empty selects the built-in default.
    pub fn dns_servers(&self) -> &[IpAddr] {
        &self.dns_servers
    }

    /// Writes the DNS servers to `config.toml`. They apply at the next start.
    pub async fn save_dns(self, servers: Vec<IpAddr>) -> Result<(), OpError> {
        let root = self.root.clone();
        blocking(move || {
            let mut config = Config::load(&root)?;
            config.dns = servers;
            config.save(&root)
        })
        .await
    }

    /// Family name of the font stored in `config.toml`.
    pub fn font_name(&self) -> Option<&str> {
        self.font.as_deref()
    }

    /// Writes the font family to `config.toml`. It applies at the next start.
    pub async fn save_font(self, name: Option<String>) -> Result<(), OpError> {
        let root = self.root.clone();
        blocking(move || {
            let mut config = Config::load(&root)?;
            config.font = name;
            config.save(&root)
        })
        .await
    }

    pub async fn save_theme(self, name: String) -> Result<(), OpError> {
        let root = self.root.clone();
        blocking(move || {
            let mut config = Config::load(&root)?;
            config.theme = Some(name);
            config.save(&root)
        })
        .await
    }

    /// Writes the token to `config.toml` and applies it to the following requests.
    pub async fn save_token(self, token: String) -> Result<(), OpError> {
        let token = Some(token.trim().to_owned()).filter(|token| !token.is_empty());
        let root = self.root.clone();
        let stored = token.clone();
        blocking(move || {
            let mut config = Config::load(&root)?;
            config.github_token = stored;
            config.save(&root)
        })
        .await?;
        let http = ReqwestHttp::with_dns(&self.user_agent, token, self.dns.clone())
            .map_err(ProviderError::from)?;
        *self.http.write().unwrap_or_else(PoisonError::into_inner) = http;
        Ok(())
    }

    pub async fn list(self) -> Result<Loaded, OpError> {
        let store = self.store.clone();
        let listing = blocking(move || store.list()).await?;
        Ok(Loaded {
            instances: listing.instances,
            failures: listing
                .failures
                .into_iter()
                .map(|failure| format!("{}: {}", failure.dir.display(), failure.error))
                .collect(),
        })
    }

    pub async fn app_versions(self, kind: Kind) -> Result<Vec<AppVersion>, OpError> {
        let provider = AnyProvider::for_kind(kind, self.http())?;
        let result = provider.app_versions().await;
        match &result {
            Ok(versions) => info!(?kind, count = versions.len(), "app versions loaded"),
            Err(error) => warn!(?kind, %error, "app versions failed"),
        }
        Ok(result?)
    }

    /// Packages of the catalog. A cache file younger than 24 hours is used unless `refresh` is set.
    /// A failed request falls back to the cache file of any age.
    pub async fn catalog(self, kind: Kind, refresh: bool) -> Result<Vec<Package>, OpError> {
        let root = self.root.clone();
        if !refresh {
            let cached = tokio::task::spawn_blocking({
                let root = root.clone();
                move || load_fresh(&root, kind)
            })
            .await
            .map_err(|error| OpError::Worker(error.to_string()))?;
            if let Some(packages) = cached {
                info!(?kind, count = packages.len(), "catalog cache hit");
                return Ok(packages);
            }
        }
        let provider = AnyProvider::for_kind(kind, self.http())?;
        match provider.catalog().await {
            Ok(packages) => {
                info!(?kind, count = packages.len(), "catalog loaded");
                let stored = packages.clone();
                let written =
                    tokio::task::spawn_blocking(move || store_catalog(&root, kind, &stored)).await;
                if let Ok(Err(error)) = written {
                    warn!(?kind, %error, "catalog cache write failed");
                }
                Ok(packages)
            }
            Err(error) => {
                warn!(?kind, %error, "catalog failed");
                let stale = tokio::task::spawn_blocking(move || load_any(&root, kind))
                    .await
                    .ok()
                    .flatten();
                stale.ok_or_else(|| error.into())
            }
        }
    }

    /// Resolves how `package` is installed. Requests the release list when the plan needs it.
    pub async fn plan(
        self,
        kind: Kind,
        package: Package,
    ) -> Result<(Package, InstallPlan), OpError> {
        let provider = AnyProvider::for_kind(kind, self.http())?;
        let plan = provider.plan_install_package(&package).await?;
        Ok((package, plan))
    }

    /// Keeps the versions that have an artifact for `selection`.
    pub fn installable(
        &self,
        kind: Kind,
        selection: AppSelection,
        versions: &[AppVersion],
    ) -> Vec<AppVersion> {
        let Ok(provider) = AnyProvider::for_kind(kind, self.http()) else {
            return Vec::new();
        };
        versions
            .iter()
            .filter(|version| provider.plan_install_app(version, selection).is_ok())
            .cloned()
            .collect()
    }

    /// Shows `url` in the browser window and waits until the window closes.
    /// Returns the download that started in the page. The window closes at that moment.
    pub async fn browse(self, url: Url) -> Result<Option<Browsed>, OpError> {
        ensure_web(&url)?;
        let exe = std::env::current_exe()
            .map_err(|error| OpError::Worker(error.to_string()))?
            .with_file_name(BROWSER_EXE);
        let worker = |error: std::io::Error| OpError::Worker(format!("{}: {error}", exe.display()));
        let mut child = Command::new(&exe)
            .arg(url.as_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(worker)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| OpError::Worker("browser stdout is unavailable".to_owned()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| OpError::Worker("browser stderr is unavailable".to_owned()))?;
        let mut lines = BufReader::new(stdout).lines();
        let mut found = None;
        while let Some(line) = lines.next_line().await.map_err(worker)? {
            let Some((name, target)) = line
                .strip_prefix(BROWSER_DOWNLOAD_PREFIX)
                .and_then(|rest| rest.split_once('\t'))
            else {
                continue;
            };
            if let Ok(url) = Url::parse(target) {
                found = Some(Browsed {
                    url,
                    name: Some(name.to_owned()).filter(|name| !name.is_empty()),
                });
                break;
            }
        }
        if found.is_some() {
            child.kill().await.map_err(worker)?;
        } else {
            let status = child.wait().await.map_err(worker)?;
            if !status.success() {
                let mut message = String::new();
                stderr.read_to_string(&mut message).await.map_err(worker)?;
                let message = message.trim();
                warn!(%url, %status, message, "browser failed");
                return Err(OpError::Worker(format!("{status}: {message}")));
            }
        }
        info!(%url, downloaded = found.is_some(), "browser closed");
        Ok(found)
    }

    /// Saves the browsed download to `<root>/downloads/`.
    /// A `.ymme` file is then installed as `package` into instance `id`.
    pub async fn download_browsed(
        self,
        handle: TaskHandle,
        id: InstanceId,
        package: Package,
        browsed: Browsed,
    ) {
        info!(%id, package = %package.id, url = %browsed.url, "browsed download task start");
        let result = self
            .download_browsed_inner(&handle, id, &package, &browsed)
            .await;
        log_outcome("browsed download", &result);
        handle.finish(&result);
    }

    async fn download_browsed_inner(
        &self,
        handle: &TaskHandle,
        id: InstanceId,
        package: &Package,
        browsed: &Browsed,
    ) -> Result<PathBuf, OpError> {
        ensure_web(&browsed.url)?;
        handle.running(0, WRITE_STEPS);
        let dir = self.root.join(BROWSED_DIR);
        let name = browsed.file_name();
        let dest = blocking(move || {
            std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            Ok(unique_path(&dir, &name))
        })
        .await?;
        let mut part = dest.clone().into_os_string();
        part.push(PART_SUFFIX);
        let part = PathBuf::from(part);
        if let Err(error) = self.browsed.fetch(&browsed.url, &part, handle).await {
            let _ = std::fs::remove_file(&part);
            return Err(error.into());
        }
        std::fs::rename(&part, &dest).map_err(|source| Error::Io {
            path: dest.clone(),
            source,
        })?;
        if is_plugin_archive(&dest) {
            let _guard = self.locks.lock(id).await;
            let installer = Arc::clone(&self.installer);
            let package = package.clone();
            let version = package
                .latest_version()
                .unwrap_or(UNKNOWN_VERSION)
                .to_owned();
            let archive = dest.clone();
            blocking(move || {
                installer
                    .install_package_file(id, &package, &version, &archive)
                    .map(drop)
            })
            .await?;
        }
        Ok(dest)
    }

    pub async fn open_folder(self, dir: PathBuf) -> Result<(), OpError> {
        let host = self.host;
        blocking(move || open_directory(&dir, host)).await
    }

    /// Creates the instance directory, then installs the app into it.
    /// Deletes the instance directory when the installation fails.
    pub async fn create(self, handle: TaskHandle, request: CreateRequest) -> Option<InstanceId> {
        info!(name = %request.name, kind = ?request.kind, version = %request.version.version, "create task start");
        let result = self.create_instance(&handle, request).await;
        log_outcome("create", &result);
        handle.finish(&result);
        result.ok()
    }

    pub async fn delete(self, handle: TaskHandle, id: InstanceId) {
        info!(%id, "delete task start");
        let result = self.delete_instance(&handle, id).await;
        log_outcome("delete", &result);
        handle.finish(&result);
    }

    pub async fn launch(self, handle: TaskHandle, id: InstanceId) {
        info!(%id, "launch task start");
        let result = self.launch_instance(&handle, id).await;
        log_outcome("launch", &result);
        handle.finish(&result);
    }

    pub async fn duplicate(
        self,
        handle: TaskHandle,
        id: InstanceId,
        name: String,
    ) -> Option<InstanceId> {
        info!(%id, "duplicate task start");
        let result = self.duplicate_instance(&handle, id, name).await;
        log_outcome("duplicate", &result);
        handle.finish(&result);
        result.ok()
    }

    pub async fn update_instance(
        self,
        id: InstanceId,
        name: String,
        group: Option<String>,
    ) -> Result<(), OpError> {
        let _guard = self.locks.lock(id).await;
        let store = self.store.clone();
        blocking(move || store.update(id, name, group).map(|_| ())).await
    }

    pub async fn install_plugin(
        self,
        handle: TaskHandle,
        id: InstanceId,
        package: Package,
        plan: InstallPlan,
    ) {
        info!(%id, package = %package.id, "install plugin task start");
        let result = self.install_plugin_inner(&handle, id, &package, plan).await;
        log_outcome("install plugin", &result);
        handle.finish(&result);
    }

    pub async fn remove_plugin(self, handle: TaskHandle, id: InstanceId, plugin_id: String) {
        info!(%id, package = %plugin_id, "remove plugin task start");
        let result = self.remove_plugin_inner(&handle, id, plugin_id).await;
        log_outcome("remove plugin", &result);
        handle.finish(&result);
    }

    async fn duplicate_instance(
        &self,
        handle: &TaskHandle,
        id: InstanceId,
        name: String,
    ) -> Result<InstanceId, OpError> {
        let _guard = self.locks.lock(id).await;
        handle.running(0, WRITE_STEPS);
        let store = self.store.clone();
        let copy = blocking(move || store.duplicate(id, name)).await?;
        Ok(copy.config.id)
    }

    async fn install_plugin_inner(
        &self,
        handle: &TaskHandle,
        id: InstanceId,
        package: &Package,
        plan: InstallPlan,
    ) -> Result<(), OpError> {
        let _guard = self.locks.lock(id).await;
        handle.running(0, WRITE_STEPS);
        match plan {
            InstallPlan::Download(artifacts) => {
                let artifact = artifacts
                    .first()
                    .ok_or_else(|| OpError::Worker("install plan holds no artifact".to_owned()))?;
                let version = package.latest_version().unwrap_or(UNKNOWN_VERSION);
                self.installer
                    .install_package(id, package, version, artifact, handle)
                    .await?;
            }
            InstallPlan::Staged(staged) => {
                self.installer
                    .install_staged(id, package, &staged, handle)
                    .await?;
            }
            InstallPlan::OpenInBrowser(_) => {}
        }
        Ok(())
    }

    async fn remove_plugin_inner(
        &self,
        handle: &TaskHandle,
        id: InstanceId,
        plugin_id: String,
    ) -> Result<(), OpError> {
        let _guard = self.locks.lock(id).await;
        handle.running(0, WRITE_STEPS);
        let installer = Arc::clone(&self.installer);
        blocking(move || installer.remove_package(id, &plugin_id)).await
    }

    async fn create_instance(
        &self,
        handle: &TaskHandle,
        request: CreateRequest,
    ) -> Result<InstanceId, OpError> {
        let CreateRequest {
            name,
            kind,
            edition,
            group,
            version,
        } = request;
        handle.running(0, CREATE_STEPS);
        let provider = AnyProvider::for_kind(kind, self.http())?;
        let selection = AppSelection {
            host: self.host,
            edition,
        };
        let artifacts = provider.plan_install_app(&version, selection)?;
        let checks = version.checks.clone();
        debug!(
            version = %version.version,
            artifacts = ?artifacts.iter().map(|a| a.url.as_str()).collect::<Vec<_>>(),
            "install plan"
        );
        let instance = self.store.create(
            NewInstance {
                name,
                kind,
                edition,
                app_version: version.version,
                group,
            },
            self.host,
        )?;
        let id = instance.config.id;
        let _guard = self.locks.lock(id).await;
        handle.running(1, CREATE_STEPS);
        if let Err(error) = self
            .installer
            .install_app_checked(id, &artifacts, &checks, handle)
            .await
        {
            warn!(%id, %error, "install failed, deleting the instance");
            if let Err(cleanup) = self.store.delete(id) {
                error!(%id, error = %cleanup, "instance cleanup failed");
            }
            return Err(error.into());
        }
        Ok(id)
    }

    async fn delete_instance(&self, handle: &TaskHandle, id: InstanceId) -> Result<(), OpError> {
        let _guard = self.locks.lock(id).await;
        handle.running(0, DELETE_STEPS);
        let store = self.store.clone();
        blocking(move || store.delete(id)).await
    }

    async fn launch_instance(&self, handle: &TaskHandle, id: InstanceId) -> Result<(), OpError> {
        let launched = {
            let _guard = self.locks.lock(id).await;
            handle.running(0, LAUNCH_STEPS);
            let store = self.store.clone();
            let host = self.host;
            blocking(move || Launched::spawn(&store.load(id)?, host)).await?
        };
        handle.running(1, LAUNCH_STEPS);
        blocking(move || launched.confirm(PROBE_WINDOW)).await
    }
}

fn log_outcome<T>(task: &str, result: &Result<T, OpError>) {
    match result {
        Ok(_) => info!(task, "task succeeded"),
        Err(error) => error!(task, %error, "task failed"),
    }
}

fn is_plugin_archive(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(PLUGIN_ARCHIVE_EXTENSION))
}

fn ensure_web(url: &Url) -> Result<(), OpError> {
    if matches!(url.scheme(), "http" | "https") {
        Ok(())
    } else {
        Err(Error::UnsafePath(url.to_string()).into())
    }
}

/// `dir/name`, or the first free `dir/stem (n).ext`.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().map_or_else(
        || name.to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned());
    let numbered = |n: u32| {
        let name = extension.as_ref().map_or_else(
            || format!("{stem} ({n})"),
            |extension| format!("{stem} ({n}).{extension}"),
        );
        dir.join(name)
    };
    (1..=u32::MAX)
        .map(numbered)
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// Sorted family names of the fonts installed on the OS.
pub async fn installed_fonts() -> Result<Vec<String>, OpError> {
    tokio::task::spawn_blocking(|| {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let mut families: Vec<String> = database
            .faces()
            .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .collect();
        families.sort();
        families.dedup();
        families
    })
    .await
    .map_err(|error| OpError::Worker(error.to_string()))
}

async fn blocking<T, F>(work: F) -> Result<T, OpError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| OpError::Worker(error.to_string()))?;
    Ok(joined?)
}
