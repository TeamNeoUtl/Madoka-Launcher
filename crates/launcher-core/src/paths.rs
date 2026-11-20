use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

use crate::Error;

/// Name of the directory beside the executable that holds all data in portable mode.
pub const PORTABLE_DIR: &str = "data";
const LOG_DIR: &str = "logs";
const MACOS_LOG_DIR: [&str; 3] = ["Library", "Logs", "Madoka-Launcher"];

/// Data directory of portable mode: `<executable directory>/data`.
/// `None` on macOS: the application bundle may sit on a read-only or translocated volume.
#[must_use]
pub fn portable_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(PORTABLE_DIR))
}

/// Data directory of the operating system: `ProjectDirs` of `Madoka-Launcher`.
#[must_use]
pub fn standard_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "Madoka-Launcher").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Portable mode is on while `<executable directory>/data` is a directory.
/// The working directory does not select a mode, so turning the mode off always takes effect.
#[must_use]
pub fn default_root() -> Option<PathBuf> {
    portable_root()
        .filter(|dir| dir.is_dir())
        .or_else(standard_root)
}

/// Directory of the launcher log: `~/Library/Logs/Madoka-Launcher` on macOS, `<root>/logs` elsewhere.
/// macOS falls back to `<root>/logs` when the home directory is unknown.
#[must_use]
pub fn log_dir(root: &Path) -> PathBuf {
    cfg!(target_os = "macos")
        .then(BaseDirs::new)
        .flatten()
        .map_or_else(
            || root.join(LOG_DIR),
            |dirs| {
                MACOS_LOG_DIR
                    .into_iter()
                    .fold(dirs.home_dir().to_path_buf(), |path, part| path.join(part))
            },
        )
}

/// Copies the content of the current data directory into the target directory of the new mode.
///
/// The next start finds the same settings, instances, and caches.
/// Turning portable mode off removes `<executable directory>/data` after the copy succeeded,
/// because that directory selects portable mode. Turning it on keeps the standard directory.
/// The running process keeps using its current directory until it restarts.
///
/// # Errors
///
/// Returns `Error::Io` when a directory cannot be created, read, copied, or removed,
/// and `Error::DataDirUnavailable` when the executable or the standard directory cannot be located.
pub fn set_portable(enabled: bool) -> Result<(), Error> {
    let portable = portable_root().ok_or(Error::DataDirUnavailable("portable"))?;
    let standard = standard_root().ok_or(Error::DataDirUnavailable("standard"))?;
    if enabled {
        copy_tree(&standard, &portable)
    } else {
        if portable.is_dir() {
            copy_tree(&portable, &standard)?;
            fs::remove_dir_all(&portable).map_err(|source| Error::io(&portable, source))?;
        }
        Ok(())
    }
}

/// Copies every file below `from` to the same relative path below `to`.
/// Existing files at the destination are overwritten. A missing `from` copies nothing
/// and still creates `to`.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    fs::create_dir_all(to).map_err(|source| Error::io(to, source))?;
    let entries = match fs::read_dir(from) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io(from, source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(from, source))?;
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|source| Error::io(&source_path, source))?;
        if kind.is_dir() {
            copy_tree(&source_path, &target_path)?;
        } else if kind.is_file() {
            fs::copy(&source_path, &target_path)
                .map_err(|source| Error::io(&source_path, source))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{copy_tree, log_dir, portable_root};

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn log_dir_sits_below_the_root() {
        let root = std::path::Path::new("root");
        assert_eq!(log_dir(root), root.join("logs"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn log_dir_sits_in_the_library_logs_directory() {
        assert!(log_dir(std::path::Path::new("root")).ends_with("Library/Logs/Madoka-Launcher"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn portable_mode_is_unavailable_on_macos() {
        assert!(portable_root().is_none());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn portable_mode_sits_beside_the_executable() {
        assert!(portable_root().is_some());
    }

    #[test]
    fn copy_tree_copies_nested_files_and_overwrites_existing_ones() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from");
        let to = dir.path().join("to");
        fs::create_dir_all(from.join("instances/a")).unwrap();
        fs::write(from.join("config.toml"), "new").unwrap();
        fs::write(from.join("instances/a/instance.toml"), "a").unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(to.join("config.toml"), "old").unwrap();
        fs::write(to.join("keep.txt"), "keep").unwrap();

        copy_tree(&from, &to).unwrap();

        assert_eq!(fs::read_to_string(to.join("config.toml")).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(to.join("instances/a/instance.toml")).unwrap(),
            "a"
        );
        assert_eq!(fs::read_to_string(to.join("keep.txt")).unwrap(), "keep");
    }

    #[test]
    fn copy_tree_creates_the_target_when_the_source_is_missing() {
        let dir = tempdir().unwrap();
        let to = dir.path().join("to");
        copy_tree(&dir.path().join("missing"), &to).unwrap();
        assert!(to.is_dir());
    }
}
