use std::collections::HashMap;

use launcher_core::{
    AppSelection, Edition, Host, InstallPlan, Location, Origin, Package, PackageSource, Root, Step,
};
use launcher_providers::{
    Aviutl2Provider, AviutlProvider, Http, HttpError, Provider, ProviderError, load_any,
    load_fresh, parse_apm_packages, parse_aviutl_addons, parse_aviutl_apps, parse_aviutl2_app,
    parse_aviutl2_catalog, parse_aviutl2_history, store_catalog,
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

const INDEX: &str =
    "https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json";
const HISTORY2_PAGE: &str = r#"<table>
<tr><td><a href="aviutl2beta9.zip">aviutl2beta9.zip</a></td></tr>
<tr><td><a href="aviutl2beta53a.zip">aviutl2beta53a.zip</a></td></tr>
<tr><td><a href="aviutl2_v2.1.9.zip">aviutl2_v2.1.9.zip</a></td></tr>
<tr><td><a href="aviutl2_v2.1.16.zip">aviutl2_v2.1.16.zip</a></td></tr>
<tr><td><a href="aviutl2_v2.1.7a.zip">aviutl2_v2.1.7a.zip</a></td></tr>
<tr><td><a href="aviutl2_sdk.zip">aviutl2_sdk.zip</a></td></tr>
<tr><td><a href="AviUtl2_v2.1.9_setup.exe">setup</a></td></tr>
<tr><td><a href="aviutl.zip">aviutl110.zip</a></td></tr></table>"#;

fn aviutl2_index() -> Vec<u8> {
    let mut core = entry(
        "Kenkun.AviUtlExEdit2",
        json!({"direct": "https://spring-fragrance.mints.ne.jp/aviutl/AviUtl2_v2.1.16_setup.exe"}),
        json!([]),
    );
    core["latest-version"] = json!("v2.1.16");
    core["version"] = json!([
        {"version": "v2.1.16", "release_date": "2026-02-01",
            "file": [{"path": "{appDir}/aviutl2.exe", "XXH3_128": ZERO_XXH}]},
        {"version": "v2.1.9", "release_date": "2025-12-01",
            "file": [{"path": "{appDir}/aviutl2.exe", "XXH3_128": ZERO_XXH}]}]);
    bytes(&json!([core]))
}

#[test]
fn aviutl2_history_lists_older_zips_newest_first() {
    let versions = parse_aviutl2_history(&aviutl2_index(), HISTORY2_PAGE.as_bytes()).unwrap();
    let labels: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(labels, ["v2.1.9", "v2.1.7a", "beta53a", "beta9"]);
    let listed = &versions[0];
    assert_eq!(listed.artifacts[0].name, "aviutl2_v2.1.9.zip");
    assert_eq!(listed.published_at.as_deref(), Some("2025-12-01"));
    assert_eq!(listed.checks.len(), 1);
    assert!(versions[1].checks.is_empty());
}

#[test]
fn aviutl2_history_rejects_a_page_without_aviutl2_zips() {
    let result = parse_aviutl2_history(&aviutl2_index(), b"<html></html>");
    assert!(matches!(result, Err(ProviderError::CatalogShapeChanged(_))));
}

#[tokio::test]
async fn aviutl2_app_versions_put_the_latest_first_and_survive_a_missing_page() {
    let index = HashMap::from([(INDEX.to_owned(), aviutl2_index())]);
    let provider = Aviutl2Provider::new(FixtureHttp(index.clone()));
    assert_eq!(provider.app_versions().await.unwrap().len(), 1);
    let mut full = index;
    full.insert(HISTORY.to_owned(), HISTORY2_PAGE.as_bytes().to_vec());
    let versions = Aviutl2Provider::new(FixtureHttp(full))
        .app_versions()
        .await
        .unwrap();
    assert_eq!(versions[0].version, "v2.1.16");
    assert_eq!(versions.len(), 5);
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

const CORE: &str = "https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/core.json";
const HISTORY: &str = "https://spring-fragrance.mints.ne.jp/aviutl/oldver2.php";
const HISTORY_PAGE: &str = r#"<table>
<tr><td><a href="aviutl110.zip">aviutl110.zip</a></td><td>version1.10</td></tr>
<tr><td><a href="https://spring-fragrance.mints.ne.jp/aviutl/aviutl99m.zip">aviutl99m.zip</a></td><td>version0.99m</td></tr>
<tr><td><a href='aviutl96g.zip'>aviutl96g.zip</a></td><td>version0.96g</td></tr>
<tr><td><a href="exedit92.zip">exedit92.zip</a></td><td>version0.92</td></tr>
<tr><td><a href="exedit91c.zip">exedit91c.zip</a></td><td>version0.91c</td></tr>
<tr><td><a href="exedit91.zip">exedit91.zip</a></td><td>version0.91</td></tr>
<tr><td><a href="aviutl2beta9.zip">aviutl2beta9.zip</a></td></tr>
<tr><td><a href="aviutl2_v2.1.9.zip">aviutl2_v2.1.9.zip</a></td></tr></table>"#;

fn aviutl_core() -> Value {
    let release = |version: &str, url: Option<&str>| {
        json!({"version": version, "url": url,
            "integrity": {"archive": SHA384_SRI, "file": [{"target": "aviutl.exe", "hash": SHA384_SRI}]}})
    };
    json!({"version": 3,
        "aviutl": {"latestVersion": "1.10", "releases": [
            release("1.10", Some("https://spring-fragrance.mints.ne.jp/aviutl/aviutl110.zip")),
            release("0.99m", None)]},
        "exedit": {"latestVersion": "0.92", "releases": [
            release("0.92", Some("https://spring-fragrance.mints.ne.jp/aviutl/exedit92.zip")),
            release("0.91c", None)]}})
}

fn labels(versions: &[launcher_core::AppVersion]) -> Vec<&str> {
    versions.iter().map(|v| v.version.as_str()).collect()
}

#[test]
fn aviutl_apps_follow_the_history_page() {
    let versions =
        parse_aviutl_apps(Some(&bytes(&aviutl_core())), HISTORY_PAGE.as_bytes()).unwrap();
    assert_eq!(labels(&versions), ["1.10", "0.99m", "0.96g"]);
    assert_eq!(versions[0].artifacts[0].name, "aviutl110.zip");
    assert_eq!(
        versions[0].artifacts[0].url.as_str(),
        "https://spring-fragrance.mints.ne.jp/aviutl/aviutl110.zip"
    );
    assert!(versions[0].artifacts[0].hash.is_some());
    assert_eq!(versions[0].checks.len(), 1);
    let listed = &versions[1];
    assert_eq!(
        listed.artifacts[0].url.as_str(),
        "https://spring-fragrance.mints.ne.jp/aviutl/aviutl99m.zip"
    );
    assert!(
        listed.artifacts[0].hash.is_some(),
        "hash of the core release"
    );
    let unlisted = &versions[2];
    assert!(unlisted.artifacts[0].hash.is_none());
    assert!(unlisted.checks.is_empty());
}

#[test]
fn aviutl_addons_follow_the_history_page() {
    let versions =
        parse_aviutl_addons(Some(&bytes(&aviutl_core())), HISTORY_PAGE.as_bytes()).unwrap();
    assert_eq!(labels(&versions), ["0.92", "0.91c", "0.91"]);
    assert_eq!(versions[0].artifacts[0].name, "exedit92.zip");
    assert!(versions[0].artifacts[0].hash.is_some());
    assert!(versions[1].artifacts[0].hash.is_some());
    assert!(versions[2].artifacts[0].hash.is_none());
    assert_eq!(
        versions[2].artifacts[0].url.as_str(),
        "https://spring-fragrance.mints.ne.jp/aviutl/exedit91.zip"
    );
}

#[test]
fn aviutl_lists_leave_out_versions_that_only_core_lists() {
    let page = concat!(
        r#"<a href="aviutl96g.zip">a</a> version0.96g "#,
        r#"<a href="exedit91.zip">e</a> version0.91"#,
    );
    let core = bytes(&aviutl_core());
    assert_eq!(
        labels(&parse_aviutl_apps(Some(&core), page.as_bytes()).unwrap()),
        ["0.96g"]
    );
    assert_eq!(
        labels(&parse_aviutl_addons(Some(&core), page.as_bytes()).unwrap()),
        ["0.91"]
    );
}

#[test]
fn aviutl_lists_keep_the_first_row_of_a_repeated_version() {
    let page = concat!(
        r#"<a href="aviutl110.zip">a</a> version1.10 "#,
        r#"<a href="aviutl110b.zip">b</a> version1.10"#,
    );
    let versions = parse_aviutl_apps(None, page.as_bytes()).unwrap();
    assert_eq!(labels(&versions), ["1.10"]);
    assert_eq!(versions[0].artifacts[0].name, "aviutl110.zip");
}

#[test]
fn aviutl_lists_carry_no_hash_without_a_usable_core() {
    for core in [None, Some(&b"{}"[..]), Some(&b"not json"[..])] {
        let versions = parse_aviutl_apps(core, HISTORY_PAGE.as_bytes()).unwrap();
        assert_eq!(labels(&versions), ["1.10", "0.99m", "0.96g"]);
        assert!(
            versions
                .iter()
                .all(|v| v.artifacts[0].hash.is_none() && v.checks.is_empty())
        );
        let addons = parse_aviutl_addons(core, HISTORY_PAGE.as_bytes()).unwrap();
        assert_eq!(labels(&addons), ["0.92", "0.91c", "0.91"]);
    }
}

#[test]
fn aviutl_lists_reject_a_page_without_archives() {
    let core = bytes(&aviutl_core());
    let page = b"<html></html>";
    assert!(matches!(
        parse_aviutl_apps(Some(&core), page),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
    assert!(matches!(
        parse_aviutl_addons(Some(&core), page),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
}

#[tokio::test]
async fn aviutl_versions_hold_both_lists() {
    let http = FixtureHttp(HashMap::from([
        (CORE.to_owned(), bytes(&aviutl_core())),
        (HISTORY.to_owned(), HISTORY_PAGE.as_bytes().to_vec()),
    ]));
    let versions = AviutlProvider::new(http).versions().await.unwrap();
    assert_eq!(labels(&versions.app), ["1.10", "0.99m", "0.96g"]);
    assert_eq!(labels(&versions.addon), ["0.92", "0.91c", "0.91"]);
    assert!(versions.app[0].artifacts[0].hash.is_some());
}

#[tokio::test]
async fn aviutl_versions_survive_a_missing_core() {
    let http = FixtureHttp(HashMap::from([(
        HISTORY.to_owned(),
        HISTORY_PAGE.as_bytes().to_vec(),
    )]));
    let versions = AviutlProvider::new(http).versions().await.unwrap();
    assert_eq!(labels(&versions.app), ["1.10", "0.99m", "0.96g"]);
    assert!(versions.app[0].artifacts[0].hash.is_none());
}

#[tokio::test]
async fn aviutl_versions_fail_without_the_history_page() {
    let http = FixtureHttp(HashMap::from([(CORE.to_owned(), bytes(&aviutl_core()))]));
    let provider = AviutlProvider::new(http);
    assert!(matches!(
        provider.versions().await,
        Err(ProviderError::Http(_))
    ));
    assert!(provider.app_versions().await.is_err());
}

#[tokio::test]
async fn kinds_without_an_addon_return_an_empty_addon_list() {
    let http = FixtureHttp(HashMap::from([(INDEX.to_owned(), aviutl2_index())]));
    let versions = Aviutl2Provider::new(http).versions().await.unwrap();
    assert_eq!(versions.app.len(), 1);
    assert!(versions.addon.is_empty());
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
    Package::simple("d/p", "p", url, PackageSource::Recipe(recipe))
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

#[test]
fn aviutl2_catalog_lists_release_dates_newest_first() {
    let mut with_history = entry("history", github_source(), copy_actions());
    with_history["version"] = json!([
        { "version": "v1", "release_date": "2025-12-01", "file": [] },
        { "version": "v3", "release_date": "2026-03-01", "file": [] },
        { "version": "v2", "release_date": "2026-01-01", "file": [] },
        { "version": "v0" }
    ]);
    let index = json!([
        entry(
            "core",
            json!({"direct": "https://x.example/setup.exe"}),
            json!([])
        ),
        with_history,
    ]);
    let packages = parse_aviutl2_catalog(&bytes(&index)).unwrap();
    let package = find(&packages, "history");
    assert_eq!(
        package.release_dates,
        ["2026-03-01", "2026-01-01", "2025-12-01"]
    );
    assert_eq!(package.latest_release(), Some("2026-03-01"));
    assert_eq!(package.first_release(), Some("2025-12-01"));
}

#[test]
fn aviutl2_catalog_builds_a_recipe_for_a_self_extracting_installer() {
    let index = json!([
        entry(
            "core",
            json!({"direct": "https://x.example/setup.exe"}),
            json!([])
        ),
        entry(
            "sfx",
            github_source(),
            json!([
                {"action": "download"},
                {"action": "extractSfx"},
                {"action": "copy", "from": "{tmp}/a.aux2", "to": "{pluginsDir}"},
                {"action": "runAuoSetup", "path": "{appDir}/exe_files/auo_setup2.exe"}
            ])
        ),
    ]);
    let packages = parse_aviutl2_catalog(&bytes(&index)).unwrap();
    let PackageSource::Recipe(recipe) = &find(&packages, "sfx").source else {
        panic!("a recipe is expected");
    };
    assert_eq!(
        recipe.steps,
        [
            Step::ExtractSfx,
            Step::Copy {
                from: Location {
                    root: Root::Tmp,
                    rel: "a.aux2".to_owned()
                },
                to: Location {
                    root: Root::Data,
                    rel: "Plugin".to_owned()
                },
            },
            Step::RunAuoSetup {
                path: Location {
                    root: Root::App,
                    rel: "exe_files/auo_setup2.exe".to_owned()
                },
            },
        ]
    );
}

#[test]
fn aviutl2_catalog_resolves_screenshots_and_keeps_the_deprecation_message() {
    let mut shot = entry("shot", github_source(), copy_actions());
    shot["images"] = json!({"detailImages": ["https://x.example/a.png", "shots/b.png"]});
    shot["deprecation"] = json!({"message": "Use the successor."});
    let index = json!([
        entry(
            "core",
            json!({"direct": "https://x.example/setup.exe"}),
            json!([])
        ),
        shot,
    ]);
    let packages = parse_aviutl2_catalog(&bytes(&index)).unwrap();
    let package = find(&packages, "shot");
    assert_eq!(package.screenshots[0], "https://x.example/a.png");
    assert!(package.screenshots[1].starts_with("https://"));
    assert!(package.screenshots[1].ends_with("/shots/b.png"));
    assert!(package.deprecated);
    assert_eq!(
        package.deprecation_message.as_deref(),
        Some("Use the successor.")
    );
}
