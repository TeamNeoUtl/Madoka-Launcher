#![expect(
    clippy::assert_is_empty,
    reason = "`assert_eq!` on an empty value needs a type at each call"
)]

use launcher_core::{
    Edition, Error, Host, Instance, InstanceStore, Kind, NewInstance, PluginRecord,
};
use tempfile::TempDir;

fn new_instance(kind: Kind) -> NewInstance {
    NewInstance {
        name: "main".to_owned(),
        kind,
        edition: Edition::Normal,
        bundle: launcher_core::Bundle::Archive,
        app_version: "v4.56.1.0".to_owned(),
        group: Some("work".to_owned()),
    }
}

fn plugin(files: &[&str]) -> PluginRecord {
    PluginRecord {
        id: "owner/repo".to_owned(),
        version: "1.0".to_owned(),
        source: "github:owner/repo".to_owned(),
        files: files.iter().map(|f| (*f).to_owned()).collect(),
        paused: false,
        uninstall_steps: Vec::new(),
    }
}

#[test]
fn create_save_load_delete() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());

    let mut created = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();
    assert!(created.app_dir().is_dir());

    created
        .config
        .plugins
        .push(plugin(&["user/plugin/repo/a.dll"]));
    store.save(&created).unwrap();

    let loaded = store.load(created.config.id).unwrap();
    assert_eq!(loaded, created);

    store.delete(created.config.id).unwrap();
    assert!(!created.dir.exists());
    assert!(matches!(
        store.load(created.config.id),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn toml_layout() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let mut created = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();
    created.config.plugins.push(plugin(&["a.dll"]));
    store.save(&created).unwrap();

    let text = std::fs::read_to_string(created.config_path()).unwrap();
    assert!(text.contains("kind = \"ymm4\""));
    assert!(text.contains("edition = \"normal\""));
    assert!(text.contains("group = \"work\""));
    assert!(text.contains("[[plugin]]"));
}

#[test]
fn list_returns_instances_and_failures() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    assert!(store.list().unwrap().instances.is_empty());

    let a = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();
    let b = store
        .create(new_instance(Kind::Neoutl), Host::Linux)
        .unwrap();
    std::fs::write(b.config_path(), "not = [valid").unwrap();

    let listing = store.list().unwrap();
    assert_eq!(listing.instances, vec![a]);
    assert_eq!(listing.failures.len(), 1);
    assert_eq!(
        listing.failures.first().map(|failure| &failure.dir),
        Some(&b.dir)
    );
}

#[test]
fn unsupported_kind_on_linux() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    for kind in [Kind::Aviutl, Kind::Aviutl2, Kind::Ymm4] {
        assert!(matches!(
            store.create(new_instance(kind), Host::Linux),
            Err(Error::UnsupportedKind { .. })
        ));
    }
    for kind in [Kind::Neoutl, Kind::Freya] {
        store.create(new_instance(kind), Host::Linux).unwrap();
    }
}

#[test]
fn rejects_invalid_values() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());

    let mut empty_name = new_instance(Kind::Ymm4);
    empty_name.name = "  ".to_owned();
    assert!(matches!(
        store.create(empty_name, Host::Windows),
        Err(Error::EmptyName)
    ));

    let mut lite_on_neoutl = new_instance(Kind::Neoutl);
    lite_on_neoutl.edition = Edition::Lite;
    assert!(matches!(
        store.create(lite_on_neoutl, Host::Windows),
        Err(Error::InvalidEdition { .. })
    ));

    let mut app_image_on_neoutl = new_instance(Kind::Neoutl);
    app_image_on_neoutl.bundle = launcher_core::Bundle::AppImage;
    assert!(matches!(
        store.create(app_image_on_neoutl, Host::Windows),
        Err(Error::InvalidBundle { .. })
    ));
}

#[test]
fn save_rejects_unsafe_plugin_paths() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let mut created = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();

    for path in ["../x", "/abs", "a/../../x", "C:/x", "C:\\x", "", "a\\..\\x"] {
        created.config.plugins = vec![plugin(&[path])];
        assert!(
            matches!(store.save(&created), Err(Error::UnsafePath(_))),
            "{path}"
        );
    }
    created.config.plugins = vec![plugin(&["user/plugin/repo/a.dll", "b\\c.dll"])];
    store.save(&created).unwrap();
}

#[test]
fn load_rejects_id_mismatch() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let a = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();
    let b = store
        .create(new_instance(Kind::Ymm4), Host::Windows)
        .unwrap();
    std::fs::copy(b.config_path(), a.config_path()).unwrap();

    assert!(matches!(
        store.load(a.config.id),
        Err(Error::IdMismatch { .. })
    ));
}

#[test]
fn instances_are_independent_directories() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let a = store
        .create(new_instance(Kind::Aviutl2), Host::Windows)
        .unwrap();
    let b = store
        .create(new_instance(Kind::Aviutl2), Host::Windows)
        .unwrap();
    assert_ne!(a.dir, b.dir);
    assert!(!a.dir.starts_with(&b.dir) && !b.dir.starts_with(&a.dir));
    assert_eq!(a.config_path().file_name().unwrap(), Instance::CONFIG_FILE);
}

