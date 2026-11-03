use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::{error, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const FILTER_ENV: &str = "Motoyori_LOG";
const DEFAULT_DIRECTIVES: &str =
    "warn,launcher_core=debug,launcher_providers=debug,reqwest=debug,hyper_util=debug";
const LOG_DIR: &str = "logs";
const LOG_FILE: &str = "launcher.log";
const PREVIOUS_LOG_FILE: &str = "launcher.previous.log";

/// Writes records to stderr and to `<root>/logs/launcher.log`.
/// The previous log is kept as `launcher.previous.log`. `Motoyori_LOG` overrides the filter.
/// Panics are recorded as `error` records.
pub fn init(root: &Path) -> PathBuf {
    let path = root.join(LOG_DIR).join(LOG_FILE);
    let file = open_log_file(&path);
    let file_error = file.as_ref().err().map(ToString::to_string);

    let filter = EnvFilter::try_from_env(FILTER_ENV).unwrap_or_else(|_| default_filter());
    let file_layer = file
        .ok()
        .map(|file| fmt::layer().with_ansi(false).with_writer(Mutex::new(file)));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(io::stderr))
        .with(file_layer)
        .init();

    if let Some(reason) = file_error {
        warn!(path = %path.display(), %reason, "log file is unavailable");
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        error!(%info, "panic");
        previous(info);
    }));
    path
}

/// The target of this crate is the binary name (`Motoyori_Launcher`), not the package name.
fn default_filter() -> EnvFilter {
    let target = module_path!().split("::").next().unwrap_or_default();
    EnvFilter::new(format!("{DEFAULT_DIRECTIVES},{target}=debug"))
}

fn open_log_file(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
        if path.exists() {
            fs::rename(path, dir.join(PREVIOUS_LOG_FILE))?;
        }
    }
    File::create(path)
}
