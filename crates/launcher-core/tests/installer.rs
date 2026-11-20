#![expect(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    reason = "helpers outside #[test] functions may panic; `assert_eq!` on empty needs a type"
)]

use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use launcher_core::{
    Artifact, Edition, Error, Fetcher, FileCheck, Hash, HashAlgo, Host, Installer, Instance,
    InstanceStore, Kind, Location, NewInstance, NoProgress, Package, PackageSource, Progress,
    ProgressSink, Root,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use url::Url;
use zip::write::SimpleFileOptions;

#[derive(Default)]
struct FakeFetcher {
    body: Mutex<Vec<u8>>,
    calls: AtomicUsize,
}

impl FakeFetcher {
    fn set(&self, body: Vec<u8>) {
        *self.body.lock().unwrap() = body;
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Fetcher for FakeFetcher {
    fn fetch(
        &self,
        _url: &Url,
        dest: &Path,
        progress: &dyn ProgressSink,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let body = self.body.lock().unwrap().clone();
        let size = body.len() as u64;
        progress.report(Progress {
            done: 0,
            total: Some(size),
        });
        let written = fs::write(dest, &body)
            .map_err(|source| Error::Fetch {
                url: dest.display().to_string(),
                message: source.to_string(),
            })
            .inspect(|()| {
                progress.report(Progress {
                    done: size,
                    total: Some(size),
                });
            });
        std::future::ready(written)
    }
}

struct Fixture {
    root: TempDir,
    installer: Installer<FakeFetcher>,
    instance: Instance,
}

fn fixture(kind: Kind) -> Fixture {
    let root = TempDir::new().unwrap();
    let instance = InstanceStore::new(root.path())
        .create(
            NewInstance {
                name: "main".to_owned(),
                kind,
                edition: Edition::Normal,
                bundle: launcher_core::Bundle::Archive,
                app_version: "v1".to_owned(),
                group: None,
            },
            Host::Windows,
        )
        .unwrap();
    let installer = Installer::new(root.path(), FakeFetcher::default());
    Fixture {
        root,
        installer,
        instance,
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        if name.ends_with('/') {
            writer
                .add_directory(*name, SimpleFileOptions::default())
                .unwrap();
            continue;
        }
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02x}").unwrap();
            hex
        })
}

fn artifact_with_hash(name: &str, host: &str, hex: Option<String>) -> Artifact {
    Artifact {
        name: name.to_owned(),
        url: Url::parse(&format!("https://{host}/o/r/releases/download/v1/{name}")).unwrap(),
        size: None,
        hash: hex.map(|hex| Hash {
            algo: HashAlgo::Sha256,
            hex,
        }),
    }
}

fn artifact(name: &str, bytes: &[u8]) -> Artifact {
    artifact_with_hash(name, "github.com", Some(sha256_hex(bytes)))
}

fn package(repo: &str) -> Package {
    Package::simple(
        format!("github:o/{repo}"),
        repo,
        Url::parse(&format!("https://github.com/o/{repo}")).unwrap(),
        PackageSource::GithubRepo {
            owner: "o".to_owned(),
            repo: repo.to_owned(),
        },
    )
}

fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(all_files(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn reload(f: &Fixture) -> Instance {
    InstanceStore::new(f.root.path())
        .load(f.instance.config.id)
        .unwrap()
}

async fn install_ymme(
    f: &Fixture,
    repo: &str,
    version: &str,
    entries: &[(&str, &[u8])],
) -> Result<launcher_core::PluginRecord, Error> {
    let bytes = zip_bytes(entries);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .install_package(
            f.instance.config.id,
            &package(repo),
            version,
            &artifact(&format!("{repo}.ymme"), &bytes),
            &NoProgress,
        )
        .await
}

#[tokio::test]
async fn install_app_extracts_tree() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[
        ("YukkuriMovieMaker/YukkuriMovieMaker.exe", b"exe"),
        ("YukkuriMovieMaker/lib/x.dll", b"dll"),
    ]);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &NoProgress,
        )
        .await
        .unwrap();

    let app = f.instance.app_dir();
    assert_eq!(
        fs::read(app.join("YukkuriMovieMaker/YukkuriMovieMaker.exe")).unwrap(),
        b"exe"
    );
    assert_eq!(
        fs::read(app.join("YukkuriMovieMaker/lib/x.dll")).unwrap(),
        b"dll"
    );
}

