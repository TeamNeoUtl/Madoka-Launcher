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

/// Normalizes archive relative path.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

/// Converts au2pkg root folder to target installation path under App.
fn target_rel_path(root: &str, relative: &str) -> String {
    match root.to_ascii_lowercase().as_str() {
        "plugin" => format!("plugins/{relative}"),
        "script" => format!("scripts/{relative}"),
        other => format!("data/{other}/{relative}"),
    }
}

/// Builds installation steps for entries inside an `.au2pkg` archive.
#[must_use]
pub fn build_au2pkg_steps(entries: &[&str]) -> Vec<Step> {
    let mut steps = vec![Step::Extract];

    for entry in entries {
        let norm = normalize_path(entry);
        if norm.is_empty() || norm.ends_with('/') {
            continue;
        }
        let parts: Vec<&str> = norm.split('/').collect();
        if parts.len() < 2 {
            continue;
        }
        let root_dir = parts[0];
        let is_allowed = AU2PKG_ALLOWED_ROOTS
            .iter()
            .any(|r| r.eq_ignore_ascii_case(root_dir));
        if !is_allowed {
            continue;
        }

        let rel = parts[1..].join("/");
        let dest = target_rel_path(root_dir, &rel);

        steps.push(Step::Copy {
            from: Location {
                root: Root::Tmp,
                rel: norm,
            },
            to: Location {
                root: Root::App,
                rel: dest,
            },
        });
    }

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

    #[test]
    fn test_build_steps() {
        let entries = [
            "Plugin/myplugin.dll",
            "Script/effect.anm",
            "Other/readme.txt",
        ];
        let steps = build_au2pkg_steps(&entries);
        assert_eq!(steps.len(), 3); // Extract + 2 copied
    }
}
