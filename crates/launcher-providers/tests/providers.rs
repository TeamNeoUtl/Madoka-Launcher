use std::collections::HashMap;

use launcher_core::{AppSelection, Edition, Host, InstallPlan, Kind, Package, PackageSource};
use launcher_providers::{
    FreyaProvider, Http, HttpError, NeoutlProvider, Provider, ProviderError, Ymm4Provider,
    parse_ymm4_catalog,
};
use url::Url;

const YMM4_YAML: &[u8] = include_bytes!("fixtures/ymm4plugins.yml");
const YMM4_RELEASES: &[u8] = include_bytes!("fixtures/ymm4_releases.json");
const NEOUTL_RELEASES: &[u8] = include_bytes!("fixtures/neoutl_releases.json");
const FREYA_RELEASES: &[u8] = include_bytes!("fixtures/freya_releases.json");
const ONE_YMME: &[u8] = include_bytes!("fixtures/latest_release_one_ymme.json");
const TWO_YMME: &[u8] = include_bytes!("fixtures/latest_release_two_ymme.json");
const NO_ASSET: &[u8] = include_bytes!("fixtures/latest_release_no_asset.json");

struct FixtureHttp {
    responses: HashMap<String, Vec<u8>>,
}

impl FixtureHttp {
    fn new(responses: &[(&str, &[u8])]) -> Self {
        Self {
            responses: responses
                .iter()
                .map(|(url, body)| ((*url).to_owned(), body.to_vec()))
                .collect(),
        }
    }
}

impl Http for FixtureHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        self.responses
            .get(url.as_str())
            .cloned()
            .ok_or_else(|| HttpError::Status {
                status: 404,
                url: url.to_string(),
            })
    }
}

fn selection(host: Host, edition: Edition) -> AppSelection {
    AppSelection { host, edition }
}

fn github_package(owner: &str, repo: &str) -> Package {
    let page_url = Url::parse(&format!("https://github.com/{owner}/{repo}")).unwrap();
    Package {
        id: format!("github:{owner}/{repo}"),
        name: "p".to_owned(),
        summary: String::new(),
        author: String::new(),
        categories: Vec::new(),
        updated: None,
        page_url,
        source: PackageSource::GithubRepo {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        },
    }
}

#[test]
fn ymm4_catalog_parses_all_entries() {
    let packages = parse_ymm4_catalog(YMM4_YAML).unwrap();
    assert_eq!(packages.len(), 412);

    let ids: std::collections::HashSet<_> = packages.iter().map(|p| &p.id).collect();
    assert_eq!(ids.len(), packages.len());

    let github = packages
        .iter()
        .filter(|p| matches!(p.source, PackageSource::GithubRepo { .. }))
        .count();
    assert_eq!(github, 284);

    assert!(packages.iter().any(|p| p.categories.len() > 1));
    assert!(packages.iter().all(|p| !p.categories.is_empty()));
    assert!(packages.iter().all(|p| p.updated.is_some()));
}

