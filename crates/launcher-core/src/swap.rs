use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::archive::{Created, copy_file, copy_link, make_dirs};

/// Content of one directory tree, as paths relative to its base.
#[derive(Default)]
struct Tree {
    dirs: Vec<PathBuf>,
    files: Vec<PathBuf>,
    links: Vec<PathBuf>,
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
            } else if file_type.is_symlink() {
                self.links.push(child);
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
    /// Directories that `replace_dir` moved into place.
    placed: Vec<PathBuf>,
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
            placed: Vec::new(),
        };
        match swap.write(&tree, source, dest, backup) {
            Ok(()) => Ok(swap),
            Err(error) => {
                swap.roll_back();
                Err(error)
            }
        }
    }

    /// Replaces the directory `dest` with the directory `source` as a whole.
    /// `dest` moves to `backup` first, then `source` moves to `dest`.
    /// Both moves are renames, so `source`, `dest`, and `backup` must share one volume.
    /// A failure rolls back before the error returns.
    pub(crate) fn replace_dir(source: &Path, dest: &Path, backup: &Path) -> Result<Self, Error> {
        let mut swap = Self {
            moved: Vec::new(),
            created: Created::default(),
            placed: Vec::new(),
        };
        match swap.move_dir(source, dest, backup) {
            Ok(()) => Ok(swap),
            Err(error) => {
                swap.roll_back();
                Err(error)
            }
        }
    }

    fn move_dir(&mut self, source: &Path, dest: &Path, backup: &Path) -> Result<(), Error> {
        if fs::symlink_metadata(dest).is_ok() {
            let name = dest
                .file_name()
                .ok_or_else(|| Error::UnsafePath(dest.display().to_string()))?;
            let saved = backup.join(name);
            fs::create_dir_all(backup).map_err(|e| Error::io(backup, e))?;
            fs::rename(dest, &saved).map_err(|e| Error::io(dest, e))?;
            self.moved.push((dest.to_owned(), saved));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        fs::rename(source, dest).map_err(|e| Error::io(dest, e))?;
        self.placed.push(dest.to_owned());
        Ok(())
    }

    pub(crate) fn roll_back(self) {
        self.created.roll_back();
        for dir in self.placed {
            let _ = fs::remove_dir_all(dir);
        }
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
            self.back_up(&target, rel, backup)?;
            let from = source.join(rel);
            copy_file(&from, &target, &mut self.created)?;
            let permissions = fs::metadata(&from)
                .map_err(|e| Error::io(&from, e))?
                .permissions();
            fs::set_permissions(&target, permissions).map_err(|e| Error::io(&target, e))?;
        }
        for rel in &tree.links {
            let target = dest.join(rel);
            self.back_up(&target, rel, backup)?;
            copy_link(&source.join(rel), &target, &mut self.created)?;
        }
        Ok(())
    }

    /// Moves a file or link at `target` to `backup/<rel>`. A missing or directory `target` stays.
    fn back_up(&mut self, target: &Path, rel: &Path, backup: &Path) -> Result<(), Error> {
        if fs::symlink_metadata(target).is_ok_and(|metadata| !metadata.is_dir()) {
            let saved = backup.join(rel);
            if let Some(parent) = saved.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            fs::rename(target, &saved).map_err(|e| Error::io(target, e))?;
            self.moved.push((target.to_owned(), saved));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::Swap;

    fn bundle(path: &std::path::Path, marker: &str) {
        fs::create_dir_all(path.join("Contents")).unwrap();
        fs::write(path.join("Contents/marker"), marker).unwrap();
    }

    #[test]
    fn replace_dir_drops_files_of_the_old_directory() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("app/X.app");
        let source = dir.path().join("new/X.app");
        bundle(&dest, "old");
        fs::write(dest.join("Contents/stale"), "stale").unwrap();
        bundle(&source, "new");

        let swap = Swap::replace_dir(&source, &dest, &dir.path().join("old")).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("Contents/marker")).unwrap(),
            "new"
        );
        assert!(!dest.join("Contents/stale").exists());
        drop(swap);
    }

    #[test]
    fn roll_back_restores_the_old_directory() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("app/X.app");
        let source = dir.path().join("new/X.app");
        bundle(&dest, "old");
        bundle(&source, "new");

        Swap::replace_dir(&source, &dest, &dir.path().join("old"))
            .unwrap()
            .roll_back();

        assert_eq!(
            fs::read_to_string(dest.join("Contents/marker")).unwrap(),
            "old"
        );
    }

    #[test]
    fn a_missing_source_leaves_the_old_directory_in_place() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("app/X.app");
        bundle(&dest, "old");

        let result = Swap::replace_dir(&dir.path().join("none"), &dest, &dir.path().join("old"));

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(dest.join("Contents/marker")).unwrap(),
            "old"
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_keeps_links_and_roll_back_restores_the_old_link() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let source = dir.path().join("new");
        let dest = dir.path().join("app");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&dest).unwrap();
        symlink("a.so.2", source.join("a.so")).unwrap();
        symlink("a.so.1", dest.join("a.so")).unwrap();

        let swap = Swap::apply(&source, &dest, &dir.path().join("old")).unwrap();
        assert_eq!(
            fs::read_link(dest.join("a.so")).unwrap().to_str(),
            Some("a.so.2")
        );

        swap.roll_back();
        assert_eq!(
            fs::read_link(dest.join("a.so")).unwrap().to_str(),
            Some("a.so.1")
        );
    }
}
