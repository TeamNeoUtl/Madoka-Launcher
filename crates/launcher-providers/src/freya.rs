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
static LINUX_ASSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^AviQtl-Arch-Linux-x86_64\.zip$").expect("static regex"));

pub struct FreyaProvider<H> {
    http: H,
}

impl<H: Http> FreyaProvider<H> {
    pub fn new(http: H) -> Self {
        Self { http }
    }
}

fn asset_pattern(host: Host) -> &'static Regex {
    match host {
        Host::Windows => &WINDOWS_ASSET,
        Host::Linux => &LINUX_ASSET,
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
            WINDOWS_ASSET.is_match(name) || LINUX_ASSET.is_match(name)
        })
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        Ok(Vec::new())
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        let pattern = asset_pattern(selection.host);
        version
            .artifacts
            .iter()
            .find(|artifact| pattern.is_match(&artifact.name))
            .cloned()
            .map(|artifact| vec![artifact])
            .ok_or_else(|| ProviderError::NoMatchingAsset {
                version: version.version.clone(),
            })
    }

    async fn plan_install_package(&self, package: &Package) -> Result<InstallPlan, ProviderError> {
        Ok(InstallPlan::OpenInBrowser(Url::clone(&package.page_url)))
    }
}
