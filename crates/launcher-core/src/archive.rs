use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::Error;
use crate::model::{ensure_relative_path, ensure_single_component};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Entries may sit in sub-directories.
    Tree,
    /// Like `Tree`. When one directory holds every entry, that directory is dropped from the paths.
    TreeWithoutRoot,
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

const MAGIC_LEN: usize = 4;
const ZSTD_MAGIC: [u8; MAGIC_LEN] = [0x28, 0xB5, 0x2F, 0xFD];
const ELF_MAGIC: [u8; MAGIC_LEN] = [0x7F, b'E', b'L', b'F'];

enum Source {
    Zip(ZipArchive<BufReader<File>>),
    /// Zstandard-compressed tar. `Entry::index` is the position in the stream.
    TarZstd,
    /// One file that is stored under `Entry::rel`.
    Single,
}

/// Container of a download, told by its first bytes.
enum Format {
    Zip,
    TarZstd,
    Executable,
}

fn sniff(path: &Path) -> Result<Format, Error> {
    let mut head = Vec::with_capacity(MAGIC_LEN);
    File::open(path)
        .map_err(|e| Error::io(path, e))?
        .take(MAGIC_LEN as u64)
        .read_to_end(&mut head)
        .map_err(|e| Error::io(path, e))?;
    Ok(if head == ELF_MAGIC {
        Format::Executable
    } else if head == ZSTD_MAGIC {
        Format::TarZstd
    } else {
        Format::Zip
    })
}

pub struct Archive {
    path: PathBuf,
    source: Source,
    entries: Vec<Entry>,
}

impl Archive {
    /// Opens a zip. Validates every entry name and type. Writes nothing.
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
            if !seen.insert(rel.clone()) {
                return Err(Error::UnsafePath(name));
            }
            entries.push(Entry { index, rel, is_dir });
        }
        Ok(Self::new(path, Source::Zip(zip), entries, layout))
    }

    /// Like `open`, and accepts the other downloads of an app: a Zstandard-compressed tar,
    /// and a bare executable, which becomes the single file `single_name`.
    pub(crate) fn open_app(path: &Path, layout: Layout, single_name: &str) -> Result<Self, Error> {
        match sniff(path)? {
            Format::Zip => Self::open(path, layout),
            Format::TarZstd => Self::open_tar(path, layout),
            Format::Executable => {
                ensure_single_component(single_name)?;
                let entry = Entry {
                    index: 0,
                    rel: single_name.to_owned(),
                    is_dir: false,
                };
                Ok(Self::new(path, Source::Single, vec![entry], layout))
            }
        }
    }

    fn open_tar(path: &Path, layout: Layout) -> Result<Self, Error> {
        let mut tar = tar_stream(path)?;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for (index, entry) in tar.entries().map_err(|e| Error::tar(path, e))?.enumerate() {
            let entry = entry.map_err(|e| Error::tar(path, e))?;
            let kind = entry.header().entry_type();
            if kind.is_pax_global_extensions() {
                continue;
            }
            let name = entry
                .path()
                .map_err(|e| Error::tar(path, e))?
                .to_string_lossy()
                .into_owned();
            if kind.is_symlink() || kind.is_hard_link() {
                return Err(Error::SymlinkEntry(name));
            }
            let is_dir = kind.is_dir();
            if !is_dir && !kind.is_file() {
                return Err(Error::UnsafePath(name));
            }
            let normalized = name.replace('\\', "/");
            let rel = normalized.trim_start_matches("./").trim_end_matches('/');
            if rel.is_empty() || rel == "." {
                continue;
            }
            ensure_relative_path(rel)?;
            if !seen.insert(rel.to_owned()) {
                return Err(Error::UnsafePath(name));
            }
            entries.push(Entry {
                index,
                rel: rel.to_owned(),
                is_dir,
            });
        }
        Ok(Self::new(path, Source::TarZstd, entries, layout))
    }

    fn new(path: &Path, source: Source, mut entries: Vec<Entry>, layout: Layout) -> Self {
        if layout == Layout::TreeWithoutRoot {
            strip_common_root(&mut entries);
        }
        Self {
            path: path.to_owned(),
            source,
            entries,
        }
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
        let result = match &mut self.source {
            Source::Zip(zip) => write_zip(zip, &self.entries, &self.path, dest, &mut created),
            Source::TarZstd => write_tar(&self.entries, &self.path, dest, &mut created),
            Source::Single => write_single(&self.entries, &self.path, dest, &mut created),
        };
        if result.is_err() {
            created.roll_back();
        }
        result
    }
}

fn tar_stream(path: &Path) -> Result<tar::Archive<impl Read>, Error> {
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let decoder = zstd::stream::read::Decoder::new(file).map_err(|e| Error::tar(path, e))?;
    Ok(tar::Archive::new(decoder))
}

/// Creates `target` with `create_new` and records it in `created`.
fn create_file(target: &Path, created: &mut Created) -> Result<File, Error> {
    let out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| Error::io(target, e))?;
    created.files.push(target.to_owned());
    Ok(out)
}

fn write_zip(
    zip: &mut ZipArchive<BufReader<File>>,
    entries: &[Entry],
    archive: &Path,
    dest: &Path,
    created: &mut Created,
) -> Result<(), Error> {
    for entry in entries {
        let target = dest.join(&entry.rel);
        if entry.is_dir {
            make_dirs(&target, created)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            make_dirs(parent, created)?;
        }
        let mut source = zip
            .by_index(entry.index)
            .map_err(|e| Error::zip(archive, e))?;
        let mut out = create_file(&target, created)?;
        io::copy(&mut source, &mut out).map_err(|e| Error::io(&target, e))?;
        #[cfg(unix)]
        set_mode(&target, source.unix_mode())?;
    }
    Ok(())
}

