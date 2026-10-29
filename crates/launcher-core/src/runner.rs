use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::{Error, Host, Instance, Kind};

const LOG_FILE: &str = "launch.log";
const LOG_TAIL_BYTES: usize = 4096;
const SEARCH_DEPTH: usize = 2;
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A process that exits with a failure status inside this window is reported as a failed launch.
pub const PROBE_WINDOW: Duration = Duration::from_secs(5);

impl Kind {
    /// File stems accepted as the executable. Compared case-insensitively.
    fn executable_stems(self) -> &'static [&'static str] {
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
    pub fn spawn(instance: &Instance, host: Host) -> Result<Self, Error> {
        let exe = find_executable(&instance.app_dir(), instance.config.kind, host)?;
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

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> Result<(), Error> {
    Ok(())
}

/// Opens `dir` in the file manager of `host`.
pub fn open_directory(dir: &Path, host: Host) -> Result<(), Error> {
    let program = match host {
        Host::Windows => "explorer",
        Host::Linux => "xdg-open",
    };
    debug!(program, dir = %dir.display(), "open directory");
    let child = Command::new(program)
        .arg(dir)
        .spawn()
        .map_err(|e| Error::io(dir, e))?;
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
