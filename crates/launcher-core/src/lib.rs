mod archive;
mod catalog;
mod config;
mod error;
mod fetch;
mod hash;
mod host;
mod installer;
mod lock;
mod model;
mod paths;
mod runner;
mod sort;
mod stage;
mod store;
mod swap;
mod task;

pub use archive::{extract_sfx, zip_entry_names};
pub use catalog::{
    AUO_SETUP_TIMEOUT_SECS, AppSelection, AppVersion, Artifact, FileCheck, Hash, HashAlgo,
    InstallPlan, Location, Origin, Package, PackageSource, Recipe, Root, StagedPlan, Step,
};
pub use config::Config;
pub use error::Error;
pub use fetch::{
    ALLOWED_HOSTS, Fetcher, HTTP_HOSTS, NoProgress, Progress, ProgressSink, is_allowed_url,
};
pub use hash::{SCAN_DIRS, is_present, scan_xxh3_128};
pub use host::Host;
pub use installer::Installer;
pub use lock::InstanceLocks;
pub use model::{Edition, Instance, InstanceConfig, InstanceId, Kind, NewInstance, PluginRecord};
pub use paths::{PORTABLE_DIR, default_root, portable_root, set_portable, standard_root};
pub use runner::{
    Launched, PROBE_WINDOW, find_executable, is_instance_running, open_directory, open_url,
};
pub use sort::{SortOrder, TREND_DAYS, sort_packages, today};
pub use store::{InstanceStore, ListFailure, Listing};
pub use task::{TaskBoard, TaskHandle, TaskId, TaskInfo, TaskSink, TaskState};
