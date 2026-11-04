use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use launcher_core::{AppSelection, Edition, Host, InstallPlan, Kind, Package, PackageSource};
use launcher_providers::{
    FreyaProvider, Http, HttpError, NeoutlProvider, Provider, ProviderError, Ymm4Provider,
    parse_ymm4_catalog,
};
use serde_json::{Value, json};
use url::Url;

const YMM4_RELEASES_URL: &str =
    "https://api.github.com/repos/manju-summoner/YukkuriMovieMaker4/releases?per_page=30";
const NEOUTL_RELEASES_URL: &str =
    "https://codeberg.org/api/v1/repos/TeamNeoUtl/NeoUtl/releases?limit=50";
const FREYA_RELEASES_URL: &str =
    "https://api.github.com/repos/GT-610/freya-nle/releases?per_page=30";
const YMM4_INDEX_URL: &str = "https://manjubox.net/ymm4plugins.yml";

const WINDOWS: AppSelection = AppSelection {
    host: Host::Windows,
    edition: Edition::Normal,
};
const WINDOWS_LITE: AppSelection = AppSelection {
    host: Host::Windows,
    edition: Edition::Lite,
};
const LINUX: AppSelection = AppSelection {
    host: Host::Linux,
    edition: Edition::Normal,
};

enum Reply {
    Body(Vec<u8>),
    Status(u16),
}

/// Answers from a fixed table. Unlisted URLs answer 404. Records every requested URL.
struct FixtureHttp {
    replies: HashMap<String, Reply>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureHttp {
    fn new<S: Into<String>>(replies: Vec<(S, Reply)>) -> Self {
        Self {
            replies: replies
                .into_iter()
                .map(|(url, reply)| (url.into(), reply))
                .collect(),
            requests: Arc::default(),
        }
    }

    fn request_log(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.requests)
    }
}

impl Http for FixtureHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        self.requests.lock().unwrap().push(url.to_string());
        match self.replies.get(url.as_str()) {
            Some(Reply::Body(bytes)) => Ok(bytes.clone()),
            Some(Reply::Status(status)) => Err(HttpError::Status {
                status: *status,
                url: url.to_string(),
            }),
            None => Err(HttpError::Status {
                status: 404,
                url: url.to_string(),
            }),
        }
    }
}

fn asset(name: &str) -> Value {
    json!({
        "name": name,
        "size": 1,
        "browser_download_url": format!("https://github.com/o/r/releases/download/t/{name}"),
    })
}

fn asset_with_digest(name: &str, digest: &str) -> Value {
    let mut value = asset(name);
    value["digest"] = json!(digest);
    value
}

fn release(tag: &str, assets: Vec<Value>) -> Value {
    json!({
        "tag_name": tag,
        "draft": false,
        "prerelease": false,
        "published_at": "2026-01-01T00:00:00Z",
        "assets": assets,
    })
}

fn body(value: &Value) -> Reply {
    Reply::Body(serde_json::to_vec(value).unwrap())
}

fn releases_body(releases: Vec<Value>) -> Reply {
    body(&Value::Array(releases))
}

fn ymm4(releases: Vec<Value>) -> Ymm4Provider<FixtureHttp> {
    Ymm4Provider::new(FixtureHttp::new(vec![(
        YMM4_RELEASES_URL,
        releases_body(releases),
    )]))
}

fn neoutl(releases: Vec<Value>) -> NeoutlProvider<FixtureHttp> {
    NeoutlProvider::new(FixtureHttp::new(vec![(
        NEOUTL_RELEASES_URL,
        releases_body(releases),
    )]))
}

fn freya(releases: Vec<Value>) -> FreyaProvider<FixtureHttp> {
    FreyaProvider::new(FixtureHttp::new(vec![(
        FREYA_RELEASES_URL,
        releases_body(releases),
    )]))
}

fn names(artifacts: &[launcher_core::Artifact]) -> Vec<&str> {
    artifacts
        .iter()
        .map(|artifact| artifact.name.as_str())
        .collect()
}

