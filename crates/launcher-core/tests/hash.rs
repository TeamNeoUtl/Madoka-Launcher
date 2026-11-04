use std::collections::HashSet;
use std::fs;
use std::path::Path;

use launcher_core::{FileCheck, Hash, HashAlgo, Location, Root, is_present, scan_xxh3_128};
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
