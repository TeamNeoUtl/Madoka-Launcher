use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::Error;

/// Name of the directory beside the executable that holds all data in portable mode.
pub const PORTABLE_DIR: &str = "data";

/// Data directory of portable mode: `<executable directory>/data`.
#[must_use]
pub fn portable_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(PORTABLE_DIR))
}

/// Data directory of the operating system: `ProjectDirs` of `Madoka-Launcher`.
#[must_use]
pub fn standard_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "Madoka-Launcher").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Portable mode is on while `<executable directory>/data` or `./data` is a directory.
#[must_use]
pub fn default_root() -> Option<PathBuf> {
    if let Some(portable) = portable_root().filter(|dir| dir.is_dir()) {
        return Some(portable);
    }
    let local = PathBuf::from(PORTABLE_DIR);
    if local.is_dir() {
        return Some(local);
    }
    standard_root()
}

/// Copies the content of the current data directory into the target directory of the new mode,
/// so that the next start finds the same settings, instances, and caches.
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

    use super::copy_tree;

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