fn index_entry(types: &str, name: &str, url: &str, extra: &str) -> String {
    format!(
        "- {{type: '{types}', date: '2026-01-01', name: '{name}', url: '{url}', \
         author: 'a', authorUrl: 'https://a.example/', description: 'd'{extra}}}\n"
    )
}

fn github_package(owner: &str, repo: &str) -> Package {
    Package::simple(
        format!("github:{owner}/{repo}"),
        "p",
        Url::parse(&format!("https://github.com/{owner}/{repo}")).unwrap(),
        PackageSource::GithubRepo {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        },
    )
}

fn latest_url(repo: &str) -> String {
    format!("https://api.github.com/repos/o/{repo}/releases/latest")
}

fn plugin_provider(replies: Vec<(String, Reply)>) -> Ymm4Provider<FixtureHttp> {
    Ymm4Provider::new(FixtureHttp::new(replies))
}

#[test]
fn catalog_keeps_fields_and_order_and_splits_categories() {
    let yaml = [
        index_entry(
            "映像エフェクト, ツール,,その他",
            "first",
            "https://x.example/1",
            "",
        ),
        index_entry("図形", "second", "https://x.example/2", ""),
    ]
    .concat();
    let packages = parse_ymm4_catalog(yaml.as_bytes()).unwrap();

    let order: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(order, ["first", "second"]);
    assert_eq!(
        packages[0].categories,
        ["映像エフェクト", "ツール", "その他"]
    );
    assert_eq!(packages[1].categories, ["図形"]);
    assert_eq!(packages[0].author, "a");
    assert_eq!(packages[0].summary, "d");
    assert_eq!(packages[0].updated.as_deref(), Some("2026-01-01"));
}

#[test]
fn catalog_hides_disabled_entries_and_enables_by_default() {
    let yaml = [
        index_entry("a", "shown", "https://x.example/1", ""),
        index_entry("a", "explicit", "https://x.example/2", ", isEnabled: true"),
        index_entry("a", "hidden", "https://x.example/3", ", isEnabled: false"),
    ]
    .concat();
    let packages = parse_ymm4_catalog(yaml.as_bytes()).unwrap();
    let shown: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(shown, ["shown", "explicit"]);
}

#[test]
fn catalog_classifies_source_by_url_shape() {
    let cases = [
        ("https://github.com/o/r", Some(("o", "r"))),
        ("https://github.com/o/r/", Some(("o", "r"))),
        ("https://github.com/o/r/tree/main", None),
        ("https://github.com/o", None),
        ("https://www.github.com/o/r", None),
        ("https://example.booth.pm/items/1", None),
    ];
    let yaml: String = cases
        .iter()
        .map(|(url, _)| index_entry("a", "n", url, ""))
        .collect();
    let packages = parse_ymm4_catalog(yaml.as_bytes()).unwrap();

    assert_eq!(packages.len(), cases.len());
    for (package, (url, repo)) in packages.iter().zip(cases) {
        match repo {
            Some((owner, repo)) => {
                assert_eq!(
                    package.source,
                    PackageSource::GithubRepo {
                        owner: owner.to_owned(),
                        repo: repo.to_owned()
                    },
                    "{url}"
                );
                assert_eq!(package.id, format!("github:{owner}/{repo}"), "{url}");
            }
            None => {
                assert_eq!(
                    package.source,
                    PackageSource::Web(Url::parse(url).unwrap()),
                    "{url}"
                );
                assert_eq!(package.id, url, "{url}");
            }
        }
        assert_eq!(package.page_url.as_str(), url);
    }
}