#[tokio::test]
async fn install_app_zip_slip_fails_without_writes() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("ok.txt", b"ok"), ("../../evil.txt", b"evil")]);
    f.installer.fetcher().set(bytes.clone());
    let result = f
        .installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::UnsafePath(_))));
    assert!(all_files(&f.instance.app_dir()).is_empty());
    assert!(!f.root.path().join("evil.txt").exists());
}

#[tokio::test]
async fn install_package_records_all_files() {
    let f = fixture(Kind::Ymm4);
    let record = install_ymme(&f, "repo", "1.0", &[("a.dll", b"a"), ("b.dll", b"b")])
        .await
        .unwrap();

    assert_eq!(
        record.files,
        ["user/plugin/repo/a.dll", "user/plugin/repo/b.dll"]
    );
    assert_eq!(record.source, "github:o/repo");
    let app = f.instance.app_dir();
    for file in &record.files {
        assert!(app.join(file).is_file());
    }
    assert_eq!(reload(&f).config.plugins, [record]);
}

#[tokio::test]
async fn install_package_zip_slip_fails_without_writes() {
    let f = fixture(Kind::Ymm4);
    let result = install_ymme(&f, "repo", "1.0", &[("a.dll", b"a"), ("../evil.dll", b"x")]).await;

    assert!(matches!(result, Err(Error::UnsafePath(_))));
    assert!(all_files(&f.instance.app_dir()).is_empty());
    assert!(!f.instance.app_dir().join("user/plugin/repo").exists());
    assert!(reload(&f).config.plugins.is_empty());
}