#[test]
fn combined_version_of_earlier_launchers_splits_on_load() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let mut created = store
        .create(new_instance(Kind::Aviutl), Host::Windows)
        .unwrap();
    created.config.app_version = "1.10 / exedit 0.92".to_owned();
    store.save(&created).unwrap();

    let loaded = store.load(created.config.id).unwrap();
    assert_eq!(loaded.config.app_version, "1.10");
    assert_eq!(loaded.config.addon_version.as_deref(), Some("0.92"));
    assert_eq!(loaded.config.version_label(), "1.10 / exedit 0.92");
}

#[test]
fn set_versions_writes_the_app_and_the_addon_version() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let created = store
        .create(new_instance(Kind::Aviutl), Host::Windows)
        .unwrap();
    let id = created.config.id;

    let updated = store
        .set_versions(id, "1.10".to_owned(), Some("0.92".to_owned()))
        .unwrap();
    assert_eq!(store.load(id).unwrap(), updated);
    let text = std::fs::read_to_string(created.config_path()).unwrap();
    assert!(text.contains("app_version = \"1.10\""));
    assert!(text.contains("addon_version = \"0.92\""));

    store.set_versions(id, "1.10".to_owned(), None).unwrap();
    let text = std::fs::read_to_string(created.config_path()).unwrap();
    assert!(!text.contains("addon_version"));
}

#[test]
fn import_copies_the_install_and_leaves_the_source() {
    let root = TempDir::new().unwrap();
    let source = TempDir::new().unwrap();
    std::fs::create_dir_all(source.path().join("Plugin")).unwrap();
    std::fs::write(source.path().join("aviutl2.exe"), b"exe").unwrap();
    std::fs::write(source.path().join("Plugin/a.auo2"), b"a").unwrap();
    std::fs::write(source.path().join("launch.log"), b"log").unwrap();
    let store = InstanceStore::new(root.path());

    let imported = store
        .import(new_instance(Kind::Aviutl2), source.path(), Host::Windows)
        .unwrap();

    let app = imported.app_dir();
    assert_eq!(std::fs::read(app.join("aviutl2.exe")).unwrap(), b"exe");
    assert_eq!(std::fs::read(app.join("Plugin/a.auo2")).unwrap(), b"a");
    assert!(!app.join("launch.log").exists());
    assert!(source.path().join("aviutl2.exe").is_file());
    assert_eq!(store.load(imported.config.id).unwrap(), imported);
}

#[test]
fn import_rejects_a_folder_without_the_executable() {
    let root = TempDir::new().unwrap();
    let source = TempDir::new().unwrap();
    std::fs::write(source.path().join("readme.txt"), b"x").unwrap();
    let store = InstanceStore::new(root.path());

    let result = store.import(new_instance(Kind::Aviutl2), source.path(), Host::Windows);

    assert!(matches!(result, Err(Error::ExecutableNotFound { .. })));
    assert_eq!(store.list().unwrap().instances.len(), 0);
}

#[test]
fn import_rejects_a_source_that_contains_the_instances_directory() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("aviutl2.exe"), b"exe").unwrap();
    let store = InstanceStore::new(root.path());

    let result = store.import(new_instance(Kind::Aviutl2), root.path(), Host::Windows);

    assert!(matches!(result, Err(Error::OverlappingSource(_))));
}

#[test]
fn import_rejects_a_source_inside_the_instances_directory() {
    let root = TempDir::new().unwrap();
    let store = InstanceStore::new(root.path());
    let existing = store
        .create(new_instance(Kind::Aviutl2), Host::Windows)
        .unwrap();
    std::fs::write(existing.app_dir().join("aviutl2.exe"), b"exe").unwrap();

    let result = store.import(
        new_instance(Kind::Aviutl2),
        &existing.app_dir(),
        Host::Windows,
    );

    assert!(matches!(result, Err(Error::OverlappingSource(_))));
}

#[test]
fn import_rejects_an_unsupported_host() {
    let root = TempDir::new().unwrap();
    let source = TempDir::new().unwrap();
    std::fs::write(source.path().join("aviutl2"), b"exe").unwrap();
    let store = InstanceStore::new(root.path());

    let result = store.import(new_instance(Kind::Aviutl2), source.path(), Host::Linux);

    assert!(matches!(result, Err(Error::UnsupportedKind { .. })));
    assert_eq!(store.list().unwrap().instances.len(), 0);
}

#[cfg(unix)]
#[test]
fn import_keeps_symbolic_links() {
    let root = TempDir::new().unwrap();
    let source = TempDir::new().unwrap();
    std::fs::create_dir_all(source.path().join("lib")).unwrap();
    std::fs::write(source.path().join("freya-nle"), b"exe").unwrap();
    std::fs::write(source.path().join("lib/a.so.1"), b"a").unwrap();
    std::os::unix::fs::symlink("a.so.1", source.path().join("lib/a.so")).unwrap();
    let store = InstanceStore::new(root.path());

    let imported = store
        .import(new_instance(Kind::Freya), source.path(), Host::Linux)
        .unwrap();

    let link = imported.app_dir().join("lib/a.so");
    assert_eq!(std::fs::read_link(link).unwrap().to_str(), Some("a.so.1"));
}
