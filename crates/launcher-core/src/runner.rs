use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use tracing::{debug, info, warn};
use url::Url;

use crate::{Error, Host, Instance, Kind};

const LOG_FILE: &str = "launch.log";
const LOG_TAIL_BYTES: usize = 4096;
const SEARCH_DEPTH: usize = 2;
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A process that exits with a failure status inside this window is reported as a failed launch.
pub const PROBE_WINDOW: Duration = Duration::from_secs(5);

impl Kind {
    /// File stems accepted as the executable. Compared case-insensitively.
    const fn executable_stems(self) -> &'static [&'static str] {
        match self {
            Self::Aviutl => &["aviutl"],
            Self::Aviutl2 => &["aviutl2"],
            Self::Ymm4 => &["YukkuriMovieMaker"],
            Self::Neoutl => &["NeoUtl"],
            Self::Freya => &["freya-nle", "freya", "aviqtl"],
        }
    }
}

fn is_executable_name(path: &Path, stems: &[&str], host: Host) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let stem = match host {
        Host::Windows => {
            let named = Path::new(name);
            if !named
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                return false;
            }
            named
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
        }
        Host::Linux => name,
    };
    stems
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(stem))
}

/// Searches `app_dir` and its sub-directories down to depth 2, shallowest level first.
/// Within one level the smallest path wins.
/// # Errors
///
/// Returns `Error::Io` when a directory cannot be read,
/// and `Error::ExecutableNotFound` when no file name matches `kind`.
pub fn find_executable(app_dir: &Path, kind: Kind, host: Host) -> Result<PathBuf, Error> {
    let stems = kind.executable_stems();
    let mut level = vec![app_dir.to_owned()];
    for _ in 0..=SEARCH_DEPTH {
        let mut found = Vec::new();
        let mut next = Vec::new();
        for dir in &level {
            for entry in fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
                let entry = entry.map_err(|e| Error::io(dir, e))?;
                let path = entry.path();
                let file_type = entry.file_type().map_err(|e| Error::io(&path, e))?;
                if file_type.is_dir() {
                    next.push(path);
                } else if file_type.is_file() && is_executable_name(&path, stems, host) {
                    found.push(path);
                }
            }
        }
        if let Some(path) = found.into_iter().min() {
            debug!(exe = %path.display(), ?kind, "executable found");
            return Ok(path);
        }
        level = next;
    }
    warn!(dir = %app_dir.display(), ?kind, "executable not found");
    Err(Error::ExecutableNotFound {
        kind,
        dir: app_dir.to_owned(),
    })
}

/// A spawned editor process. stdout and stderr go to `<instance>/launch.log`.
#[derive(Debug)]
pub struct Launched {
    child: Child,
    exe: PathBuf,
    log: PathBuf,
}

impl Launched {
    /// # Errors
    ///
    /// Returns the errors of `find_executable`, and `Error::Io` when the executable bit,
    /// the log file, or the process start fails.
    pub fn spawn(instance: &Instance, host: Host) -> Result<Self, Error> {
        let exe = find_executable(&instance.app_dir(), instance.config.kind, host)?;
        #[cfg(unix)]
        ensure_executable(&exe)?;
        let log = instance.dir.join(LOG_FILE);
        let out = File::create(&log).map_err(|e| Error::io(&log, e))?;
        let err = out.try_clone().map_err(|e| Error::io(&log, e))?;
        let child = Command::new(&exe)
            .current_dir(exe.parent().unwrap_or(instance.dir.as_path()))
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|e| Error::io(&exe, e))?;
        info!(exe = %exe.display(), pid = child.id(), log = %log.display(), "process spawned");
        Ok(Self { child, exe, log })
    }

    /// Blocks up to `window`. Returns `LaunchFailed` with the log tail when the process
    /// exits with a failure status inside the window.
    ///
    /// # Errors
    ///
    /// Returns `Error::LaunchFailed` for a failure exit status inside `window`,
    /// and `Error::Io` when the process state cannot be read.
    pub fn confirm(self, window: Duration) -> Result<(), Error> {
        let Self {
            mut child,
            exe,
            log,
        } = self;
        let deadline = Instant::now() + window;
        loop {
            match child.try_wait().map_err(|e| Error::io(&exe, e))? {
                Some(status) if status.success() => {
                    info!(exe = %exe.display(), %status, "process exited with success");
                    return Ok(());
                }
                Some(status) => {
                    warn!(exe = %exe.display(), %status, "process exited with failure");
                    return Err(Error::LaunchFailed {
                        exe,
                        status: status.to_string(),
                        log: read_tail(&log),
                    });
                }
                None if Instant::now() >= deadline => {
                    info!(exe = %exe.display(), window_ms = window.as_millis(), "process still running");
                    reap(child);
                    return Ok(());
                }
                None => thread::sleep(POLL_INTERVAL),
            }
        }
    }
}

