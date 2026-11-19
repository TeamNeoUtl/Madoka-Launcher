use std::sync::LazyLock;

use launcher_core::{AppSelection, AppVersion, Artifact, Bundle, Host, InstallPlan, Kind, Package};
use regex::Regex;
use url::Url;

use crate::release::{app_versions_from_json, parse_url};
use crate::{Http, Provider, ProviderError};

const RELEASES_URL: &str = "https://api.github.com/repos/GT-610/freya-nle/releases?per_page=30";

/// Releases before 0.8 name the Linux archive after the identifier that predates Freya NLE.
const LEGACY_LINUX_ARCHIVE: &str = "AviQtl-Arch-Linux-x86_64.zip";
const BUNDLES: [Bundle; 2] = [Bundle::Archive, Bundle::AppImage];

#[expect(clippy::expect_used, reason = "the patterns are constant literals")]
fn pattern(text: &str) -> Regex {
    Regex::new(text).expect("static regex")
}

/// `AviQtl-…` names belong to releases before 0.8. Later releases name assets
/// `Freya-NLE-<version>-<platform>.<extension>`.
static WINDOWS_ASSET: LazyLock<Regex> = LazyLock::new(|| {
    pattern(r"^(?:AviQtl-(?:MSVC|MSYS2-UCRT64)-x86_64|Freya-NLE-.+-windows-x86_64)\.zip$")
});
static LINUX_ARCHIVE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^Freya-NLE-.+-linux-x86_64-archlinux\.tar\.zst$"));
static LINUX_APPIMAGE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^Freya-NLE-.+-linux-x86_64\.AppImage$"));

pub struct FreyaProvider<H> {
    http: H,
}

impl<H: Http> FreyaProvider<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }
}

fn is_asset_of(host: Host, bundle: Bundle, name: &str) -> bool {
    match (host, bundle) {
        (Host::Windows, _) => WINDOWS_ASSET.is_match(name),
        (Host::Linux, Bundle::Archive) => {
            name == LEGACY_LINUX_ARCHIVE || LINUX_ARCHIVE.is_match(name)
        }
        (Host::Linux, Bundle::AppImage) => LINUX_APPIMAGE.is_match(name),
    }
}

fn is_any_asset(name: &str) -> bool {
    is_asset_of(Host::Windows, Bundle::Archive, name)
        || BUNDLES
            .into_iter()
            .any(|bundle| is_asset_of(Host::Linux, bundle, name))
}

impl<H: Http> Provider for FreyaProvider<H> {
    fn kind(&self) -> Kind {
        Kind::Freya
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        let url = parse_url(RELEASES_URL)?;
        let bytes = self.http.get(&url).await?;
        app_versions_from_json(&bytes, is_any_asset)
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
            .find(|artifact| is_asset_of(selection.host, selection.bundle, &artifact.name))
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