#[tokio::test]
async fn install_package_rejects_parent_entries() {
    let f = fixture(Kind::Ymm4);
    for name in [
        "..\\evil.dll",
        "sub/../../evil.dll",
        "/evil.dll",
        "C:\\evil.dll",
    ] {
        let result = install_ymme(&f, "repo", "1.0", &[(name, b"x")]).await;
        assert!(matches!(result, Err(Error::UnsafePath(_))), "{name}");
    }
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

#[tokio::test]
async fn install_package_drops_the_single_root_directory() {
    let f = fixture(Kind::Ymm4);
    let record = install_ymme(
        &f,
        "repo",
        "1.0",
        &[
            ("Plugin/", b""),
            ("Plugin/a.dll", b"a"),
            ("Plugin/README.md", b"r"),
        ],
    )
    .await
    .unwrap();

    assert_eq!(
        record.files,
        ["user/plugin/repo/a.dll", "user/plugin/repo/README.md"]
    );
    let app = f.instance.app_dir();
    assert_eq!(fs::read(app.join("user/plugin/repo/a.dll")).unwrap(), b"a");
    assert!(!app.join("user/plugin/repo/Plugin").exists());
}

#[tokio::test]
async fn install_package_keeps_nested_directories() {
    let f = fixture(Kind::Ymm4);
    let record = install_ymme(
        &f,
        "repo",
        "1.0",
        &[("a.dll", b"a"), ("res/", b""), ("res/b.png", b"b")],
    )
    .await
    .unwrap();

    assert_eq!(
        record.files,
        ["user/plugin/repo/a.dll", "user/plugin/repo/res/b.png"]
    );
    let app = f.instance.app_dir();
    assert_eq!(
        fs::read(app.join("user/plugin/repo/res/b.png")).unwrap(),
        b"b"
    );
}

#[tokio::test]
async fn install_package_rejects_symlink_entry() {
    let f = fixture(Kind::Ymm4);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink("link", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    f.installer.fetcher().set(bytes.clone());
    let result = f
        .installer
        .install_package(
            f.instance.config.id,
            &package("repo"),
            "1.0",
            &artifact("repo.ymme", &bytes),
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::SymlinkEntry(_))));
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

#[tokio::test]
async fn install_package_rejects_unsafe_repo_name() {
    let f = fixture(Kind::Ymm4);
    for repo in ["..", "a\\b"] {
        let result = install_ymme(&f, repo, "1.0", &[("a.dll", b"a")]).await;
        assert!(matches!(result, Err(Error::UnsafePath(_))), "{repo}");
    }
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

#[tokio::test]
async fn install_package_rolls_back_on_write_failure() {
    let f = fixture(Kind::Ymm4);
    let dir = f.instance.app_dir().join("user/plugin/repo");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("b.dll"), b"keep").unwrap();

    let result = install_ymme(
        &f,
        "repo",
        "1.0",
        &[("a.dll", b"a"), ("b.dll", b"b"), ("c.dll", b"c")],
    )
    .await;

    assert!(matches!(result, Err(Error::Io { .. })));
    assert_eq!(all_files(&f.instance.app_dir()), [dir.join("b.dll")]);
    assert_eq!(fs::read(dir.join("b.dll")).unwrap(), b"keep");
    assert!(reload(&f).config.plugins.is_empty());
}

#[tokio::test]
async fn install_package_replaces_previous_version() {
    let f = fixture(Kind::Ymm4);
    install_ymme(&f, "repo", "1.0", &[("a.dll", b"a1"), ("b.dll", b"b1")])
        .await
        .unwrap();
    let record = install_ymme(&f, "repo", "2.0", &[("a.dll", b"a2"), ("c.dll", b"c2")])
        .await
        .unwrap();

    let app = f.instance.app_dir();
    assert_eq!(record.version, "2.0");
    assert!(!app.join("user/plugin/repo/b.dll").exists());
    assert_eq!(fs::read(app.join("user/plugin/repo/a.dll")).unwrap(), b"a2");
    assert_eq!(reload(&f).config.plugins, [record]);
}

#[tokio::test]
async fn remove_package_deletes_files_and_record() {
    let f = fixture(Kind::Ymm4);
    let record = install_ymme(&f, "repo", "1.0", &[("a.dll", b"a"), ("b.dll", b"b")])
        .await
        .unwrap();
    f.installer
        .remove_package(f.instance.config.id, &record.id)
        .unwrap();

    assert!(all_files(&f.instance.app_dir()).is_empty());
    assert!(!f.instance.app_dir().join("user").exists());
    assert!(reload(&f).config.plugins.is_empty());
    assert!(matches!(
        f.installer.remove_package(f.instance.config.id, &record.id),
        Err(Error::PluginNotFound(_))
    ));
}

#[tokio::test]
async fn hash_mismatch_fails_and_leaves_no_download() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("a.dll", b"a")]);
    f.installer.fetcher().set(bytes);
    let result = f
        .installer
        .install_package(
            f.instance.config.id,
            &package("repo"),
            "1.0",
            &artifact("repo.ymme", b"other"),
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::HashMismatch { .. })));
    assert!(all_files(&f.root.path().join("cache")).is_empty());
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

#[tokio::test]
async fn host_outside_allow_list_is_rejected_before_fetch() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("a.dll", b"a")]);
    let plain = artifact_with_hash("repo.ymme", "example.com", Some(sha256_hex(&bytes)));
    let result = f
        .installer
        .install_package(
            f.instance.config.id,
            &package("repo"),
            "1.0",
            &plain,
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::HostNotAllowed(_))));
    assert_eq!(f.installer.fetcher().calls(), 0);
}

#[tokio::test]
async fn cached_download_skips_fetch() {
    let f = fixture(Kind::Ymm4);
    let entries: &[(&str, &[u8])] = &[("a.dll", b"a")];
    install_ymme(&f, "repo", "1.0", entries).await.unwrap();
    install_ymme(&f, "repo", "1.0", entries).await.unwrap();
    assert_eq!(f.installer.fetcher().calls(), 1);
}

#[tokio::test]
async fn download_without_hash_leaves_no_cache_file() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("a.dll", b"a")]);
    f.installer.fetcher().set(bytes);
    f.installer
        .install_package(
            f.instance.config.id,
            &package("repo"),
            "1.0",
            &artifact_with_hash("repo.ymme", "github.com", None),
            &NoProgress,
        )
        .await
        .unwrap();
    assert!(all_files(&f.root.path().join("cache")).is_empty());
}

#[tokio::test]
async fn package_install_is_unsupported_for_other_kinds() {
    let f = fixture(Kind::Neoutl);
    let result = install_ymme(&f, "repo", "1.0", &[("a.dll", b"a")]).await;
    assert!(matches!(
        result,
        Err(Error::PluginUnsupported(Kind::Neoutl))
    ));
}

