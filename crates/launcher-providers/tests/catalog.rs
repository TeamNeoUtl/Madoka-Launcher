use std::collections::HashMap;

use launcher_core::{
    AppSelection, Edition, Host, InstallPlan, Origin, Package, PackageSource, Root, Step,
};
use launcher_providers::{
    Aviutl2Provider, AviutlProvider, Http, HttpError, Provider, ProviderError, load_any,
    load_fresh, parse_apm_packages, parse_aviutl_app, parse_aviutl2_app, parse_aviutl2_catalog,
    store_catalog,
};
use serde_json::{Value, json};
use url::Url;

const ZERO_XXH: &str = "00112233445566778899aabbccddeeff";
const SHA384_SRI: &str = "sha384-YJcXuvmKzJ3nUsGPHaylBdFI9Q4AxAwSDUqpDyZ2wLzYQaU/OE1abw8tMpLf4Jwq";

struct FixtureHttp(HashMap<String, Vec<u8>>);

impl Http for FixtureHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        self.0.get(url.as_str()).cloned().ok_or(HttpError::Status {
            status: 404,
            url: url.to_string(),
        })
    }
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn entry(id: &str, source: Value, install: Value) -> Value {
    json!({
        "id": id, "name": id, "type": "スクリプト", "summary": "s", "author": "a",
        "repoURL": "https://example.com/page", "latest-version": "v1",
        "installer": { "source": source, "install": install, "uninstall": [] },
        "version": [{ "version": "v1", "release_date": "2026-01-01",
            "file": [{ "path": "{pluginsDir}/a.aux2", "XXH3_128": ZERO_XXH }] }]
    })
}

fn copy_actions() -> Value {
    json!([
        {"action": "download"},
        {"action": "extract"},
        {"action": "delete", "path": "{tmp}/x"},
        {"action": "copy", "from": "{tmp}/a.aux2", "to": "{pluginsDir}"}
    ])
}

fn github_source() -> Value {
    json!({"github": {"owner": "o", "repo": "r", "pattern": "^a\\.zip$"}})
}

fn find<'a>(packages: &'a [Package], id: &str) -> &'a Package {
    packages.iter().find(|package| package.id == id).unwrap()
}

#[test]
fn aviutl2_catalog_builds_recipes_and_leaves_the_rest_to_the_browser() {
    let index = json!([
        entry(
            "core",
            json!({"direct": "https://x.example/setup.exe"}),
            json!([])
        ),
        entry("ok", github_source(), copy_actions()),
        entry(
            "booth",
            json!({"booth": "https://b.example/1"}),
            copy_actions()
        ),
        entry(
            "run",
            github_source(),
            json!([{"action": "download"}, {"action": "run", "path": "{download}"}]),
        ),
        entry(
            "tmpless",
            github_source(),
            json!([{"action": "download"}, {"action": "copy", "from": "{unknown}/a", "to": "{pluginsDir}"}]),
        ),
    ]);
    let packages = parse_aviutl2_catalog(&bytes(&index)).unwrap();
    let PackageSource::Recipe(recipe) = &find(&packages, "ok").source else {
        panic!("recipe expected");
    };
    assert_eq!(recipe.version, "v1");
    assert!(matches!(recipe.origin, Origin::GithubRelease { .. }));
    assert_eq!(recipe.steps.len(), 2, "delete is skipped");
    let Step::Copy { from, to } = &recipe.steps[1] else {
        panic!("copy expected");
    };
    assert_eq!((from.root, from.rel.as_str()), (Root::Tmp, "a.aux2"));
    assert_eq!((to.root, to.rel.as_str()), (Root::Data, "Plugin"));
    assert_eq!(recipe.checks.len(), 1);
    for id in ["booth", "run", "tmpless"] {
        assert!(
            matches!(find(&packages, id).source, PackageSource::Web(_)),
            "{id}"
        );
    }
}

#[test]
fn aviutl2_catalog_rejects_an_empty_index() {
    let result = parse_aviutl2_catalog(b"[]");
    assert!(matches!(result, Err(ProviderError::CatalogShapeChanged(_))));
}

#[test]
fn aviutl2_app_points_at_the_portable_zip_beside_the_setup() {
    let mut core = entry(
        "Kenkun.AviUtlExEdit2",
        json!({"direct": "https://spring-fragrance.mints.ne.jp/aviutl/AviUtl2_v1_setup.exe"}),
        json!([]),
    );
    core["version"][0]["file"][0]["path"] = json!("{appDir}/aviutl2.exe");
    let version = parse_aviutl2_app(&bytes(&json!([core]))).unwrap();
    let artifact = &version.artifacts[0];
    assert_eq!(artifact.name, "aviutl2_v1.zip");
    assert_eq!(
        artifact.url.as_str(),
        "https://spring-fragrance.mints.ne.jp/aviutl/aviutl2_v1.zip"
    );
    assert_eq!(version.checks.len(), 1);
    assert_eq!(version.checks[0].location.rel, "aviutl2.exe");
}

