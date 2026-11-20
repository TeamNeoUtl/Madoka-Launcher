use std::sync::LazyLock;

use launcher_core::{AppSelection, AppVersion, Arch, Artifact, Host, InstallPlan, Kind, Package};
use regex::Regex;
use url::Url;

use crate::release::{app_versions_from_json, parse_url};
use crate::{Http, Provider, ProviderError};

const RELEASES_URL: &str = "https://api.github.com/repos/Ren9618/MacUtl/releases?per_page=30";

/// The zip holds `MacUtl.app`. The `.dmg` of the same release is not installable here.
#[expect(clippy::expect_used, reason = "the pattern is a constant literal")]
static ASSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^MacUtl-.+-macos\.zip$").expect("static regex"));

/// Non-official macOS port of `NeoUtl`. Apple Silicon only.
pub struct MacutlProvider<H> {
    http: H,
}

impl<H: Http> MacutlProvider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }
}

impl<H: Http> Provider for MacutlProvider<H> {
    fn kind(&self) -> Kind {
        Kind::Macutl
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
        if selection.host != Host::MacOs || selection.arch != Arch::Aarch64 {
            return Err(ProviderError::UnsupportedHost {
                kind: Kind::Macutl,
                host: selection.host,
            });
        }
        version
            .artifacts
            .iter()
            .find(|artifact| ASSET.is_match(&artifact.name))
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
