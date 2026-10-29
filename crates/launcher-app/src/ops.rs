use std::path::PathBuf;
use std::sync::Arc;

use launcher_core::{
    AppSelection, AppVersion, Config, Edition, Error, Host, Installer, Instance, InstanceId,
    InstanceLocks, InstanceStore, Kind, Launched, NewInstance, PROBE_WINDOW, TaskBoard, TaskHandle,
    open_directory,
};
use launcher_providers::{AnyProvider, Provider, ProviderError, ReqwestFetcher, ReqwestHttp};
use tracing::{debug, error, info, warn};

const CREATE_STEPS: u16 = 2;
const DELETE_STEPS: u16 = 1;
const LAUNCH_STEPS: u16 = 2;

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

/// Everything a task needs. Clones share the task board, the locks and the installer.
#[derive(Clone)]
pub struct Services {
    pub host: Host,
    pub board: TaskBoard,
    root: PathBuf,
    theme: Option<String>,
    store: InstanceStore,
    installer: Arc<Installer<ReqwestFetcher>>,
    http: ReqwestHttp,
    locks: InstanceLocks,
}

impl Services {
    pub fn new(root: PathBuf) -> Result<Self, OpError> {
        let config = Config::load(&root)?;
        let user_agent = format!("MLAYN-Launcher/{}", env!("CARGO_PKG_VERSION"));
        let token = config.github_token.filter(|token| !token.trim().is_empty());
        let config_has_token = token.is_some();
        let theme = config.theme;
        let http = ReqwestHttp::new(&user_agent, token).map_err(ProviderError::from)?;
        let fetcher = ReqwestFetcher::new(&user_agent)?;
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
            store: InstanceStore::new(&root),
            root: root.clone(),
            installer: Arc::new(Installer::new(root, fetcher)),
            http,
            locks: InstanceLocks::default(),
        })
    }

    /// Name of the theme stored in `config.toml`.
    pub fn theme_name(&self) -> Option<&str> {
        self.theme.as_deref()
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
        let provider = AnyProvider::for_kind(kind, self.http.clone())?;
        let result = provider.app_versions().await;
        match &result {
            Ok(versions) => info!(?kind, count = versions.len(), "app versions loaded"),
            Err(error) => warn!(?kind, %error, "app versions failed"),
        }
        Ok(result?)
    }

    /// Keeps the versions that have an artifact for `selection`.
    pub fn installable(
        &self,
        kind: Kind,
        selection: AppSelection,
        versions: &[AppVersion],
    ) -> Vec<AppVersion> {
        let Ok(provider) = AnyProvider::for_kind(kind, self.http.clone()) else {
            return Vec::new();
        };
        versions
            .iter()
            .filter(|version| provider.plan_install_app(version, selection).is_ok())
            .cloned()
            .collect()
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
        let provider = AnyProvider::for_kind(kind, self.http.clone())?;
        let selection = AppSelection {
            host: self.host,
            edition,
        };
        let artifacts = provider.plan_install_app(&version, selection)?;
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
        if let Err(error) = self.installer.install_app(id, &artifacts, handle).await {
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
