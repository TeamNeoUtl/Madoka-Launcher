use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Mutex;

use launcher_core::{
    Artifact, Edition, Error, Fetcher, FileCheck, Hash, HashAlgo, Host, Installer, Instance,
    InstanceStore, Kind, Location, NewInstance, NoProgress, Package, PackageSource, ProgressSink,
    Root, StagedPlan, Step,
};
use tempfile::TempDir;
use url::Url;
use zip::write::SimpleFileOptions;

#[derive(Default)]
struct FakeFetcher {
    body: Mutex<Vec<u8>>,
}

impl Fetcher for FakeFetcher {
    async fn fetch(
        &self,
        _url: &Url,
        dest: &Path,
        _progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let body = self.body.lock().unwrap().clone();
        fs::write(dest, body).map_err(|source| Error::Fetch {
            url: dest.display().to_string(),
            message: source.to_string(),
        })
    }
}

struct Fixture {
    _root: TempDir,
    installer: Installer<FakeFetcher>,
    instance: Instance,
}

fn fixture(kind: Kind, exe: &str) -> Fixture {
    let root = TempDir::new().unwrap();
    let instance = InstanceStore::new(root.path())
        .create(
            NewInstance {
                name: "main".to_owned(),
                kind,
                edition: Edition::Normal,
                app_version: "v1".to_owned(),
                group: None,
            },
            Host::Windows,
        )
        .unwrap();
    fs::write(instance.app_dir().join(exe), b"exe").unwrap();
    Fixture {
        installer: Installer::new(root.path(), FakeFetcher::default()),
        _root: root,
        instance,
    }
}

impl Fixture {
    fn serve(&self, entries: &[(&str, &[u8])]) {
        *self.installer.fetcher().body.lock().unwrap() = zip_bytes(entries);
    }

    fn app(&self) -> std::path::PathBuf {
        self.instance.app_dir()
    }

    fn reload(&self) -> Instance {
        InstanceStore::new(self._root.path())
            .load(self.instance.config.id)
            .unwrap()
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn xxh3(bytes: &[u8]) -> Hash {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("f");
    fs::write(&path, bytes).unwrap();
    Hash::of_file(HashAlgo::Xxh3_128, &path).unwrap()
}

fn loc(root: Root, rel: &str) -> Location {
    Location {
        root,
        rel: rel.to_owned(),
    }
}

fn package(id: &str) -> Package {
    let url = Url::parse("https://example.com/").unwrap();
    Package {
        id: id.to_owned(),
        name: id.to_owned(),
        summary: String::new(),
        author: String::new(),
        categories: Vec::new(),
        updated: None,
        page_url: url.clone(),
        source: PackageSource::Web(url),
    }
}

fn plan(steps: Vec<Step>, checks: Vec<FileCheck>) -> StagedPlan {
    StagedPlan {
        artifact: Artifact {
            name: "p.zip".to_owned(),
            url: Url::parse("https://github.com/o/r/releases/download/v1/p.zip").unwrap(),
            size: None,
            hash: None,
        },
        version: "1.0".to_owned(),
        steps,
        checks,
    }
}

fn copy_plugins(from: &str) -> Step {
    Step::Copy {
        from: loc(Root::Tmp, from),
        to: loc(Root::Data, "Plugin"),
    }
}

#[tokio::test]
async fn copy_step_writes_below_data_and_records_every_file() {
    let fx = fixture(Kind::Aviutl2, "aviutl2.exe");
    fx.serve(&[("a.aux2", b"alpha")]);
    let check = FileCheck {
        location: loc(Root::Data, "Plugin/a.aux2"),
        hash: xxh3(b"alpha"),
    };
    let record = fx
        .installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(vec![Step::Extract, copy_plugins("a.aux2")], vec![check]),
            &NoProgress,
        )
        .await
        .unwrap();
    assert_eq!(record.files, ["data/Plugin/a.aux2"]);
    assert!(fx.app().join("data/Plugin/a.aux2").is_file());
    assert_eq!(fx.reload().config.plugins, [record]);
}

#[tokio::test]
async fn hash_mismatch_removes_every_written_file() {
    let fx = fixture(Kind::Aviutl2, "aviutl2.exe");
    fx.serve(&[("a.aux2", b"alpha")]);
    let check = FileCheck {
        location: loc(Root::Data, "Plugin/a.aux2"),
        hash: xxh3(b"other"),
    };
    let result = fx
        .installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(vec![Step::Extract, copy_plugins("a.aux2")], vec![check]),
            &NoProgress,
        )
        .await;
    assert!(matches!(result, Err(Error::HashMismatch { .. })));
    assert!(!fx.app().join("data/Plugin/a.aux2").exists());
    assert!(fx.reload().config.plugins.is_empty());
}

