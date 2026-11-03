use std::sync::LazyLock;

use launcher_core::{AppSelection, AppVersion, Artifact, Host, InstallPlan, Kind, Package};
use regex::Regex;
use url::Url;

use crate::release::{app_versions_from_json, parse_url};
use crate::{Http, Provider, ProviderError};

const RELEASES_URL: &str = "https://api.github.com/repos/GT-610/freya-nle/releases?per_page=30";

/// Release asset names keep the identifier that predates the rename to Freya NLE.
static WINDOWS_ASSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^AviQtl-(MSVC|MSYS2-UCRT64)-x86_64\.zip$").expect("static regex")
});
const LINUX_ASSET: &str = "AviQtl-Arch-Linux-x86_64.zip";

pub struct FreyaProvider<H> {
    http: H,
}

impl<H: Http> FreyaProvider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }
}

fn is_asset_of(host: Host, name: &str) -> bool {
    match host {
        Host::Windows => WINDOWS_ASSET.is_match(name),
        Host::Linux => name == LINUX_ASSET,
    }
}

impl<H: Http> Provider for FreyaProvider<H> {
    fn kind(&self) -> Kind {
        Kind::Freya
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        let url = parse_url(RELEASES_URL)?;
        let bytes = self.http.get(&url).await?;
        app_versions_from_json(&bytes, |name| {
            is_asset_of(Host::Windows, name) || is_asset_of(Host::Linux, name)
        })
    }

    fn catalog(&self) -> impl Future<Output = Result<Vec<Package>, ProviderError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        version
            .artifacts
            .iter()
            .find(|artifact| is_asset_of(selection.host, &artifact.name))
            .cloned()
            .map(|artifact| vec![artifact])
            .ok_or_else(|| ProviderError::NoMatchingAsset {
                version: version.version.clone(),
            })
    }

    fn plan_install_package(
        &self,
        package: &Package,
    ) -> impl Future<Output = Result<InstallPlan, ProviderError>> + Send {
        std::future::ready(Ok(InstallPlan::OpenInBrowser(Url::clone(
            &package.page_url,
        ))))
    }
}
