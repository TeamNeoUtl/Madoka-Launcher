//! Application bundles (`X.app`) of macOS.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use sysinfo::System;

const BUNDLE_EXTENSION: &str = "app";
const EXECUTABLE_DIR: [&str; 2] = ["Contents", "MacOS"];
const INFO_PLIST: [&str; 2] = ["Contents", "Info.plist"];
const EXECUTABLE_KEY: &str = "CFBundleExecutable";
const MIN_OS_KEY: &str = "LSMinimumSystemVersion";

/// `true` for a directory name that ends with `.app`.
pub fn is_bundle_dir(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(BUNDLE_EXTENSION))
}

/// `X.app` of the executable `X.app/Contents/MacOS/<exe>`. `None` for any other layout.
pub fn bundle_root(exe: &Path) -> Option<&Path> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let [first, second] = EXECUTABLE_DIR;
    (macos.file_name()? == second && contents.file_name()? == first && is_bundle_dir(bundle))
        .then_some(bundle)
}

/// Directory `Contents/MacOS` of `bundle`.
pub fn executable_dir(bundle: &Path) -> PathBuf {
    EXECUTABLE_DIR
        .into_iter()
        .fold(bundle.to_path_buf(), |path, part| path.join(part))
}

/// String value of `key` in `Contents/Info.plist`. `None` when the file or the key is missing.
fn info_string(bundle: &Path, key: &str) -> Option<String> {
    let plist = INFO_PLIST
        .into_iter()
        .fold(bundle.to_path_buf(), |path, part| path.join(part));
    plist::Value::from_file(plist)
        .ok()?
        .as_dictionary()?
        .get(key)?
        .as_string()
        .map(str::to_owned)
}

/// Executable that `CFBundleExecutable` names, below `Contents/MacOS`.
/// `None` when the plist is missing or the value is not a plain file name.
pub fn declared_executable(bundle: &Path) -> Option<PathBuf> {
    let name = info_string(bundle, EXECUTABLE_KEY)?;
    (Path::new(&name).file_name() == Some(OsStr::new(&name)))
        .then(|| executable_dir(bundle).join(name))
}

/// `LSMinimumSystemVersion` of `bundle` when the running macOS is older, else `None`.
pub fn unsupported_min_os(bundle: &Path) -> Option<String> {
    let required = info_string(bundle, MIN_OS_KEY)?;
    let current = System::os_version().ok()?;
    is_older(&current, &required).then_some(required)
}

/// `true` when the dotted version `current` is older than `required`. Missing parts count as 0.
fn is_older(current: &str, required: &str) -> bool {
    let parts = |version: &str| -> Vec<u32> {
        version
            .split('.')
            .map_while(|part| part.parse().ok())
            .collect()
    };
    let (current, required) = (parts(current), parts(required));
    (0..current.len().max(required.len()))
        .map(|index| {
            (
                current.get(index).copied().unwrap_or(0),
                required.get(index).copied().unwrap_or(0),
            )
        })
        .find(|(have, need)| have != need)
        .is_some_and(|(have, need)| have < need)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{declared_executable, is_older};

    fn bundle_with(executable: &str) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let contents = dir.path().join("X.app/Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>{executable}</string></dict></plist>"
        );
        fs::write(contents.join("Info.plist"), plist).unwrap();
        dir
    }

    #[test]
    fn older_version_is_detected_with_padding() {
        assert!(is_older("15.1", "27.0"));
        assert!(is_older("27", "27.0.1"));
        assert!(!is_older("27", "27.0"));
        assert!(!is_older("27.1", "27.0"));
    }

    #[test]
    fn declared_executable_names_a_file_of_contents_macos() {
        let dir = bundle_with("Tool");
        let bundle = dir.path().join("X.app");
        assert_eq!(
            declared_executable(&bundle),
            Some(bundle.join("Contents/MacOS/Tool"))
        );
    }

    #[test]
    fn declared_executable_rejects_a_path() {
        let dir = bundle_with("../Tool");
        assert_eq!(declared_executable(&dir.path().join("X.app")), None);
    }

    #[test]
    fn declared_executable_is_none_without_plist() {
        let dir = tempdir().unwrap();
        assert_eq!(declared_executable(dir.path()), None);
    }
}
