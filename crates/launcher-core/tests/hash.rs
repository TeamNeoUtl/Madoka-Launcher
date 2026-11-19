#![expect(
    clippy::expect_used,
    reason = "test helpers outside #[test] functions may panic"
)]

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use launcher_core::{
    AppVersion, FileCheck, Hash, HashAlgo, Host, Kind, Location, Root, check_path, detect_version,
    is_present, scan_xxh3_128,
};
use tempfile::tempdir;

fn write(app: &Path, rel: &str, bytes: &[u8]) -> Hash {
    let path = app.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(&path, bytes).expect("write");
    Hash::of_file(HashAlgo::Xxh3_128, &path).expect("hash")
}

fn check(rel: &str, hash: Hash) -> FileCheck {
    FileCheck {
        location: Location {
            root: Root::App,
            rel: rel.to_owned(),
        },
        hash,
    }
}

#[test]
fn scan_collects_nested_files() {
    let dir = tempdir().expect("tempdir");
    let top = write(dir.path(), "Plugin/a.auo2", b"a");
    let nested = write(dir.path(), "Script/sub/b.anm2", b"b");
    write(dir.path(), "Other/c.txt", b"c");
    let scanned = scan_xxh3_128(dir.path());
    assert_eq!(scanned, HashSet::from([top.hex, nested.hex]));
}

#[test]
fn present_at_recorded_path() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "Plugin/a.auo2", b"a");
    let scanned = scan_xxh3_128(dir.path());
    assert!(is_present(
        dir.path(),
        &[check("Plugin/a.auo2", hash)],
        &scanned
    ));
}

#[test]
fn present_at_moved_path() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "Plugin/sub/a.auo2", b"a");
    let scanned = scan_xxh3_128(dir.path());
    assert!(is_present(
        dir.path(),
        &[check("Plugin/a.auo2", hash)],
        &scanned
    ));
}

#[test]
fn absent_when_content_differs() {
    let dir = tempdir().expect("tempdir");
    let recorded = write(dir.path(), "Other/original.bin", b"original");
    write(dir.path(), "Plugin/a.auo2", b"modified");
    let scanned = scan_xxh3_128(dir.path());
    assert!(!is_present(
        dir.path(),
        &[check("Plugin/a.auo2", recorded)],
        &scanned
    ));
}

#[test]
fn absent_when_one_check_fails() {
    let dir = tempdir().expect("tempdir");
    let present = write(dir.path(), "Plugin/a.auo2", b"a");
    let missing = Hash::parse_xxh3_128(&"0".repeat(32)).expect("hash");
    let scanned = scan_xxh3_128(dir.path());
    let checks = [
        check("Plugin/a.auo2", present),
        check("Plugin/b.auo2", missing),
    ];
    assert!(!is_present(dir.path(), &checks, &scanned));
}

#[test]
fn absent_without_checks() {
    let dir = tempdir().expect("tempdir");
    assert!(!is_present(dir.path(), &[], &HashSet::new()));
}

fn data_check(rel: &str, hash: Hash) -> FileCheck {
    FileCheck {
        location: Location {
            root: Root::Data,
            rel: rel.to_owned(),
        },
        hash,
    }
}

#[test]
fn scan_collects_files_below_data() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "data/Plugin/a.aui2", b"a");
    let scanned = scan_xxh3_128(dir.path());
    assert!(scanned.contains(&hash.hex));
}

#[test]
fn data_root_check_holds_at_recorded_path() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "data/Plugin/a.aui2", b"a");
    let checks = [data_check("Plugin/a.aui2", hash)];
    assert!(is_present(dir.path(), &checks, &HashSet::new()));
}

#[test]
fn data_root_check_holds_for_moved_file() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "data/Plugin/sub/a.aui2", b"a");
    let checks = [data_check("Plugin/a.aui2", hash)];
    let scanned = scan_xxh3_128(dir.path());
    assert!(is_present(dir.path(), &checks, &scanned));
}

#[test]
fn check_path_joins_root_and_rel() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "x.txt", b"x");
    let app = check_path(dir.path(), &check("Plugin/a", hash.clone()));
    let data = check_path(dir.path(), &data_check("Plugin/a", hash));
    assert_eq!(app, Some(dir.path().join("Plugin").join("a")));
    assert_eq!(data, Some(dir.path().join("data").join("Plugin").join("a")));
}

fn version(label: &str, checks: Vec<FileCheck>) -> AppVersion {
    AppVersion {
        version: label.to_owned(),
        prerelease: false,
        published_at: None,
        artifacts: Vec::new(),
        checks,
    }
}

#[test]
fn detect_version_returns_the_oldest_version_with_matching_files() {
    let dir = tempdir().expect("tempdir");
    let same = write(dir.path(), "aviutl2.exe", b"exe");
    let other = write(dir.path(), "other.bin", b"other");
    let versions = [
        version("v3", vec![check("aviutl2.exe", other)]),
        version("v2", vec![check("aviutl2.exe", same.clone())]),
        version("v1", vec![check("aviutl2.exe", same)]),
    ];
    let found = detect_version(dir.path(), Kind::Aviutl2, Host::Windows, &versions);
    assert_eq!(found.map(|v| v.version.as_str()), Some("v1"));
}

#[test]
fn detect_version_finds_the_executable_in_a_subdirectory() {
    let dir = tempdir().expect("tempdir");
    let exe = write(dir.path(), "AviUtl2/aviutl2.exe", b"exe");
    let versions = [version("v1", vec![check("aviutl2.exe", exe)])];
    let found = detect_version(dir.path(), Kind::Aviutl2, Host::Windows, &versions);
    assert_eq!(found.map(|v| v.version.as_str()), Some("v1"));
}

#[test]
fn detect_version_is_none_when_no_hash_matches() {
    let dir = tempdir().expect("tempdir");
    write(dir.path(), "aviutl2.exe", b"exe");
    let other = write(dir.path(), "other.bin", b"other");
    let versions = [version("v1", vec![check("aviutl2.exe", other)])];
    assert!(detect_version(dir.path(), Kind::Aviutl2, Host::Windows, &versions).is_none());
}

#[test]
fn detect_version_is_none_for_a_version_without_checks() {
    let dir = tempdir().expect("tempdir");
    write(dir.path(), "aviutl2.exe", b"exe");
    let versions = [version("v1", Vec::new())];
    assert!(detect_version(dir.path(), Kind::Aviutl2, Host::Windows, &versions).is_none());
}

#[test]
fn detect_version_is_none_without_an_executable() {
    let dir = tempdir().expect("tempdir");
    let hash = write(dir.path(), "readme.txt", b"exe");
    let versions = [version("v1", vec![check("readme.txt", hash)])];
    assert!(detect_version(dir.path(), Kind::Aviutl2, Host::Windows, &versions).is_none());
}
