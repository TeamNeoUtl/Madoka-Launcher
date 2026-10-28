use launcher_core::{AppSelection, Edition, Host};
use launcher_providers::{FreyaProvider, NeoutlProvider, Provider, ReqwestHttp, Ymm4Provider};

fn http() -> ReqwestHttp {
    let token = std::env::var("GITHUB_TOKEN").ok();
    ReqwestHttp::new("MLAYN-Launcher-test/0.1", token).unwrap()
}

#[tokio::test]
#[ignore = "uses the network"]
async fn ymm4_live() {
    let provider = Ymm4Provider::new(http());
    let versions = provider.app_versions().await.unwrap();
    let selection = AppSelection {
        host: Host::Windows,
        edition: Edition::Lite,
    };
    provider.plan_install_app(&versions[0], selection).unwrap();
    assert!(provider.catalog().await.unwrap().len() > 400);
}

#[tokio::test]
#[ignore = "uses the network"]
async fn neoutl_live() {
    let provider = NeoutlProvider::new(http());
    let versions = provider.app_versions().await.unwrap();
    let selection = AppSelection {
        host: Host::Linux,
        edition: Edition::Normal,
    };
    provider.plan_install_app(&versions[0], selection).unwrap();
}

#[tokio::test]
#[ignore = "uses the network"]
async fn freya_live() {
    let provider = FreyaProvider::new(http());
    let versions = provider.app_versions().await.unwrap();
    let selection = AppSelection {
        host: Host::Windows,
        edition: Edition::Normal,
    };
    provider.plan_install_app(&versions[0], selection).unwrap();
}
