use std::collections::HashSet;

use launcher_core::{AppSelection, Edition, Host};
use launcher_providers::{FreyaProvider, NeoutlProvider, Provider, ReqwestHttp, Ymm4Provider};

fn http() -> ReqwestHttp {
    let token = std::env::var("GITHUB_TOKEN").ok();
    ReqwestHttp::new("MLAYN-Launcher-test/0.1", token).unwrap()
}

fn selection(host: Host, edition: Edition) -> AppSelection {
    AppSelection { host, edition }
}

/// Every host has at least one version with an installable artifact.
async fn assert_installable_on_each_host(provider: &impl Provider) {
    let versions = provider.app_versions().await.unwrap();
    assert!(!versions.is_empty());
    for host in [Host::Windows, Host::Linux] {
        let installable = versions.iter().any(|v| {
            provider
                .plan_install_app(v, selection(host, Edition::Normal))
                .is_ok()
        });
        assert!(installable, "no installable version for {host:?}");
    }
}

#[tokio::test]
#[ignore = "uses the network"]
async fn ymm4_live() {
    let provider = Ymm4Provider::new(http());
    let versions = provider.app_versions().await.unwrap();
    assert!(!versions.is_empty());
    for edition in [Edition::Normal, Edition::Lite] {
        provider
            .plan_install_app(&versions[0], selection(Host::Windows, edition))
            .unwrap();
    }

    let catalog = provider.catalog().await.unwrap();
    let ids: HashSet<_> = catalog.iter().map(|package| &package.id).collect();
    assert_eq!(ids.len(), catalog.len(), "package ids must be unique");
    assert!(catalog.iter().all(|package| !package.categories.is_empty()));
    assert!(catalog.iter().all(|package| package.updated.is_some()));
}

#[tokio::test]
#[ignore = "uses the network"]
async fn neoutl_live() {
    let provider = NeoutlProvider::new(http());
    let versions = provider.app_versions().await.unwrap();
    assert!(!versions.is_empty());
    for host in [Host::Windows, Host::Linux] {
        provider
            .plan_install_app(&versions[0], selection(host, Edition::Normal))
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "uses the network"]
async fn freya_live() {
    assert_installable_on_each_host(&FreyaProvider::new(http())).await;
}