#[test]
fn ymm4_catalog_detects_shape_change() {
    assert!(matches!(
        parse_ymm4_catalog(b"[]"),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
    assert!(matches!(
        parse_ymm4_catalog(b"- type: a\n  name: b\n"),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
    assert!(matches!(
        parse_ymm4_catalog(b"<html></html>"),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
    let disabled = "- {type: a, date: '2026-01-01', name: n, url: 'https://x.example/', \
                    author: a, authorUrl: 'https://x.example/', description: d, isEnabled: false}";
    assert!(matches!(
        parse_ymm4_catalog(disabled.as_bytes()),
        Err(ProviderError::CatalogShapeChanged(_))
    ));
}

#[tokio::test]
async fn ymm4_app_versions_and_plan() {
    let http = FixtureHttp::new(&[(
        "https://api.github.com/repos/manju-summoner/YukkuriMovieMaker4/releases?per_page=30",
        YMM4_RELEASES,
    )]);
    let provider = Ymm4Provider::new(http);
    let versions = provider.app_versions().await.unwrap();
    assert_eq!(versions.len(), 5);
    assert_eq!(versions[0].version, "v4.56.1.0");
    assert_eq!(versions[0].artifacts.len(), 2);

    let normal = provider
        .plan_install_app(&versions[0], selection(Host::Windows, Edition::Normal))
        .unwrap();
    assert_eq!(normal[0].name, "YukkuriMovieMaker_v4.56.1.0.zip");
    assert_eq!(
        normal[0].hash.as_ref().unwrap().hex,
        "34b5f4c737345f1b82507381af514fc14b629f35e9ba778c25434f235fdb35cf"
    );

    let lite = provider
        .plan_install_app(&versions[0], selection(Host::Windows, Edition::Lite))
        .unwrap();
    assert_eq!(lite[0].name, "YukkuriMovieMaker_v4.56.1.0_Lite.zip");

    assert!(matches!(
        provider.plan_install_app(&versions[0], selection(Host::Linux, Edition::Normal)),
        Err(ProviderError::UnsupportedHost {
            kind: Kind::Ymm4,
            ..
        })
    ));
}

#[tokio::test]
async fn ymm4_catalog_via_http() {
    let provider = Ymm4Provider::new(FixtureHttp::new(&[(
        "https://manjubox.net/ymm4plugins.yml",
        YMM4_YAML,
    )]));
    assert_eq!(provider.catalog().await.unwrap().len(), 412);
}

#[tokio::test]
async fn ymm4_package_plan() {
    let http = FixtureHttp::new(&[
        (
            "https://api.github.com/repos/o/one/releases/latest",
            ONE_YMME,
        ),
        (
            "https://api.github.com/repos/o/two/releases/latest",
            TWO_YMME,
        ),
        (
            "https://api.github.com/repos/o/none/releases/latest",
            NO_ASSET,
        ),
    ]);
    let provider = Ymm4Provider::new(http);

    let plan = provider
        .plan_install_package(&github_package("o", "one"))
        .await
        .unwrap();
    let InstallPlan::Download(artifacts) = plan else {
        panic!("expected download");
    };
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].name, "Plugin.ymme");
    assert!(artifacts[0].hash.is_some());

    for repo in ["two", "none", "missing"] {
        let package = github_package("o", repo);
        let plan = provider.plan_install_package(&package).await.unwrap();
        assert_eq!(plan, InstallPlan::OpenInBrowser(package.page_url), "{repo}");
    }

    let web_url = Url::parse("https://example.booth.pm/items/1").unwrap();
    let web = Package {
        source: PackageSource::Web(web_url.clone()),
        page_url: web_url.clone(),
        ..github_package("o", "x")
    };
    assert_eq!(
        provider.plan_install_package(&web).await.unwrap(),
        InstallPlan::OpenInBrowser(web_url)
    );
}

#[tokio::test]
async fn neoutl_selects_stable_assets_only() {
    let provider = NeoutlProvider::new(FixtureHttp::new(&[(
        "https://codeberg.org/api/v1/repos/TeamNeoUtl/NeoUtl/releases?limit=50",
        NEOUTL_RELEASES,
    )]));
    let versions = provider.app_versions().await.unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, "0.8.11");
    assert!(!versions[0].prerelease);

    let windows = provider
        .plan_install_app(&versions[0], selection(Host::Windows, Edition::Normal))
        .unwrap();
    assert_eq!(windows[0].name, "neoutl-windows-x86_64.zip");
    assert!(windows[0].hash.is_none());

    let linux = provider
        .plan_install_app(&versions[0], selection(Host::Linux, Edition::Normal))
        .unwrap();
    assert_eq!(linux[0].name, "neoutl-linux-x86_64.zip");
    assert!(provider.catalog().await.unwrap().is_empty());
}

#[tokio::test]
async fn freya_selects_asset_per_host() {
    let provider = FreyaProvider::new(FixtureHttp::new(&[(
        "https://api.github.com/repos/GT-610/freya-nle/releases?per_page=30",
        FREYA_RELEASES,
    )]));
    let versions = provider.app_versions().await.unwrap();
    assert_eq!(versions.len(), 10);
    assert_eq!(versions[0].version, "v0.7.0-rc.1");

    let windows = provider
        .plan_install_app(&versions[0], selection(Host::Windows, Edition::Normal))
        .unwrap();
    assert_eq!(windows[0].name, "AviQtl-MSVC-x86_64.zip");
    assert_eq!(
        windows[0].hash.as_ref().unwrap().hex,
        "f392bc7017692e7ed7cf9e9dbf6f1f9ea6ba9547f4c116472e2114e47995ab45"
    );

    let linux = provider
        .plan_install_app(&versions[0], selection(Host::Linux, Edition::Normal))
        .unwrap();
    assert_eq!(linux[0].name, "AviQtl-Arch-Linux-x86_64.zip");

    let msys2 = versions.last().unwrap();
    let windows_old = provider
        .plan_install_app(msys2, selection(Host::Windows, Edition::Normal))
        .unwrap();
    assert_eq!(windows_old[0].name, "AviQtl-MSYS2-UCRT64-x86_64.zip");
    assert!(provider.catalog().await.unwrap().is_empty());
}