#[derive(Default)]
struct Recorder(Mutex<Vec<Progress>>);

impl ProgressSink for Recorder {
    fn report(&self, progress: Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

#[tokio::test]
async fn install_app_forwards_download_progress() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("a.txt", b"a")]);
    f.installer.fetcher().set(bytes.clone());
    let recorder = Recorder::default();
    f.installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &recorder,
        )
        .await
        .unwrap();

    let size = bytes.len() as u64;
    assert_eq!(
        *recorder.0.lock().unwrap(),
        [
            Progress {
                done: 0,
                total: Some(size)
            },
            Progress {
                done: size,
                total: Some(size)
            },
        ]
    );
}

async fn install_repo(
    f: &Fixture,
    target: &Artifact,
    progress: &dyn ProgressSink,
) -> Result<launcher_core::PluginRecord, Error> {
    f.installer
        .install_package(
            f.instance.config.id,
            &package("repo"),
            "1.0",
            target,
            progress,
        )
        .await
}

#[tokio::test]
async fn cached_download_reports_completed_progress() {
    let f = fixture(Kind::Ymm4);
    let bytes = zip_bytes(&[("a.dll", b"a")]);
    f.installer.fetcher().set(bytes.clone());
    let target = artifact("repo.ymme", &bytes);
    install_repo(&f, &target, &NoProgress).await.unwrap();

    let recorder = Recorder::default();
    install_repo(&f, &target, &recorder).await.unwrap();

    let size = bytes.len() as u64;
    assert_eq!(
        *recorder.0.lock().unwrap(),
        [Progress {
            done: size,
            total: Some(size)
        }]
    );
    assert_eq!(f.installer.fetcher().calls(), 1);
}

