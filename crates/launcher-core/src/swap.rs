use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::archive::{Created, copy_file, make_dirs};

/// Content of one directory tree, as paths relative to its base.
#[derive(Default)]
struct Tree {
    dirs: Vec<PathBuf>,
    files: Vec<PathBuf>,
}

impl Tree {
    fn collect(&mut self, base: &Path, rel: &Path) -> Result<(), Error> {
        let dir = base.join(rel);
        for entry in fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
            let entry = entry.map_err(|e| Error::io(&dir, e))?;
            let child = rel.join(entry.file_name());
            let file_type = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
            if file_type.is_dir() {
                self.dirs.push(child.clone());
                self.collect(base, &child)?;
            } else if file_type.is_file() {
                self.files.push(child);
            }
        }
        Ok(())
    }
}

/// Copy of a source tree over a destination tree.
/// A destination file that exists moves to the backup directory first.
/// `roll_back` removes the copied files and moves the backups back.
pub struct Swap {
    moved: Vec<(PathBuf, PathBuf)>,
    created: Created,
}

impl Swap {
    /// Copies every file below `source` to the same relative path below `dest`.
    /// A failure rolls back the copy before the error returns.
    pub(crate) fn apply(source: &Path, dest: &Path, backup: &Path) -> Result<Self, Error> {
        let mut tree = Tree::default();
        tree.collect(source, Path::new(""))?;
        let mut swap = Self {
            moved: Vec::new(),
            created: Created::default(),
        };
        match swap.write(&tree, source, dest, backup) {
            Ok(()) => Ok(swap),
            Err(error) => {
                swap.roll_back();
                Err(error)
            }
        }
    }

    pub(crate) fn roll_back(self) {
        self.created.roll_back();
        for (target, saved) in self.moved.into_iter().rev() {
            let _ = fs::rename(saved, target);
        }
    }

    fn write(
        &mut self,
        tree: &Tree,
        source: &Path,
        dest: &Path,
        backup: &Path,
    ) -> Result<(), Error> {
        for dir in &tree.dirs {
            make_dirs(&dest.join(dir), &mut self.created)?;
        }
        for rel in &tree.files {
            let target = dest.join(rel);
            if target.is_file() {
                let saved = backup.join(rel);
                if let Some(parent) = saved.parent() {
                    fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
                }
                fs::rename(&target, &saved).map_err(|e| Error::io(&target, e))?;
                self.moved.push((target.clone(), saved));
            }
            let from = source.join(rel);
            copy_file(&from, &target, &mut self.created)?;
            let permissions = fs::metadata(&from)
                .map_err(|e| Error::io(&from, e))?
                .permissions();
            fs::set_permissions(&target, permissions).map_err(|e| Error::io(&target, e))?;
        }
        Ok(())
    }
}
