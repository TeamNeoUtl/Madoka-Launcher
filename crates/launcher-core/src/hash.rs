//! Detection of catalog packages whose files exist in an instance without an installer record.

use std::collections::HashSet;
use std::fs;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::thread;

use crate::catalog::{AppVersion, FileCheck, Hash, HashAlgo, Root};
use crate::runner::find_executable;
use crate::stage::DATA_DIR;
use crate::{Host, Kind};

/// Directories under the app directory and under its `data` directory
/// that hold manually placed plugins and scripts.
pub const SCAN_DIRS: [&str; 4] = ["Plugin", "Plugins", "Script", "Scripts"];

const MAX_THREADS: usize = 8;

/// XXH3-128 values of every file under `SCAN_DIRS` of `app_dir` and of `app_dir/data`,
/// subdirectories included.
/// Each value has 32 lowercase hexadecimal digits. Unreadable files are skipped.
#[must_use]
pub fn scan_xxh3_128(app_dir: &Path) -> HashSet<String> {
    let mut files = Vec::new();
    for base in [app_dir.to_path_buf(), app_dir.join(DATA_DIR)] {
        for sub in SCAN_DIRS {
            collect_files(&base.join(sub), &mut files);
        }
    }
    if files.is_empty() {
        return HashSet::new();
    }
    let threads = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_THREADS)
        .min(files.len());
    let chunk = files.len().div_ceil(threads);
    thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every thread must start before the first join"
        )]
        let workers: Vec<_> = files
            .chunks(chunk)
            .map(|paths| {
                scope.spawn(move || {
                    paths
                        .iter()
                        .filter_map(|path| Hash::of_file(HashAlgo::Xxh3_128, path).ok())
                        .map(|hash| hash.hex)
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

/// Holds when `checks` is not empty and every check holds.
///
/// A check holds when the file at its path has the recorded hash,
/// or, for XXH3-128, when a scanned file at another path has that hash.
/// `app_dir` is the directory of the executable. `Root::Tmp` checks do not hold.
#[must_use]
pub fn is_present<S: BuildHasher>(
    app_dir: &Path,
    checks: &[FileCheck],
    scanned: &HashSet<String, S>,
) -> bool {
    !checks.is_empty() && checks.iter().all(|check| holds(app_dir, check, scanned))
}

/// Version of `versions` whose recorded files all match the install at `app_dir`.
///
/// `versions` holds the newest first. Versions can share the same files, so the oldest match
/// is returned: a wrong label then offers an update instead of hiding one.
/// `None` when `app_dir` holds no executable of `kind` or no version matches.
#[must_use]
pub fn detect_version<'a>(
    app_dir: &Path,
    kind: Kind,
    host: Host,
    versions: &'a [AppVersion],
) -> Option<&'a AppVersion> {
    let exe = find_executable(app_dir, kind, host).ok()?;
    let exe_dir = exe.parent()?;
    let scanned = HashSet::new();
    versions
        .iter()
        .rev()
        .find(|version| is_present(exe_dir, &version.checks, &scanned))
}

/// Path of the file that `check` names, below `app_dir`, the directory of the executable.
/// `None` for `Root::Tmp`.
#[must_use]
pub fn check_path(app_dir: &Path, check: &FileCheck) -> Option<PathBuf> {
    let base = match check.location.root {
        Root::App => app_dir.to_path_buf(),
        Root::Data => app_dir.join(DATA_DIR),
        Root::Tmp => return None,
    };
    Some(
        check
            .location
            .rel
            .split('/')
            .fold(base, |path, part| path.join(part)),
    )
}

fn holds<S: BuildHasher>(app_dir: &Path, check: &FileCheck, scanned: &HashSet<String, S>) -> bool {
    let Some(path) = check_path(app_dir, check) else {
        return false;
    };
    let expected = check.hash.hex.to_ascii_lowercase();
    if Hash::of_file(check.hash.algo, &path).is_ok_and(|hash| hash.hex == expected) {
        return true;
    }
    check.hash.algo == HashAlgo::Xxh3_128 && scanned.contains(&expected)
}

/// Appends the regular files under `dir`. Symbolic links are not followed.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(&entry.path(), files);
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
}
