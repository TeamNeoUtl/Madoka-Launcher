use launcher_core::{
    Edition, Error, Host, Instance, InstanceStore, Kind, NewInstance, PluginRecord,
};
use tempfile::TempDir;

fn new_instance(kind: Kind) -> NewInstance {
    NewInstance {
        name: "main".to_owned(),
        kind,
        edition: Edition::Normal,
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
    assert_eq!(listing.failures[0].dir, b.dir);
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