#[cfg(unix)]
fn zip_with_modes(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data, mode) in entries {
        let options = SimpleFileOptions::default().unix_permissions(*mode);
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_keeps_executable_bits_from_the_zip() {
    use std::os::unix::fs::PermissionsExt;

    let f = fixture(Kind::Neoutl);
    let bytes = zip_with_modes(&[
        ("run", b"#!/bin/sh\n", 0o755),
        ("lib/helper", b"bin", 0o700),
        ("data.txt", b"text", 0o644),
        ("secret.txt", b"text", 0o600),
    ]);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &NoProgress,
        )
        .await
        .unwrap();

    let app = f.instance.app_dir();
    let mode = |name: &str| fs::metadata(app.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("run"), 0o755);
    assert_eq!(mode("lib/helper"), 0o755);
    assert_eq!(mode("data.txt"), 0o644);
    assert_eq!(mode("secret.txt"), 0o644);
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_without_zip_modes_leaves_files_non_executable() {
    use std::os::unix::fs::PermissionsExt;

    let f = fixture(Kind::Neoutl);
    let bytes = zip_bytes(&[("NeoUtl", b"bin")]);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &NoProgress,
        )
        .await
        .unwrap();

    let mode = fs::metadata(f.instance.app_dir().join("NeoUtl"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0);
}

fn web_package(page: &str) -> Package {
    let url = Url::parse(page).unwrap();
    Package::simple(url.to_string(), "web", url.clone(), PackageSource::Web(url))
}

fn write_zip(f: &Fixture, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
    let path = f.root.path().join(name);
    fs::write(&path, zip_bytes(entries)).unwrap();
    path
}

#[test]
fn install_package_file_records_a_web_package() {
    let f = fixture(Kind::Ymm4);
    let package = web_package("https://example.com/plugin/1");
    let path = write_zip(
        &f,
        "web.ymme",
        &[("P/", b""), ("P/a.dll", b"a"), ("P/b.dll", b"b")],
    );
    let record = f
        .installer
        .install_package_file(f.instance.config.id, &package, "-", &path)
        .unwrap();

    assert_eq!(record.id, package.id);
    assert_eq!(record.files.len(), 2);
    let app = f.instance.app_dir();
    for file in &record.files {
        assert!(file.starts_with("user/plugin/web-"), "{file}");
        assert!(app.join(file).is_file(), "{file}");
    }
    assert_eq!(reload(&f).config.plugins, [record]);
}

#[test]
fn install_package_file_replaces_the_record_of_the_same_package() {
    let f = fixture(Kind::Ymm4);
    let package = web_package("https://example.com/plugin/1");
    let first = write_zip(&f, "1.ymme", &[("a.dll", b"a1"), ("b.dll", b"b1")]);
    let second = write_zip(&f, "2.ymme", &[("a.dll", b"a2")]);
    let id = f.instance.config.id;
    f.installer
        .install_package_file(id, &package, "-", &first)
        .unwrap();
    let record = f
        .installer
        .install_package_file(id, &package, "-", &second)
        .unwrap();

    assert_eq!(record.files.len(), 1);
    assert_eq!(all_files(&f.instance.app_dir()).len(), 1);
    assert_eq!(reload(&f).config.plugins, [record]);
}

#[test]
fn reinstall_keeps_the_pause_of_the_package() {
    let f = fixture(Kind::Ymm4);
    let package = web_package("https://example.com/plugin/1");
    let path = write_zip(&f, "p.ymme", &[("a.dll", b"a")]);
    let id = f.instance.config.id;
    f.installer
        .install_package_file(id, &package, "-", &path)
        .unwrap();
    InstanceStore::new(f.root.path())
        .set_plugin_paused(id, &package.id, true)
        .unwrap();
    let record = f
        .installer
        .install_package_file(id, &package, "-", &path)
        .unwrap();

    assert!(record.paused);
    assert!(
        reload(&f)
            .config
            .plugins
            .first()
            .is_some_and(|plugin| plugin.paused)
    );
}

#[test]
fn install_package_file_separates_web_packages() {
    let f = fixture(Kind::Ymm4);
    let id = f.instance.config.id;
    let path = write_zip(&f, "p.ymme", &[("a.dll", b"a")]);
    let one = f
        .installer
        .install_package_file(id, &web_package("https://example.com/1"), "-", &path)
        .unwrap();
    let two = f
        .installer
        .install_package_file(id, &web_package("https://example.com/2"), "-", &path)
        .unwrap();

    assert_ne!(one.files, two.files);
    assert_eq!(reload(&f).config.plugins.len(), 2);
}

#[test]
fn install_package_file_rejects_unsupported_kind() {
    let f = fixture(Kind::Aviutl2);
    let path = write_zip(&f, "p.ymme", &[("a.dll", b"a")]);
    let result = f.installer.install_package_file(
        f.instance.config.id,
        &web_package("https://example.com/1"),
        "-",
        &path,
    );

    assert!(matches!(result, Err(Error::PluginUnsupported(_))));
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

fn update_fixture() -> Fixture {
    let f = fixture(Kind::Aviutl);
    let app = f.instance.app_dir();
    fs::write(app.join("aviutl.exe"), b"old exe").unwrap();
    fs::write(app.join("user.ini"), b"mine").unwrap();
    f
}

fn exe_check(bytes: &[u8]) -> FileCheck {
    FileCheck {
        location: Location {
            root: Root::App,
            rel: "aviutl.exe".to_owned(),
        },
        hash: Hash {
            algo: HashAlgo::Sha256,
            hex: sha256_hex(bytes),
        },
    }
}

#[tokio::test]
async fn update_app_replaces_files_and_keeps_the_others() {
    let f = update_fixture();
    let bytes = zip_bytes(&[
        ("aviutl110/aviutl.exe", b"new exe"),
        ("aviutl110/lang/ja.txt", b"ja"),
    ]);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .update_app_checked(
            f.instance.config.id,
            &[artifact("aviutl110.zip", &bytes)],
            &[exe_check(b"new exe")],
            &NoProgress,
        )
        .await
        .unwrap();

    let app = f.instance.app_dir();
    assert_eq!(fs::read(app.join("aviutl.exe")).unwrap(), b"new exe");
    assert_eq!(fs::read(app.join("lang/ja.txt")).unwrap(), b"ja");
    assert_eq!(fs::read(app.join("user.ini")).unwrap(), b"mine");
}

#[tokio::test]
async fn update_app_restores_the_previous_files_when_a_check_fails() {
    let f = update_fixture();
    let bytes = zip_bytes(&[
        ("aviutl110/aviutl.exe", b"new exe"),
        ("aviutl110/lang/ja.txt", b"ja"),
    ]);
    f.installer.fetcher().set(bytes.clone());
    let result = f
        .installer
        .update_app_checked(
            f.instance.config.id,
            &[artifact("aviutl110.zip", &bytes)],
            &[exe_check(b"other")],
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::HashMismatch { .. })));
    let app = f.instance.app_dir();
    assert_eq!(fs::read(app.join("aviutl.exe")).unwrap(), b"old exe");
    assert_eq!(fs::read(app.join("user.ini")).unwrap(), b"mine");
    assert!(!app.join("lang").exists());
}