#[test]
fn catalog_detects_shape_change() {
    let missing_key = "- type: a\n  name: b\n";
    let all_disabled = index_entry("a", "n", "https://x.example/", ", isEnabled: false");
    let invalid_url = index_entry("a", "n", "not a url", "");
    let inputs: [(&str, &[u8]); 5] = [
        ("empty list", b"[]"),
        ("missing keys", missing_key.as_bytes()),
        ("html", b"<html></html>"),
        ("all disabled", all_disabled.as_bytes()),
        ("invalid url", invalid_url.as_bytes()),
    ];
    for (label, input) in inputs {
        assert!(
            matches!(
                parse_ymm4_catalog(input),
                Err(ProviderError::CatalogShapeChanged(_))
            ),
            "{label}"
        );
    }
}

#[tokio::test]
async fn catalog_is_fetched_from_the_index_url() {
    let yaml = index_entry("a", "n", "https://x.example/1", "");
    let provider = Ymm4Provider::new(FixtureHttp::new(vec![(
        YMM4_INDEX_URL,
        Reply::Body(yaml.into_bytes()),
    )]));
    assert_eq!(provider.catalog().await.unwrap().len(), 1);
}

#[tokio::test]
async fn drafts_and_releases_without_accepted_assets_are_dropped() {
    let mut draft = release("v2", vec![asset("YukkuriMovieMaker_v2.zip")]);
    draft["draft"] = json!(true);
    let provider = ymm4(vec![
        release("v3", vec![asset("YukkuriMovieMaker_v3.zip")]),
        draft,
        release("v1", vec![asset("source.zip")]),
    ]);
    let versions = provider.app_versions().await.unwrap();
    let tags: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(tags, ["v3"]);
}

