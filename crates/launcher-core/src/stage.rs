use std::fs;
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::archive::{Archive, Created, Layout, copy_file, make_dirs};
use crate::model::ensure_relative_path;
use crate::{Error, FileCheck, Hash, Kind, Location, Root, Step, find_executable};

pub const DATA_DIR: &str = "data";

/// Directories that `Location` values resolve against.
pub struct Roots {
    tmp: Option<PathBuf>,
    pub(crate) app: PathBuf,
    pub(crate) data: PathBuf,
}

impl Roots {
    /// `App` is the directory of the executable below `app_dir`.
    /// Only `AviUtl` and `AviUtl2` have catalogs that address files by location.
    pub(crate) fn locate(kind: Kind, app_dir: &Path, tmp: Option<&Path>) -> Result<Self, Error> {
        if !matches!(kind, Kind::Aviutl | Kind::Aviutl2) {
            return Err(Error::PluginUnsupported(kind));
        }
        let exe = find_executable(app_dir, kind, crate::Host::Windows)?;
        let app = exe.parent().unwrap_or(app_dir).to_owned();
        Ok(Self {
            tmp: tmp.map(Path::to_owned),
            data: app.join(DATA_DIR),
            app,
        })
    }

    fn resolve(&self, location: &Location) -> Result<PathBuf, Error> {
        let base = match location.root {
            Root::Tmp => self
                .tmp
                .as_ref()
                .ok_or_else(|| Error::UnsafePath(location.rel.clone()))?,
            Root::App => &self.app,
            Root::Data => &self.data,
        };
        if location.rel.is_empty() {
            return Ok(base.clone());
        }
        ensure_relative_path(&location.rel)?;
        Ok(base.join(location.rel.replace('\\', "/")))
    }
}

/// Runs `steps` in order. `download` is the downloaded archive.
/// Every file and directory that a step creates is recorded in `created`.
pub fn run(
    roots: &Roots,
    download: &Path,
    steps: &[Step],
    created: &mut Created,
) -> Result<(), Error> {
    for step in steps {
        debug!(?step, "step");
        match step {
            Step::Extract => {
                let tmp = roots.resolve(&Location {
                    root: Root::Tmp,
                    rel: String::new(),
                })?;
                Archive::open(download, Layout::Tree)?.extract(&tmp)?;
            }
            Step::Copy { from, to } => {
                copy(&roots.resolve(from)?, &roots.resolve(to)?, created)?;
            }
            Step::Place {
                name,
                archive_dir,
                directory,
            } => place(
                roots,
                download,
                name,
                archive_dir.as_deref(),
                *directory,
                created,
            )?,
        }
    }
    Ok(())
}

/// Compares the hash of every file in `checks` with the file on disk.
pub fn verify(roots: &Roots, checks: &[FileCheck]) -> Result<(), Error> {
    for check in checks {
        let path = roots.resolve(&check.location)?;
        let actual = Hash::of_file(check.hash.algo, &path).map_err(|e| Error::io(&path, e))?;
        if !actual.hex.eq_ignore_ascii_case(&check.hash.hex) {
            return Err(Error::HashMismatch {
                name: check.location.rel.clone(),
                expected: check.hash.hex.clone(),
                actual: actual.hex,
            });
        }
        debug!(file = %check.location.rel, "file hash verified");
    }
    Ok(())
}

/// Copies a file into the directory `to_dir`. Copies the content of a directory into `to_dir`.
fn copy(from: &Path, to_dir: &Path, created: &mut Created) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(from).map_err(|e| Error::io(from, e))?;
    if metadata.is_dir() {
        return copy_tree(from, to_dir, created);
    }
    let name = from
        .file_name()
        .ok_or_else(|| Error::UnsafePath(from.display().to_string()))?;
    copy_file(from, &to_dir.join(name), created)
}

