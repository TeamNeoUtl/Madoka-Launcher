use std::fs;
use std::path::Path;

use launcher_core::{Error, Host, Kind, find_executable};
use tempfile::tempdir;

fn touch(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"").unwrap();
}

#[test]
fn windows_matches_exe_name_case_insensitively_one_level_down() {
    let dir = tempdir().unwrap();
    touch(dir.path(), "YukkuriMovieMaker_v4/yukkurimoviemaker.EXE");
    touch(dir.path(), "YukkuriMovieMaker_v4/readme.txt");
    let found = find_executable(dir.path(), Kind::Ymm4, Host::Windows).unwrap();
    assert_eq!(
        found,
        dir.path()
            .join("YukkuriMovieMaker_v4/yukkurimoviemaker.EXE")
    );
}

#[test]
fn shallowest_level_wins() {
    let dir = tempdir().unwrap();
    touch(dir.path(), "NeoUtl.exe");
    touch(dir.path(), "nested/NeoUtl.exe");
    let found = find_executable(dir.path(), Kind::Neoutl, Host::Windows).unwrap();
    assert_eq!(found, dir.path().join("NeoUtl.exe"));
}

#[test]
fn linux_matches_name_without_extension_only() {
    let dir = tempdir().unwrap();
    touch(dir.path(), "NeoUtl.exe");
    let missing = find_executable(dir.path(), Kind::Neoutl, Host::Linux);
    assert!(matches!(missing, Err(Error::ExecutableNotFound { .. })));

    touch(dir.path(), "NeoUtl");
    let found = find_executable(dir.path(), Kind::Neoutl, Host::Linux).unwrap();
    assert_eq!(found, dir.path().join("NeoUtl"));
}

#[test]
fn depth_two_is_searched_and_depth_three_is_not() {
    let dir = tempdir().unwrap();
    touch(dir.path(), "a/b/c/NeoUtl.exe");
    let missing = find_executable(dir.path(), Kind::Neoutl, Host::Windows);
    assert!(matches!(
        missing,
        Err(Error::ExecutableNotFound {
            kind: Kind::Neoutl,
            ..
        })
    ));

    touch(dir.path(), "a/b/NeoUtl.exe");
    let found = find_executable(dir.path(), Kind::Neoutl, Host::Windows).unwrap();
    assert_eq!(found, dir.path().join("a/b/NeoUtl.exe"));
}

#[test]
fn freya_accepts_each_candidate_stem() {
    for stem in ["freya-nle", "freya", "AviQtl"] {
        let dir = tempdir().unwrap();
        touch(dir.path(), &format!("{stem}.exe"));
        let found = find_executable(dir.path(), Kind::Freya, Host::Windows).unwrap();
        assert_eq!(found, dir.path().join(format!("{stem}.exe")));
    }
}
