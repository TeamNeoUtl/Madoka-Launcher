mod archive;
mod catalog;
mod error;
mod fetch;
mod host;
mod installer;
mod model;
mod paths;
mod store;

pub use catalog::{
    AppSelection, AppVersion, Artifact, Hash, HashAlgo, InstallPlan, Package, PackageSource,
};
pub use error::Error;
pub use fetch::{ALLOWED_HOSTS, Fetcher, is_allowed_url};
pub use host::Host;
pub use installer::Installer;
pub use model::{Edition, Instance, InstanceConfig, InstanceId, Kind, NewInstance, PluginRecord};
pub use paths::default_root;
pub use store::{InstanceStore, ListFailure, Listing};
