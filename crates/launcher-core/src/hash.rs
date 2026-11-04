//! Detection of catalog packages whose files exist in an instance without an installer record.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::catalog::{FileCheck, Hash, HashAlgo, Root};

/// Directories under the app directory that hold manually placed plugins and scripts.
pub const SCAN_DIRS: [&str; 4] = ["Plugin", "Plugins", "Script", "Scripts"];

const MAX_THREADS: usize = 8;

/// XXH3-128 values of every file under `SCAN_DIRS` of `app_dir`, subdirectories included.
/// Each value has 32 lowercase hexadecimal digits. Unreadable files are skipped.
#[must_use]
pub fn scan_xxh3_128(app_dir: &Path) -> HashSet<String> {
    let mut files = Vec::new();
    for sub in SCAN_DIRS {
        collect_files(&app_dir.join(sub), &mut files);
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
/// A check holds when the file at its path has the recorded hash,
/// or, for XXH3-128, when a scanned file at another path has that hash.
/// Checks outside `Root::App` do not hold.
#[must_use]
pub fn is_present(app_dir: &Path, checks: &[FileCheck], scanned: &HashSet<String>) -> bool {
    !checks.is_empty() && checks.iter().all(|check| holds(app_dir, check, scanned))
}

fn holds(app_dir: &Path, check: &FileCheck, scanned: &HashSet<String>) -> bool {
    if check.location.root != Root::App {
        return false;
    }
    let path = check
        .location
        .rel
        .split('/')
        .fold(app_dir.to_path_buf(), |path, part| path.join(part));
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
