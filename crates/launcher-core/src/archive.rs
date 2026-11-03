use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::Error;
use crate::model::ensure_relative_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Entries may sit in sub-directories.
    Tree,
    /// Like `Tree`. When one directory holds every entry, that directory is dropped from the paths.
    TreeWithoutRoot,
    /// Entries sit directly under the destination.
    Flat,
}

struct Entry {
    index: usize,
    rel: String,
    is_dir: bool,
}

/// Files and directories that a task created, in creation order.
#[derive(Default)]
pub struct Created {
    pub(crate) files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Created {
    pub(crate) fn roll_back(self) {
        for file in self.files {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.into_iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
}

pub struct Archive {
    path: PathBuf,
    zip: ZipArchive<BufReader<File>>,
    entries: Vec<Entry>,
}

impl Archive {
    /// Validates every entry name and type. Writes nothing.
    pub(crate) fn open(path: &Path, layout: Layout) -> Result<Self, Error> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let mut zip = ZipArchive::new(BufReader::new(file)).map_err(|e| Error::zip(path, e))?;
        let mut entries = Vec::with_capacity(zip.len());
        let mut seen = HashSet::new();
        for index in 0..zip.len() {
            let entry = zip.by_index_raw(index).map_err(|e| Error::zip(path, e))?;
            let name = entry.name().map_err(|e| Error::zip(path, e))?.into_owned();
            if entry.is_symlink() {
                return Err(Error::SymlinkEntry(name));
            }
            let is_dir = entry.is_dir();
            let rel = name.replace('\\', "/").trim_end_matches('/').to_owned();
            ensure_relative_path(&rel)?;
            if layout == Layout::Flat && (is_dir || name.contains(['/', '\\'])) {
                return Err(Error::UnsafePath(name));
            }
            if !seen.insert(rel.clone()) {
                return Err(Error::UnsafePath(name));
            }
            entries.push(Entry { index, rel, is_dir });
        }
        if layout == Layout::TreeWithoutRoot {
            strip_common_root(&mut entries);
        }
        Ok(Self {
            path: path.to_owned(),
            zip,
            entries,
        })
    }

    pub(crate) fn files(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.rel.clone())
            .collect()
    }

    /// Creates every file with `create_new`. On failure removes the files and directories it created.
    pub(crate) fn extract(&mut self, dest: &Path) -> Result<(), Error> {
        let mut created = Created::default();
        let result = self.write_entries(dest, &mut created);
        if result.is_err() {
            created.roll_back();
        }
        result
    }

    fn write_entries(&mut self, dest: &Path, created: &mut Created) -> Result<(), Error> {
        for entry in &self.entries {
            let target = dest.join(&entry.rel);
            if entry.is_dir {
                make_dirs(&target, created)?;
                continue;
            }
            if let Some(parent) = target.parent() {
                make_dirs(parent, created)?;
            }
            let mut source = self
                .zip
                .by_index(entry.index)
                .map_err(|e| Error::zip(&self.path, e))?;
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| Error::io(&target, e))?;
            created.files.push(target.clone());
            io::copy(&mut source, &mut out).map_err(|e| Error::io(&target, e))?;
            #[cfg(unix)]
            set_mode(&target, source.unix_mode())?;
        }
        Ok(())
    }
}

/// Removes the first path component of every entry when one directory holds all entries.
fn strip_common_root(entries: &mut Vec<Entry>) {
    let Some(root) = entries
        .first()
        .and_then(|entry| entry.rel.split('/').next())
        .map(str::to_owned)
    else {
        return;
    };
    let prefix = format!("{root}/");
    let all_inside = entries
        .iter()
        .all(|entry| (entry.is_dir && entry.rel == root) || entry.rel.starts_with(&prefix));
    let has_child = entries.iter().any(|entry| entry.rel.starts_with(&prefix));
    if !(all_inside && has_child) {
        return;
    }
    entries.retain(|entry| entry.rel != root);
    for entry in entries.iter_mut() {
        entry.rel = entry.rel[prefix.len()..].to_owned();
    }
}

/// Sets 0o755 on entries that carry an executable bit in the zip, 0o644 on the others.
#[cfg(unix)]
fn set_mode(path: &Path, zip_mode: Option<u32>) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;

    let executable = zip_mode.is_some_and(|mode| mode & 0o111 != 0);
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| Error::io(path, e))
}

pub fn make_dirs(path: &Path, created: &mut Created) -> Result<(), Error> {
    let mut missing = Vec::new();
    let mut current = Some(path);
    while let Some(dir) = current {
        if dir.exists() {
            break;
        }
        missing.push(dir.to_owned());
        current = dir.parent();
    }
    for dir in missing.into_iter().rev() {
        fs::create_dir(&dir).map_err(|e| Error::io(&dir, e))?;
        created.dirs.push(dir);
    }
    Ok(())
}

/// Copies `source` to `target` with `create_new`, creating the missing parent directories.
/// Fails with `AlreadyExists` when `target` exists.
pub fn copy_file(source: &Path, target: &Path, created: &mut Created) -> Result<(), Error> {
    if let Some(parent) = target.parent() {
        make_dirs(parent, created)?;
    }
    let mut input = File::open(source).map_err(|e| Error::io(source, e))?;
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => Error::AlreadyExists(target.to_owned()),
            _ => Error::io(target, e),
        })?;
    created.files.push(target.to_owned());
    io::copy(&mut input, &mut out).map_err(|e| Error::io(target, e))?;
    Ok(())
}