#[tokio::test]
async fn existing_destination_fails_and_stays_untouched() {
    let fx = fixture(Kind::Aviutl2, "aviutl2.exe");
    fs::create_dir_all(fx.app().join("data/Plugin")).unwrap();
    fs::write(fx.app().join("data/Plugin/a.aux2"), b"mine").unwrap();
    fx.serve(&[("a.aux2", b"alpha")]);
    let result = fx
        .installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(vec![Step::Extract, copy_plugins("a.aux2")], Vec::new()),
            &NoProgress,
        )
        .await;
    assert!(matches!(result, Err(Error::AlreadyExists(_))));
    assert_eq!(
        fs::read(fx.app().join("data/Plugin/a.aux2")).unwrap(),
        b"mine"
    );
}

#[tokio::test]
async fn place_uses_archive_dir_and_falls_back_to_search() {
    let fx = fixture(Kind::Aviutl, "aviutl.exe");
    fx.serve(&[("pkg/a.auf", b"a"), ("deep/er/b.auf", b"b")]);
    let steps = vec![
        Step::Extract,
        Step::Place {
            name: "plugins/a.auf".to_owned(),
            archive_dir: Some("pkg/".to_owned()),
            directory: false,
        },
        Step::Place {
            name: "b.auf".to_owned(),
            archive_dir: None,
            directory: false,
        },
    ];
    let record = fx
        .installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(steps, Vec::new()),
            &NoProgress,
        )
        .await
        .unwrap();
    assert_eq!(record.files, ["plugins/a.auf", "b.auf"]);
    assert_eq!(fs::read(fx.app().join("plugins/a.auf")).unwrap(), b"a");
}

#[tokio::test]
async fn place_fails_when_the_archive_lacks_the_entry() {
    let fx = fixture(Kind::Aviutl, "aviutl.exe");
    fx.serve(&[("x.txt", b"x")]);
    let steps = vec![
        Step::Extract,
        Step::Place {
            name: "a.auf".to_owned(),
            archive_dir: None,
            directory: false,
        },
    ];
    let result = fx
        .installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(steps, Vec::new()),
            &NoProgress,
        )
        .await;
    assert!(matches!(result, Err(Error::EntryNotFound(_))));
}

#[tokio::test]
async fn removing_the_last_aviutl2_plugin_keeps_the_data_directory() {
    let fx = fixture(Kind::Aviutl2, "aviutl2.exe");
    fx.serve(&[("a.aux2", b"alpha")]);
    fx.installer
        .install_staged(
            fx.instance.config.id,
            &package("p"),
            &plan(vec![Step::Extract, copy_plugins("a.aux2")], Vec::new()),
            &NoProgress,
        )
        .await
        .unwrap();
    fx.installer
        .remove_package(fx.instance.config.id, "p")
        .unwrap();
    assert!(fx.app().join("data").is_dir());
    assert!(!fx.app().join("data/Plugin").exists());
}

#[tokio::test]
async fn http_download_without_hash_is_refused() {
    let fx = fixture(Kind::Aviutl, "aviutl.exe");
    let mut staged = plan(vec![Step::Extract], Vec::new());
    staged.artifact.url = Url::parse("http://spring-fragrance.mints.ne.jp/a.zip").unwrap();
    let result = fx
        .installer
        .install_staged(fx.instance.config.id, &package("p"), &staged, &NoProgress)
        .await;
    assert!(matches!(result, Err(Error::HashRequired(_))));
}

#[tokio::test]
async fn aviutl_app_archive_drops_its_single_root_directory() {
    let fx = fixture(Kind::Aviutl, "placeholder.txt");
    fs::remove_file(fx.app().join("placeholder.txt")).unwrap();
    fx.serve(&[
        ("aviutl110/aviutl.exe", b"exe"),
        ("aviutl110/aviutl.txt", b"t"),
    ]);
    let artifact = plan(Vec::new(), Vec::new()).artifact;
    fx.installer
        .install_app_checked(fx.instance.config.id, &[artifact], &[], &NoProgress)
        .await
        .unwrap();
    assert!(fx.app().join("aviutl.exe").is_file());
    assert!(!fx.app().join("aviutl110").exists());
}

#[tokio::test]
async fn aviutl2_app_gets_a_data_directory() {
    let fx = fixture(Kind::Aviutl2, "placeholder.txt");
    fs::remove_file(fx.app().join("placeholder.txt")).unwrap();
    fx.serve(&[("aviutl2.exe", b"exe")]);
    let artifact = plan(Vec::new(), Vec::new()).artifact;
    fx.installer
        .install_app_checked(fx.instance.config.id, &[artifact], &[], &NoProgress)
        .await
        .unwrap();
    assert!(fx.app().join("data").is_dir());
}
