use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::Error;
use crate::model::ensure_relative_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
    /// Entries may sit in sub-directories.
    Tree,
    /// Entries sit directly under the destination.
    Flat,
}

struct Entry {
    index: usize,
    rel: String,
    is_dir: bool,
}

#[derive(Default)]
struct Created {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Created {
    fn roll_back(self) {
        for file in self.files {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.into_iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
}

pub(crate) struct Archive {
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
        }
        Ok(())
    }
}

fn make_dirs(path: &Path, created: &mut Created) -> Result<(), Error> {
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