fn write_tar(
    entries: &[Entry],
    archive: &Path,
    dest: &Path,
    created: &mut Created,
) -> Result<(), Error> {
    let mut tar = tar_stream(archive)?;
    let mut wanted = entries.iter().peekable();
    for (index, entry) in tar
        .entries()
        .map_err(|e| Error::tar(archive, e))?
        .enumerate()
    {
        let Some(want) = wanted.next_if(|want| want.index == index) else {
            continue;
        };
        let mut entry = entry.map_err(|e| Error::tar(archive, e))?;
        let target = dest.join(&want.rel);
        if want.is_dir {
            make_dirs(&target, created)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            make_dirs(parent, created)?;
        }
        let mut out = create_file(&target, created)?;
        io::copy(&mut entry, &mut out).map_err(|e| Error::tar(archive, e))?;
        #[cfg(unix)]
        set_mode(&target, entry.header().mode().ok())?;
    }
    Ok(())
}

/// Copies the downloaded executable to its entry and sets the executable bits.
fn write_single(
    entries: &[Entry],
    source: &Path,
    dest: &Path,
    created: &mut Created,
) -> Result<(), Error> {
    for entry in entries {
        let target = dest.join(&entry.rel);
        copy_file(source, &target, created)?;
        #[cfg(unix)]
        set_mode(&target, Some(0o755))?;
    }
    Ok(())
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
        if let Some(stripped) = entry.rel.strip_prefix(&prefix).map(str::to_owned) {
            entry.rel = stripped;
        }
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

/// Names of the file entries of the zip at `path`. Separators are `/`.
///
/// # Errors
///
/// Returns `Error::Io` when `path` cannot be opened and `Error::Zip` when it is not a zip
/// or an entry cannot be read.
pub fn zip_entry_names(path: &Path) -> Result<Vec<String>, Error> {
    let file = File::open(path).map_err(|source| Error::io(path, source))?;
    let mut zip =
        ZipArchive::new(BufReader::new(file)).map_err(|source| Error::zip(path, source))?;
    let mut names = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|source| Error::zip(path, source))?;
        if !entry.is_dir() {
            let name = entry.name().map_err(|source| Error::zip(path, source))?;
            names.push(name.replace('\\', "/"));
        }
    }
    Ok(names)
}

/// Reads the file entry `name` of the zip at `path` (case-insensitive, `/` separators).
/// Returns `None` when there is no such entry or it exceeds `limit` bytes.
///
/// # Errors
///
/// Returns `Error::Io` when `path` cannot be opened and `Error::Zip` when it is not a zip.
pub fn zip_read_entry(path: &Path, name: &str, limit: u64) -> Result<Option<Vec<u8>>, Error> {
    let file = File::open(path).map_err(|source| Error::io(path, source))?;
    let mut zip =
        ZipArchive::new(BufReader::new(file)).map_err(|source| Error::zip(path, source))?;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|source| Error::zip(path, source))?;
        if entry.is_dir() {
            continue;
        }
        let Ok(entry_name) = entry.name() else {
            continue;
        };
        if !entry_name.replace('\\', "/").eq_ignore_ascii_case(name) || entry.size() > limit {
            continue;
        }
        let mut bytes = Vec::new();
        entry
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|source| Error::io(path, source))?;
        return Ok(Some(bytes));
    }
    Ok(None)
}

const SEVEN_ZIP_SIGNATURE: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

/// Offset of the first 7z signature in `bytes`.
fn find_seven_zip_signature(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(SEVEN_ZIP_SIGNATURE.len())
        .position(|window| window == SEVEN_ZIP_SIGNATURE)
}

/// Extracts the 7z data of the self-extracting executable `sfx` into `dest`.
/// An entry whose path leaves `dest` is skipped.
///
/// # Errors
///
/// Returns `Error::Io` when `sfx` cannot be read or `dest` cannot be created,
/// and `Error::Zip` when `sfx` holds no 7z data or the extraction fails.
pub fn extract_sfx(sfx: &Path, dest: &Path) -> Result<(), Error> {
    let bytes = fs::read(sfx).map_err(|source| Error::io(sfx, source))?;
    let payload = find_seven_zip_signature(&bytes)
        .and_then(|offset| bytes.get(offset..))
        .ok_or_else(|| Error::zip(sfx, "7z signature not found"))?;
    fs::create_dir_all(dest).map_err(|source| Error::io(dest, source))?;
    sevenz_rust2::decompress_with_extract_fn_and_password(
        Cursor::new(payload),
        dest,
        sevenz_rust2::Password::empty(),
        |entry, reader, target| {
            if ensure_relative_path(entry.name()).is_err() {
                return Ok(true);
            }
            sevenz_rust2::default_entry_extract_fn(entry, reader, target)
        },
    )
    .map_err(|source| Error::zip(sfx, source))
}

#[cfg(test)]
mod sfx_tests {
    use super::{SEVEN_ZIP_SIGNATURE, find_seven_zip_signature};

    #[test]
    fn signature_is_found_after_the_executable_stub() {
        let mut bytes = vec![0x4D, 0x5A, 0x00, 0x37, 0x7A];
        let offset = bytes.len();
        bytes.extend_from_slice(&SEVEN_ZIP_SIGNATURE);
        bytes.extend_from_slice(&[0, 4]);
        assert_eq!(find_seven_zip_signature(&bytes), Some(offset));
    }

    #[test]
    fn signature_is_absent_from_a_plain_executable() {
        assert_eq!(find_seven_zip_signature(&[0x4D, 0x5A, 0x90, 0x00]), None);
    }
}