/// Adds the executable bits to a file that has none. The launcher has selected this file
/// as the executable of the instance, so the bits are set at launch.
#[cfg(unix)]
fn ensure_executable(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|e| Error::io(path, e))?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    if mode & 0o111 == 0 {
        permissions.set_mode(mode | 0o111);
        fs::set_permissions(path, permissions).map_err(|e| Error::io(path, e))?;
        info!(exe = %path.display(), "executable bit set");
    }
    Ok(())
}

/// Opens `dir` in the file manager of `host`.
///
/// # Errors
///
/// Returns `Error::Io` when the file manager cannot start.
pub fn open_directory(dir: &Path, host: Host) -> Result<(), Error> {
    debug!(dir = %dir.display(), "open directory");
    open_with_shell(dir.as_os_str(), host, dir)
}

/// Opens an `http` or `https` URL in the default browser of `host`.
///
/// # Errors
///
/// Returns `Error::UnsafePath` for another scheme, and `Error::Io` when the browser cannot start.
pub fn open_url(url: &Url, host: Host) -> Result<(), Error> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::UnsafePath(url.to_string()));
    }
    debug!(%url, "open url");
    open_with_shell(OsStr::new(url.as_str()), host, Path::new(url.as_str()))
}

fn open_with_shell(target: &OsStr, host: Host, error_path: &Path) -> Result<(), Error> {
    let program = match host {
        Host::Windows => "explorer",
        Host::Linux => "xdg-open",
    };
    let child = Command::new(program)
        .arg(target)
        .spawn()
        .map_err(|e| Error::io(error_path, e))?;
    reap(child);
    Ok(())
}

fn reap(mut child: Child) {
    thread::spawn(move || {
        let _ = child.wait();
    });
}

fn read_tail(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    let start = bytes.len().saturating_sub(LOG_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).trim().to_owned()
}

/// Checks if an executable belonging to this instance is currently running.
#[must_use]
pub fn is_instance_running(instance: &Instance) -> bool {
    let Ok(mut system) = System::new_with_specifics(
        RefreshKind::nothing().with_processes(
            ProcessRefreshKind::nothing()
                .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
                .with_cwd(sysinfo::UpdateKind::OnlyIfNotSet),
        ),
    )
    .inspect_err(|error| tracing::warn!(%error, "process list is unavailable")) else {
        return false;
    };
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
            .with_cwd(sysinfo::UpdateKind::OnlyIfNotSet),
    );

    let instance_dir = &instance.dir;
    let stems = instance.config.kind.executable_stems();

    for process in system.processes().values() {
        if process
            .exe()
            .is_some_and(|exe| exe.starts_with(instance_dir))
        {
            return true;
        }
        let proc_name = process.name().to_string_lossy();
        let stem = proc_name.strip_suffix(".exe").unwrap_or(&proc_name);
        if stems.iter().any(|s| s.eq_ignore_ascii_case(stem))
            && process
                .cwd()
                .is_some_and(|cwd| cwd.starts_with(instance_dir))
        {
            return true;
        }
    }
    false
}