#[tokio::test]
async fn order_prerelease_flag_and_date_are_preserved() {
    let mut pre = release("v2-rc", vec![asset("AviQtl-Arch-Linux-x86_64.zip")]);
    pre["prerelease"] = json!(true);
    let stable = release("v1", vec![asset("AviQtl-Arch-Linux-x86_64.zip")]);
    let versions = freya(vec![pre, stable]).app_versions().await.unwrap();

    let tags: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(tags, ["v2-rc", "v1"]);
    assert!(versions[0].prerelease);
    assert!(!versions[1].prerelease);
    assert_eq!(
        versions[0].published_at.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
}

#[tokio::test]
async fn ymm4_asset_names_are_filtered_by_pattern() {
    let provider = ymm4(vec![release(
        "v4",
        vec![
            asset("source.zip"),
            asset("YukkuriMovieMaker_v4.1.0_Beta.zip"),
            asset("YukkuriMovieMaker_v4.1.0.zip.sha256"),
            asset("YukkuriMovieMaker_v4.1.0.zip"),
            asset("YukkuriMovieMaker_v4.1.0_Lite.zip"),
        ],
    )]);
    let versions = provider.app_versions().await.unwrap();
    assert_eq!(
        names(&versions[0].artifacts),
        [
            "YukkuriMovieMaker_v4.1.0.zip",
            "YukkuriMovieMaker_v4.1.0_Lite.zip"
        ]
    );
}

#[tokio::test]
async fn ymm4_edition_selects_matching_artifact() {
    let both = ymm4(vec![release(
        "v4",
        vec![
            asset("YukkuriMovieMaker_v4.zip"),
            asset("YukkuriMovieMaker_v4_Lite.zip"),
        ],
    )]);
    let version = both.app_versions().await.unwrap().remove(0);
    let normal = both.plan_install_app(&version, WINDOWS).unwrap();
    let lite = both.plan_install_app(&version, WINDOWS_LITE).unwrap();
    assert_eq!(names(&normal), ["YukkuriMovieMaker_v4.zip"]);
    assert_eq!(names(&lite), ["YukkuriMovieMaker_v4_Lite.zip"]);

    let normal_only = ymm4(vec![release("v5", vec![asset("YukkuriMovieMaker_v5.zip")])]);
    let version = normal_only.app_versions().await.unwrap().remove(0);
    assert!(matches!(
        normal_only.plan_install_app(&version, WINDOWS_LITE),
        Err(ProviderError::NoMatchingAsset { .. })
    ));
}

#[tokio::test]
async fn ymm4_is_unsupported_on_linux() {
    let provider = ymm4(vec![release("v4", vec![asset("YukkuriMovieMaker_v4.zip")])]);
    let version = provider.app_versions().await.unwrap().remove(0);
    assert!(matches!(
        provider.plan_install_app(&version, LINUX),
        Err(ProviderError::UnsupportedHost {
            kind: Kind::Ymm4,
            host: Host::Linux
        })
    ));
}

#[tokio::test]
async fn digest_becomes_sha256_hash_only_when_well_formed() {
    let valid = "ab".repeat(32);
    let upper = "AB".repeat(32);
    let provider = ymm4(vec![release(
        "v4",
        vec![
            asset_with_digest("YukkuriMovieMaker_v1.zip", &format!("sha256:{valid}")),
            asset_with_digest("YukkuriMovieMaker_v2.zip", &format!("sha256:{upper}")),
            asset_with_digest("YukkuriMovieMaker_v3.zip", "sha256:abc"),
            asset_with_digest("YukkuriMovieMaker_v4.zip", &format!("md5:{valid}")),
            asset("YukkuriMovieMaker_v5.zip"),
        ],
    )]);
    let versions = provider.app_versions().await.unwrap();
    let hashes: Vec<Option<&str>> = versions[0]
        .artifacts
        .iter()
        .map(|artifact| artifact.hash.as_ref().map(|hash| hash.hex.as_str()))
        .collect();
    assert_eq!(
        hashes,
        [Some(valid.as_str()), Some(valid.as_str()), None, None, None]
    );
}

#[tokio::test]
async fn neoutl_selects_by_host_and_ignores_other_names() {
    let provider = neoutl(vec![
        release(
            "1.0",
            vec![
                asset("neoutl-linux-x86_64.zip"),
                asset("neoutl-windows-x86_64.zip"),
                asset("neoutl-macos-arm64.zip"),
            ],
        ),
        release("0.1-Unstable", vec![asset("AviQtl-Linux-x86_64.zip")]),
        release("0.9", vec![asset("neoutl-windows-x86_64.zip")]),
    ]);
    let versions = provider.app_versions().await.unwrap();
    let tags: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    assert_eq!(tags, ["1.0", "0.9"]);

    let windows = provider.plan_install_app(&versions[0], WINDOWS).unwrap();
    let linux = provider.plan_install_app(&versions[0], LINUX).unwrap();
    assert_eq!(names(&windows), ["neoutl-windows-x86_64.zip"]);
    assert_eq!(names(&linux), ["neoutl-linux-x86_64.zip"]);
    assert!(matches!(
        provider.plan_install_app(&versions[1], LINUX),
        Err(ProviderError::NoMatchingAsset { .. })
    ));
    assert!(provider.catalog().await.unwrap().is_empty());
}

#[tokio::test]
async fn freya_selects_by_host_and_ignores_other_names() {
    let provider = freya(vec![
        release(
            "v2",
            vec![
                asset("AviQtl-MSVC-x86_64.zip"),
                asset("AviQtl-Arch-Linux-x86_64.zip"),
                asset("AviQtl-macOS-ARM64.zip"),
            ],
        ),
        release("v1", vec![asset("AviQtl-MSYS2-UCRT64-x86_64.zip")]),
    ]);
    let versions = provider.app_versions().await.unwrap();
    assert_eq!(
        names(&versions[0].artifacts),
        ["AviQtl-MSVC-x86_64.zip", "AviQtl-Arch-Linux-x86_64.zip"]
    );

    let windows = provider.plan_install_app(&versions[0], WINDOWS).unwrap();
    let linux = provider.plan_install_app(&versions[0], LINUX).unwrap();
    let windows_msys2 = provider.plan_install_app(&versions[1], WINDOWS).unwrap();
    assert_eq!(names(&windows), ["AviQtl-MSVC-x86_64.zip"]);
    assert_eq!(names(&linux), ["AviQtl-Arch-Linux-x86_64.zip"]);
    assert_eq!(names(&windows_msys2), ["AviQtl-MSYS2-UCRT64-x86_64.zip"]);
    assert!(matches!(
        provider.plan_install_app(&versions[1], LINUX),
        Err(ProviderError::NoMatchingAsset { .. })
    ));
    assert!(provider.catalog().await.unwrap().is_empty());
}

#[tokio::test]
async fn malformed_release_body_is_a_json_error() {
    let provider = Ymm4Provider::new(FixtureHttp::new(vec![(
        YMM4_RELEASES_URL,
        Reply::Body(b"<html></html>".to_vec()),
    )]));
    assert!(matches!(
        provider.app_versions().await,
        Err(ProviderError::Json { .. })
    ));
}

#[tokio::test]
async fn http_failure_is_propagated() {
    let provider = Ymm4Provider::new(FixtureHttp::new(vec![(
        YMM4_RELEASES_URL,
        Reply::Status(500),
    )]));
    assert!(matches!(
        provider.app_versions().await,
        Err(ProviderError::Http(HttpError::Status { status: 500, .. }))
    ));
}

#[tokio::test]
async fn package_with_one_ymme_asset_is_downloaded() {
    let digest = format!("sha256:{}", "ab".repeat(32));
    let provider = plugin_provider(vec![
        (
            latest_url("one"),
            body(&release(
                "v1",
                vec![
                    asset_with_digest("Plugin.ymme", &digest),
                    asset("source.zip"),
                ],
            )),
        ),
        (
            latest_url("upper"),
            body(&release("v1", vec![asset("Plugin.YMME")])),
        ),
    ]);

    for (repo, expected) in [("one", "Plugin.ymme"), ("upper", "Plugin.YMME")] {
        let plan = provider
            .plan_install_package(&github_package("o", repo))
            .await
            .unwrap();
        let InstallPlan::Download(artifacts) = plan else {
            panic!("{repo}: expected download");
        };
        assert_eq!(names(&artifacts), [expected], "{repo}");
    }
    let plan = provider
        .plan_install_package(&github_package("o", "one"))
        .await
        .unwrap();
    let InstallPlan::Download(artifacts) = plan else {
        panic!("expected download");
    };
    assert!(artifacts[0].hash.is_some());
}

#[tokio::test]
async fn package_without_a_single_ymme_asset_opens_in_browser() {
    let provider = plugin_provider(vec![
        (
            latest_url("two"),
            body(&release("v1", vec![asset("A.ymme"), asset("B.ymme")])),
        ),
        (
            latest_url("none"),
            body(&release("v1", vec![asset("source.zip")])),
        ),
    ]);
    for repo in ["two", "none", "missing"] {
        let package = github_package("o", repo);
        let plan = provider.plan_install_package(&package).await.unwrap();
        assert_eq!(plan, InstallPlan::OpenInBrowser(package.page_url), "{repo}");
    }
}

#[tokio::test]
async fn package_plan_propagates_errors_other_than_404() {
    let provider = plugin_provider(vec![(latest_url("broken"), Reply::Status(500))]);
    assert!(matches!(
        provider
            .plan_install_package(&github_package("o", "broken"))
            .await,
        Err(ProviderError::Http(HttpError::Status { status: 500, .. }))
    ));
}

#[tokio::test]
async fn web_package_opens_in_browser_without_a_request() {
    let http = FixtureHttp::new(Vec::<(String, Reply)>::new());
    let log = http.request_log();
    let provider = Ymm4Provider::new(http);
    let web_url = Url::parse("https://example.booth.pm/items/1").unwrap();
    let package = Package {
        source: PackageSource::Web(web_url.clone()),
        page_url: web_url.clone(),
        ..github_package("o", "x")
    };
    assert_eq!(
        provider.plan_install_package(&package).await.unwrap(),
        InstallPlan::OpenInBrowser(web_url)
    );
    assert!(log.lock().unwrap().is_empty());
}