#[tokio::test]
async fn update_app_rejects_an_archive_before_it_writes() {
    let f = update_fixture();
    let bytes = zip_bytes(&[("aviutl.exe", b"new exe"), ("../evil.txt", b"x")]);
    f.installer.fetcher().set(bytes.clone());
    let result = f
        .installer
        .update_app_checked(
            f.instance.config.id,
            &[artifact("aviutl110.zip", &bytes)],
            &[],
            &NoProgress,
        )
        .await;

    assert!(matches!(result, Err(Error::UnsafePath(_))));
    let app = f.instance.app_dir();
    assert_eq!(fs::read(app.join("aviutl.exe")).unwrap(), b"old exe");
    assert!(!f.instance.dir.join("evil.txt").exists());
}

#[tokio::test]
async fn update_app_follows_the_directory_of_the_installed_executable() {
    let f = fixture(Kind::Ymm4);
    let app = f.instance.app_dir();
    fs::create_dir_all(app.join("YukkuriMovieMaker")).unwrap();
    fs::write(app.join("YukkuriMovieMaker/YukkuriMovieMaker.exe"), b"old").unwrap();
    fs::write(app.join("YukkuriMovieMaker/keep.ini"), b"mine").unwrap();
    let bytes = zip_bytes(&[
        ("v2/YukkuriMovieMaker.exe", b"new"),
        ("v2/lib/x.dll", b"dll"),
    ]);
    f.installer.fetcher().set(bytes.clone());
    f.installer
        .update_app_checked(
            f.instance.config.id,
            &[artifact("app.zip", &bytes)],
            &[],
            &NoProgress,
        )
        .await
        .unwrap();

    let root = app.join("YukkuriMovieMaker");
    assert_eq!(
        fs::read(root.join("YukkuriMovieMaker.exe")).unwrap(),
        b"new"
    );
    assert_eq!(fs::read(root.join("lib/x.dll")).unwrap(), b"dll");
    assert_eq!(fs::read(root.join("keep.ini")).unwrap(), b"mine");
    assert!(!app.join("v2").exists());
}

fn tar_zst_bytes(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for &(name, data, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(mode);
        builder.append_data(&mut header, name, data).unwrap();
    }
    zstd::encode_all(builder.into_inner().unwrap().as_slice(), 0).unwrap()
}

async fn install_freya_app(f: &Fixture, name: &str, bytes: &[u8]) -> Result<(), Error> {
    f.installer.fetcher().set(bytes.to_vec());
    f.installer
        .install_app(f.instance.config.id, &[artifact(name, bytes)], &NoProgress)
        .await
}