fn apm_package(id: &str, extra: Value) -> Value {
    let mut package = json!({
        "id": id, "name": id, "overview": "o", "developer": "d",
        "pageURL": "https://example.com/p", "downloadURLs": ["https://example.com/d"],
        "latestVersion": "1.0",
        "files": [
            {"filename": "a.auf"},
            {"filename": "dir", "isDirectory": true, "archivePath": "pkg/"},
            {"filename": "old.auf", "isObsolete": true},
            {"filename": "keep.ini", "isUninstallOnly": true}
        ],
        "releases": [{"version": "1.0", "integrity": {
            "archive": SHA384_SRI, "file": [{"target": "a.auf", "hash": SHA384_SRI}]}}]
    });
    for (key, value) in extra.as_object().unwrap() {
        package[key] = value.clone();
    }
    package
}

#[test]
fn apm_packages_map_files_to_place_steps() {
    let data = json!({"version": 3, "packages": [
        apm_package("d/direct", json!({"directURL": "https://spring-fragrance.mints.ne.jp/a.zip"})),
        apm_package("d/github", json!({"downloadURLs": ["https://github.com/o/r/releases"]})),
        apm_package("d/web", json!({})),
        apm_package("d/hidden", json!({"isHidden": true})),
    ]});
    let packages = parse_apm_packages(&bytes(&data)).unwrap();
    assert_eq!(packages.len(), 3);
    let PackageSource::Recipe(recipe) = &find(&packages, "d/github").source else {
        panic!("recipe expected");
    };
    assert_eq!(recipe.steps.len(), 3, "extract + two placed files");
    assert_eq!(
        recipe.steps[2],
        Step::Place {
            name: "dir".to_owned(),
            archive_dir: Some("pkg/".to_owned()),
            directory: true
        }
    );
    assert!(recipe.hash.is_some());
    assert_eq!(recipe.checks[0].location.root, Root::App);
    assert!(matches!(
        &find(&packages, "d/direct").source,
        PackageSource::Recipe(r) if matches!(r.origin, Origin::Direct(_))
    ));
    assert!(matches!(
        find(&packages, "d/web").source,
        PackageSource::Web(_)
    ));
}

#[test]
fn aviutl_app_joins_both_archives_and_their_hashes() {
    let program = |file: &str, url: &str| {
        json!({"latestVersion": "1", "releases": [{"version": "1", "url": url,
            "integrity": {"archive": SHA384_SRI, "file": [{"target": file, "hash": SHA384_SRI}]}}]})
    };
    let core = json!({"version": 3,
        "aviutl": program("aviutl.exe", "http://spring-fragrance.mints.ne.jp/aviutl/aviutl110.zip"),
        "exedit": program("exedit.auf", "http://spring-fragrance.mints.ne.jp/aviutl/exedit.zip")});
    let version = parse_aviutl_app(&bytes(&core)).unwrap();
    assert_eq!(version.artifacts.len(), 2);
    assert!(version.artifacts.iter().all(|a| a.hash.is_some()));
    assert_eq!(version.checks.len(), 2);
}

fn recipe_package(origin: Origin, hash: bool) -> Package {
    let url = Url::parse("https://example.com/page").unwrap();
    let recipe = launcher_core::Recipe {
        version: "1".to_owned(),
        origin,
        hash: hash
            .then(|| launcher_core::Hash::parse_sri(SHA384_SRI))
            .flatten(),
        steps: vec![Step::Extract],
        checks: Vec::new(),
    };
    Package {
        id: "d/p".to_owned(),
        name: "p".to_owned(),
        summary: String::new(),
        author: String::new(),
        categories: Vec::new(),
        updated: None,
        page_url: url,
        source: PackageSource::Recipe(recipe),
    }
}

fn release_json(assets: &[&str]) -> Vec<u8> {
    let assets: Vec<Value> = assets
        .iter()
        .map(|name| {
            json!({"name": name, "size": 1,
                "browser_download_url": format!("https://github.com/o/r/releases/download/t/{name}")})
        })
        .collect();
    bytes(&json!({"tag_name": "t", "assets": assets}))
}

