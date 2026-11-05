use launcher_core::{Location, Root, Step};
use url::Url;
use xxhash_rust::xxh3::xxh3_128;

pub const AU2PKG_ALLOWED_ROOTS: &[&str] = &[
    "Plugin",
    "Script",
    "Language",
    "Alias",
    "Figure",
    "Transition",
    "Preset",
    "Default",
];

/// Checks if a file path has an au2pkg extension (`.au2pkg.zip` or `.au2pkg`).
#[must_use]
pub fn is_au2pkg_filename(file_path: &str) -> bool {
    let lower = file_path.to_ascii_lowercase();
    lower.ends_with(".au2pkg.zip") || lower.ends_with(".au2pkg")
}

/// Computes the 32-character hexadecimal XXH3-128 hash of bytes.
#[must_use]
pub fn calc_xxh3_128_hex(bytes: &[u8]) -> String {
    let h = xxh3_128(bytes);
    format!("{:032x}", h)
}

/// Resolves Google Drive file ID to a direct download URL.
#[must_use]
pub fn google_drive_direct_url(file_id: &str) -> String {
    format!("https://drive.google.com/uc?export=download&id={file_id}")
}

/// Extracts Google Drive file ID from various URL formats.
#[must_use]
pub fn extract_google_drive_id(url: &Url) -> Option<String> {
    if !url
        .host_str()
        .is_some_and(|h| h.contains("drive.google.com"))
    {
        return None;
    }
    // Check query params: ?id=...
    for (key, val) in url.query_pairs() {
        if key == "id" {
            return Some(val.into_owned());
        }
    }
    // Check path: /file/d/{id}/...
    let segments: Vec<&str> = url.path_segments()?.collect();
    let pos = segments.iter().position(|&s| s == "d")?;
    segments.get(pos + 1).map(|id| (*id).to_owned())
}

/// Checks if a URL points to BOOTH.
#[must_use]
pub fn is_booth_url(url: &Url) -> bool {
    url.host_str().is_some_and(|h| h.ends_with("booth.pm"))
}

/// Checks if a URL is a BOOTH login / auth page.
#[must_use]
pub fn is_booth_login_url(url: &Url) -> bool {
    if !is_booth_url(url) {
        return false;
    }
    let p = url.path().trim_end_matches('/');
    p.starts_with("/users/sign_in")
        || p.starts_with("/users/sign_in_by_password")
        || p.starts_with("/users/password/new")
}

/// A file of an `.au2pkg.zip` that the launcher installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Au2pkgFile {
    /// Entry path in the archive: `<root>/<relative path>`, with the canonical root name.
    pub archive_path: String,
    /// Canonical name of the first path component, one of `AU2PKG_ALLOWED_ROOTS`.
    pub root: &'static str,
    /// Directory below `<App>/data` that receives the file.
    pub install_to: Location,
}

fn normalize_path(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

fn root_priority(root: &str) -> u8 {
    match root {
        "Plugin" => 0,
        "Script" => 1,
        _ => 2,
    }
}

/// Selects the entries whose first component names an allowed root and that hold a file below it.
/// An entry with a `..` component is dropped. Duplicates collapse into one.
/// The result lists `Plugin`, then `Script`, then the other roots, each sorted by path.
#[must_use]
pub fn collect_au2pkg_files(entries: &[String]) -> Vec<Au2pkgFile> {
    let mut files: Vec<Au2pkgFile> = Vec::new();
    for entry in entries {
        let parts = normalize_path(entry);
        if parts.len() < 2 || parts.contains(&"..") {
            continue;
        }
        let Some(&root) = AU2PKG_ALLOWED_ROOTS
            .iter()
            .find(|root| root.eq_ignore_ascii_case(parts[0]))
        else {
            continue;
        };
        let archive_path = parts.join("/").replacen(parts[0], root, 1);
        if files.iter().any(|file| file.archive_path == archive_path) {
            continue;
        }
        let parent = parts[1..parts.len() - 1].join("/");
        let rel = if parent.is_empty() {
            root.to_owned()
        } else {
            format!("{root}/{parent}")
        };
        files.push(Au2pkgFile {
            archive_path,
            root,
            install_to: Location {
                root: Root::Data,
                rel,
            },
        });
    }
    files.sort_by(|a, b| {
        root_priority(a.root)
            .cmp(&root_priority(b.root))
            .then_with(|| a.archive_path.cmp(&b.archive_path))
    });
    files
}

/// Extracts the archive, then copies each file into its directory below `<App>/data`.
#[must_use]
pub fn build_au2pkg_steps(files: &[Au2pkgFile]) -> Vec<Step> {
    let mut steps = vec![Step::Extract];
    steps.extend(files.iter().map(|file| Step::Copy {
        from: Location {
            root: Root::Tmp,
            rel: file.archive_path.clone(),
        },
        to: file.install_to.clone(),
    }));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_au2pkg() {
        assert!(is_au2pkg_filename("my-package.au2pkg.zip"));
        assert!(is_au2pkg_filename("my-package.au2pkg"));
        assert!(!is_au2pkg_filename("my-package.zip"));
    }

    #[test]
    fn test_xxh3_128_hex() {
        let hash = calc_xxh3_128_hex(b"hello world");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_google_drive() {
        let url =
            Url::parse("https://drive.google.com/file/d/12345abcdef/view?usp=sharing").unwrap();
        assert_eq!(
            extract_google_drive_id(&url),
            Some("12345abcdef".to_owned())
        );
        assert_eq!(
            google_drive_direct_url("12345abcdef"),
            "https://drive.google.com/uc?export=download&id=12345abcdef"
        );
    }

    fn names(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|entry| (*entry).to_owned()).collect()
    }

    #[test]
    fn collects_allowed_roots_in_install_order() {
        let files = collect_au2pkg_files(&names(&[
            "Script/effect.anm",
            "plugin/sub/a.auf2",
            "Plugin/b.dll",
            "Language/ja.txt",
            "Other/readme.txt",
            "readme.txt",
            "Plugin/b.dll",
            "Plugin/../evil.dll",
        ]));
        let paths: Vec<&str> = files.iter().map(|f| f.archive_path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Plugin/b.dll",
                "Plugin/sub/a.auf2",
                "Script/effect.anm",
                "Language/ja.txt"
            ]
        );
        assert_eq!(files[1].install_to.rel, "Plugin/sub");
        assert_eq!(files[1].install_to.root, Root::Data);
        assert_eq!(files[0].install_to.rel, "Plugin");
        assert_eq!(files[3].install_to.rel, "Language");
    }

    #[test]
    fn steps_extract_then_copy_each_file_into_its_directory() {
        let files = collect_au2pkg_files(&names(&["Plugin/sub/a.auf2", "Script\\b.anm"]));
        let steps = build_au2pkg_steps(&files);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], Step::Extract);
        assert_eq!(
            steps[1],
            Step::Copy {
                from: Location {
                    root: Root::Tmp,
                    rel: "Plugin/sub/a.auf2".to_owned()
                },
                to: Location {
                    root: Root::Data,
                    rel: "Plugin/sub".to_owned()
                },
            }
        );
    }
}