#[tokio::test]
async fn install_app_extracts_tar_zst_tree() {
    let f = fixture(Kind::Freya);
    let bytes = tar_zst_bytes(&[("freya-nle", b"bin", 0o755), ("lib/x.so", b"so", 0o644)]);
    install_freya_app(&f, "app.tar.zst", &bytes).await.unwrap();

    let app = f.instance.app_dir();
    assert_eq!(fs::read(app.join("freya-nle")).unwrap(), b"bin");
    assert_eq!(fs::read(app.join("lib/x.so")).unwrap(), b"so");
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_keeps_executable_bits_from_the_tar() {
    use std::os::unix::fs::PermissionsExt;

    let f = fixture(Kind::Freya);
    let bytes = tar_zst_bytes(&[("freya-nle", b"bin", 0o750), ("lib/x.so", b"so", 0o600)]);
    install_freya_app(&f, "app.tar.zst", &bytes).await.unwrap();

    let app = f.instance.app_dir();
    let mode = |name: &str| fs::metadata(app.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("freya-nle"), 0o755);
    assert_eq!(mode("lib/x.so"), 0o644);
}

#[tokio::test]
async fn install_app_rejects_a_symlink_in_the_tar() {
    let f = fixture(Kind::Freya);
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    builder
        .append_link(&mut header, "link", "/etc/passwd")
        .unwrap();
    let bytes = zstd::encode_all(builder.into_inner().unwrap().as_slice(), 0).unwrap();

    let result = install_freya_app(&f, "app.tar.zst", &bytes).await;
    assert!(matches!(result, Err(Error::SymlinkEntry(_))));
    assert!(all_files(&f.instance.app_dir()).is_empty());
}

#[tokio::test]
async fn install_app_stores_an_executable_download_under_the_executable_name() {
    let f = fixture(Kind::Freya);
    let bytes = b"\x7fELF-appimage".to_vec();
    install_freya_app(&f, "Freya-NLE-0.8.1-linux-x86_64.AppImage", &bytes)
        .await
        .unwrap();

    let files = all_files(&f.instance.app_dir());
    assert_eq!(files, [f.instance.app_dir().join("freya-nle")]);
    assert_eq!(fs::read(&files[0]).unwrap(), bytes);
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_makes_the_executable_download_runnable() {
    use std::os::unix::fs::PermissionsExt;

    let f = fixture(Kind::Freya);
    install_freya_app(&f, "app.AppImage", b"\x7fELF-appimage")
        .await
        .unwrap();

    let mode = fs::metadata(f.instance.app_dir().join("freya-nle"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
}

fn app_zip_with_link(link: &str, target: &str) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "X.app/Contents/Frameworks/F.framework/Versions/A/F",
            SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(b"binary").unwrap();
    writer
        .add_symlink(link, target, SimpleFileOptions::default())
        .unwrap();
    writer.finish().unwrap().into_inner()
}

async fn install_app_bytes(f: &Fixture, bytes: &[u8]) -> Result<(), Error> {
    f.installer.fetcher().set(bytes.to_vec());
    f.installer
        .install_app(
            f.instance.config.id,
            &[artifact("app.zip", bytes)],
            &NoProgress,
        )
        .await
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_creates_a_relative_symlink_inside_the_app() {
    let f = fixture(Kind::Freya);
    let link = "X.app/Contents/Frameworks/F.framework/Versions/Current";
    install_app_bytes(&f, &app_zip_with_link(link, "A"))
        .await
        .unwrap();

    let path = f.instance.app_dir().join(link);
    assert_eq!(fs::read_link(&path).unwrap(), Path::new("A"));
    assert_eq!(fs::read(path.join("F")).unwrap(), b"binary");
}

#[tokio::test]
async fn install_app_rejects_a_symlink_that_leaves_the_app() {
    let f = fixture(Kind::Freya);
    let link = "X.app/Contents/Frameworks/escape";
    for target in ["/etc", "../../../..", "A/../../../../.."] {
        let result = install_app_bytes(&f, &app_zip_with_link(link, target)).await;

        assert!(matches!(result, Err(Error::SymlinkEntry(_))), "{target}");
        assert!(all_files(&f.instance.app_dir()).is_empty());
    }
}

#[tokio::test]
async fn install_app_skips_macos_archive_metadata() {
    let f = fixture(Kind::Freya);
    let bytes = zip_bytes(&[
        ("X.app/Contents/MacOS/freya-nle", b"exe"),
        ("X.app/Contents/MacOS/._freya-nle", b"apple double"),
        ("X.app/.DS_Store", b"store"),
        ("__MACOSX/X.app/._Info.plist", b"apple double"),
    ]);
    install_app_bytes(&f, &bytes).await.unwrap();

    let files = all_files(&f.instance.app_dir());
    assert_eq!(
        files,
        [f.instance.app_dir().join("X.app/Contents/MacOS/freya-nle")]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn install_app_accepts_a_mach_o_download_as_a_bare_executable() {
    let f = fixture(Kind::Freya);
    let macho = [0xCF, 0xFA, 0xED, 0xFE, 0, 0, 0, 0];
    install_app_bytes(&f, &macho).await.unwrap();

    let installed = f.instance.app_dir().join("freya-nle");
    assert_eq!(fs::read(&installed).unwrap(), macho);
}