const LATEST: &str = "https://api.github.com/repos/o/r/releases/latest";
const PATTERNS: &str =
    "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/util/archive-name-pattern.json";

fn github_origin() -> Origin {
    Origin::GithubRelease {
        owner: "o".to_owned(),
        repo: "r".to_owned(),
        pattern: None,
    }
}

#[tokio::test]
async fn apm_single_zip_asset_is_planned_with_the_index_hash() {
    let http = FixtureHttp(HashMap::from([(
        LATEST.to_owned(),
        release_json(&["a.zip", "a.txt"]),
    )]));
    let plan = AviutlProvider::new(http)
        .plan_install_package(&recipe_package(github_origin(), true))
        .await
        .unwrap();
    let InstallPlan::Staged(staged) = plan else {
        panic!("staged plan expected");
    };
    assert_eq!(staged.artifact.name, "a.zip");
    assert!(staged.artifact.hash.is_some());
}

#[tokio::test]
async fn apm_several_zips_use_the_listed_pattern() {
    let http = FixtureHttp(HashMap::from([
        (LATEST.to_owned(), release_json(&["a_1.0.zip", "b_1.0.zip"])),
        (PATTERNS.to_owned(), bytes(&json!({"d/p": "b_[0-9.]+"}))),
    ]));
    let plan = AviutlProvider::new(http)
        .plan_install_package(&recipe_package(github_origin(), true))
        .await
        .unwrap();
    let InstallPlan::Staged(staged) = plan else {
        panic!("staged plan expected");
    };
    assert_eq!(staged.artifact.name, "b_1.0.zip");
}

#[tokio::test]
async fn apm_several_zips_without_a_pattern_open_the_browser() {
    let http = FixtureHttp(HashMap::from([
        (LATEST.to_owned(), release_json(&["a.zip", "b.zip"])),
        (PATTERNS.to_owned(), bytes(&json!({}))),
    ]));
    let plan = AviutlProvider::new(http)
        .plan_install_package(&recipe_package(github_origin(), true))
        .await
        .unwrap();
    assert!(matches!(plan, InstallPlan::OpenInBrowser(_)));
}

#[tokio::test]
async fn direct_downloads_from_unlisted_hosts_or_plain_http_open_the_browser() {
    let provider = Aviutl2Provider::new(FixtureHttp(HashMap::new()));
    for (url, hash) in [
        ("https://unlisted.example/a.zip", true),
        ("http://spring-fragrance.mints.ne.jp/a.zip", false),
    ] {
        let package = recipe_package(Origin::Direct(Url::parse(url).unwrap()), hash);
        let plan = provider.plan_install_package(&package).await.unwrap();
        assert!(matches!(plan, InstallPlan::OpenInBrowser(_)), "{url}");
    }
    let allowed = recipe_package(
        Origin::Direct(Url::parse("http://spring-fragrance.mints.ne.jp/a.zip").unwrap()),
        true,
    );
    let plan = provider.plan_install_package(&allowed).await.unwrap();
    assert!(matches!(plan, InstallPlan::Staged(_)));
}

#[test]
fn aviutl_kinds_install_on_windows_only() {
    let version = parse_aviutl2_app(&bytes(&json!([entry(
        "Kenkun.AviUtlExEdit2",
        json!({"direct": "https://spring-fragrance.mints.ne.jp/aviutl/s.exe"}),
        json!([])
    )])))
    .unwrap();
    let provider = Aviutl2Provider::new(FixtureHttp(HashMap::new()));
    let selection = |host| AppSelection {
        host,
        edition: Edition::Normal,
    };
    assert!(
        provider
            .plan_install_app(&version, selection(Host::Windows))
            .is_ok()
    );
    assert!(matches!(
        provider.plan_install_app(&version, selection(Host::Linux)),
        Err(ProviderError::UnsupportedHost { .. })
    ));
}

#[test]
fn catalog_cache_round_trips_and_ignores_broken_files() {
    let root = tempfile::tempdir().unwrap();
    let packages = vec![recipe_package(github_origin(), true)];
    assert!(load_any(root.path(), launcher_core::Kind::Aviutl).is_none());
    store_catalog(root.path(), launcher_core::Kind::Aviutl, &packages).unwrap();
    assert_eq!(
        load_fresh(root.path(), launcher_core::Kind::Aviutl),
        Some(packages)
    );
    let path = root.path().join("cache/catalog/aviutl.json");
    std::fs::write(path, b"{").unwrap();
    assert!(load_any(root.path(), launcher_core::Kind::Aviutl).is_none());
}