fn copy_tree(from: &Path, to: &Path, created: &mut Created) -> Result<(), Error> {
    make_dirs(to, created)?;
    let mut children = Vec::new();
    for entry in fs::read_dir(from).map_err(|e| Error::io(from, e))? {
        children.push(entry.map_err(|e| Error::io(from, e))?.path());
    }
    children.sort();
    for child in children {
        let Some(name) = child.file_name() else {
            continue;
        };
        let target = to.join(name);
        let file_type = fs::symlink_metadata(&child)
            .map_err(|e| Error::io(&child, e))?
            .file_type();
        if file_type.is_dir() {
            copy_tree(&child, &target, created)?;
        } else if file_type.is_file() {
            copy_file(&child, &target, created)?;
        }
    }
    Ok(())
}

fn place(
    roots: &Roots,
    download: &Path,
    name: &str,
    archive_dir: Option<&str>,
    directory: bool,
    created: &mut Created,
) -> Result<(), Error> {
    let tmp = roots.resolve(&Location {
        root: Root::Tmp,
        rel: String::new(),
    })?;
    let source = find_source(&tmp, download, name, archive_dir, directory)?;
    let target = roots.resolve(&Location {
        root: Root::App,
        rel: name.to_owned(),
    })?;
    debug!(source = %source.display(), target = %target.display(), "place");
    if directory {
        copy_tree(&source, &target, created)
    } else {
        copy_file(&source, &target, created)
    }
}

/// Paths below `dir` relative to `dir`, written with `/`. `skip` is left out.
fn list(dir: &Path, skip: &Path) -> Result<Vec<(String, bool)>, Error> {
    let mut found = Vec::new();
    collect(dir, dir, skip, &mut found)?;
    Ok(found)
}

fn collect(
    base: &Path,
    dir: &Path,
    skip: &Path,
    found: &mut Vec<(String, bool)>,
) -> Result<(), Error> {
    for entry in fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        if path == skip {
            continue;
        }
        let file_type = fs::symlink_metadata(&path)
            .map_err(|e| Error::io(&path, e))?
            .file_type();
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let rel = rel
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if file_type.is_dir() {
            found.push((rel, true));
            collect(base, &path, skip, found)?;
        } else if file_type.is_file() {
            found.push((rel, false));
        }
    }
    Ok(())
}

fn find_source(
    tmp: &Path,
    download: &Path,
    name: &str,
    archive_dir: Option<&str>,
    directory: bool,
) -> Result<PathBuf, Error> {
    let candidates: Vec<String> = list(tmp, download)?
        .into_iter()
        .filter(|(_, is_dir)| *is_dir == directory)
        .map(|(rel, _)| rel)
        .collect();
    let name = name.replace('\\', "/").trim_matches('/').to_owned();
    let base = name.rsplit('/').next().unwrap_or(&name).to_owned();
    let exact = match archive_dir.map(|dir| dir.replace('\\', "/")) {
        Some(dir) if !dir.trim_matches('/').is_empty() => {
            format!("{}/{base}", dir.trim_matches('/'))
        }
        Some(_) => base.clone(),
        None => name.clone(),
    };

    let unique = |matches: Vec<&String>| -> Result<Option<PathBuf>, Error> {
        match matches.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(tmp.join(only.as_str()))),
            _ => Err(Error::EntryAmbiguous(name.clone())),
        }
    };
    let same = |candidate: &String, wanted: &str| candidate.eq_ignore_ascii_case(wanted);
    let ends_with = |candidate: &String, wanted: &str| {
        let candidate = candidate.to_ascii_lowercase();
        let wanted = format!("/{}", wanted.to_ascii_lowercase());
        candidate.ends_with(&wanted)
    };

    if let Some(path) = unique(candidates.iter().filter(|c| same(c, &exact)).collect())? {
        return Ok(path);
    }
    if let Some(path) = unique(
        candidates
            .iter()
            .filter(|c| same(c, &name) || ends_with(c, &name))
            .collect(),
    )? {
        return Ok(path);
    }
    unique(
        candidates
            .iter()
            .filter(|c| same(c, &base) || ends_with(c, &base))
            .collect(),
    )?
    .ok_or_else(|| Error::EntryNotFound(name.clone()))
}
