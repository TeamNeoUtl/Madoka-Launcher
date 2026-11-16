#![expect(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] functions may panic"
)]

use launcher_core::{Edition, Host, InstanceStore, Kind, NewInstance, is_instance_running};
use tempfile::TempDir;

fn instance(root: &TempDir) -> launcher_core::Instance {
    InstanceStore::new(root.path())
        .create(
            NewInstance {
                name: "r".to_owned(),
                kind: Kind::Aviutl2,
                edition: Edition::Normal,
                app_version: "1".to_owned(),
                group: None,
            },
            Host::Windows,
        )
        .unwrap()
}

#[test]
fn idle_instance_is_not_running() {
    let root = TempDir::new().unwrap();
    let instance = instance(&root);
    assert!(!is_instance_running(&instance));
    assert!(!instance.is_running());
}

#[cfg(target_os = "linux")]
#[test]
fn process_started_from_instance_dir_counts_as_running() {
    let root = TempDir::new().unwrap();
    let instance = instance(&root);
    let copy = instance.app_dir().join("sleeper");
    std::fs::copy("/bin/sleep", &copy).unwrap();
    let mut child = std::process::Command::new(&copy).arg("30").spawn().unwrap();
    let running = is_instance_running(&instance);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(running);
    assert!(!is_instance_running(&instance));
}
