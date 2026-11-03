use std::sync::LazyLock;

use launcher_core::{AppSelection, AppVersion, Artifact, Host, InstallPlan, Kind, Package};
use regex::Regex;
use url::Url;

use crate::release::{app_versions_from_json, parse_url};
use crate::{Http, Provider, ProviderError};

const RELEASES_URL: &str = "https://codeberg.org/api/v1/repos/TeamNeoUtl/NeoUtl/releases?limit=50";

static ASSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^neoutl-(linux|windows)-x86_64\.zip$").expect("static regex"));

pub struct NeoutlProvider<H> {
    http: H,
}

impl<H: Http> NeoutlProvider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }
}

const fn asset_name(host: Host) -> &'static str {
    match host {
        Host::Windows => "neoutl-windows-x86_64.zip",
        Host::Linux => "neoutl-linux-x86_64.zip",
    }
}

impl<H: Http> Provider for NeoutlProvider<H> {
    fn kind(&self) -> Kind {
        Kind::Neoutl
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        let url = parse_url(RELEASES_URL)?;
        let bytes = self.http.get(&url).await?;
        app_versions_from_json(&bytes, |name| ASSET.is_match(name))
    }

    fn catalog(&self) -> impl Future<Output = Result<Vec<Package>, ProviderError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        let name = asset_name(selection.host);
        version
            .artifacts
            .iter()
            .find(|artifact| artifact.name == name)
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
