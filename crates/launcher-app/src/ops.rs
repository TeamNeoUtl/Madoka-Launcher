use std::path::PathBuf;
use std::sync::Arc;

use launcher_core::{
    AppSelection, AppVersion, Config, Edition, Error, Host, Installer, Instance, InstanceId,
    InstanceLocks, InstanceStore, Kind, Launched, NewInstance, PROBE_WINDOW, TaskBoard, TaskHandle,
    default_root, open_directory,
};
use launcher_providers::{AnyProvider, Provider, ProviderError, ReqwestFetcher, ReqwestHttp};

const CREATE_STEPS: u16 = 2;
const DELETE_STEPS: u16 = 1;
const LAUNCH_STEPS: u16 = 2;

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("data directory is unavailable")]
    NoDataDir,
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
    store: InstanceStore,
    installer: Arc<Installer<ReqwestFetcher>>,
    http: ReqwestHttp,
    locks: InstanceLocks,
}

impl Services {
    pub fn new() -> Result<Self, OpError> {
        let root = default_root().ok_or(OpError::NoDataDir)?;
        let config = Config::load(&root)?;
        let user_agent = format!("MLAYN-Launcher/{}", env!("CARGO_PKG_VERSION"));
        let token = config.github_token.filter(|token| !token.trim().is_empty());
        let http = ReqwestHttp::new(&user_agent, token).map_err(ProviderError::from)?;
        let fetcher = ReqwestFetcher::new(&user_agent)?;
        Ok(Self {
            host: Host::current(),
            board: TaskBoard::default(),
            store: InstanceStore::new(&root),
            installer: Arc::new(Installer::new(root, fetcher)),
            http,
            locks: InstanceLocks::default(),
        })
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
        Ok(provider.app_versions().await?)
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
        let result = self.create_instance(&handle, request).await;
        handle.finish(&result);
        result.ok()
    }

    pub async fn delete(self, handle: TaskHandle, id: InstanceId) {
        let result = self.delete_instance(&handle, id).await;
        handle.finish(&result);
    }

    pub async fn launch(self, handle: TaskHandle, id: InstanceId) {
        let result = self.launch_instance(&handle, id).await;
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
        if let Err(error) = self.installer.install_app(id, &artifacts).await {
            let _ = self.store.delete(id);
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
