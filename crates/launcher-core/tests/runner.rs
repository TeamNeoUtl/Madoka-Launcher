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

/// One test, so that no other thread forks while a script is open for writing (ETXTBSY).
#[cfg(unix)]
#[test]
fn spawn_sets_the_executable_bit_and_reports_the_exit_status() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    use launcher_core::{Edition, InstanceStore, Launched, NewInstance};

    let dir = tempdir().unwrap();
    let instance = InstanceStore::new(dir.path())
        .create(
            NewInstance {
                name: "n".to_owned(),
                kind: Kind::Neoutl,
                edition: Edition::Normal,
                app_version: "1".to_owned(),
                group: None,
            },
            Host::Linux,
        )
        .unwrap();
    let exe = instance.app_dir().join("NeoUtl");
    let write_script = |script: &str| {
        fs::write(&exe, script).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o644)).unwrap();
    };
    let spawn = || Launched::spawn(&instance, Host::Linux).unwrap();

    write_script("#!/bin/sh\nexit 0\n");
    spawn().confirm(Duration::from_secs(5)).unwrap();
    assert_ne!(fs::metadata(&exe).unwrap().permissions().mode() & 0o111, 0);

    write_script("#!/bin/sh\necho boom >&2\nexit 3\n");
    let error = spawn().confirm(Duration::from_secs(5)).unwrap_err();
    assert!(matches!(
        &error,
        Error::LaunchFailed { status, log, .. } if status.contains('3') && log.contains("boom")
    ));

    write_script("#!/bin/sh\nsleep 2\n");
    spawn().confirm(Duration::from_millis(200)).unwrap();
}
