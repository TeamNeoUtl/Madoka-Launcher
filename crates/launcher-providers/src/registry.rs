use launcher_core::{AppSelection, AppVersion, Artifact, InstallPlan, Kind, Package};

use crate::{FreyaProvider, Http, NeoutlProvider, Provider, ProviderError, Ymm4Provider};

/// Kinds for which `AnyProvider::for_kind` returns a provider.
pub const fn has_provider(kind: Kind) -> bool {
    matches!(kind, Kind::Ymm4 | Kind::Neoutl | Kind::Freya)
}

/// Dispatches `Provider` calls by `Kind`. `Provider` is not object-safe.
pub enum AnyProvider<H> {
    Ymm4(Ymm4Provider<H>),
    Neoutl(NeoutlProvider<H>),
    Freya(FreyaProvider<H>),
}

impl<H: Http> AnyProvider<H> {
    pub fn for_kind(kind: Kind, http: H) -> Result<Self, ProviderError> {
        match kind {
            Kind::Ymm4 => Ok(Self::Ymm4(Ymm4Provider::new(http))),
            Kind::Neoutl => Ok(Self::Neoutl(NeoutlProvider::new(http))),
            Kind::Freya => Ok(Self::Freya(FreyaProvider::new(http))),
            Kind::Aviutl | Kind::Aviutl2 => Err(ProviderError::NoProvider(kind)),
        }
    }
}

impl<H: Http> Provider for AnyProvider<H> {
    fn kind(&self) -> Kind {
        match self {
            Self::Ymm4(provider) => provider.kind(),
            Self::Neoutl(provider) => provider.kind(),
            Self::Freya(provider) => provider.kind(),
        }
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        match self {
            Self::Ymm4(provider) => provider.app_versions().await,
            Self::Neoutl(provider) => provider.app_versions().await,
            Self::Freya(provider) => provider.app_versions().await,
        }
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        match self {
            Self::Ymm4(provider) => provider.catalog().await,
            Self::Neoutl(provider) => provider.catalog().await,
            Self::Freya(provider) => provider.catalog().await,
        }
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        match self {
            Self::Ymm4(provider) => provider.plan_install_app(version, selection),
            Self::Neoutl(provider) => provider.plan_install_app(version, selection),
            Self::Freya(provider) => provider.plan_install_app(version, selection),
        }
    }

    async fn plan_install_package(&self, package: &Package) -> Result<InstallPlan, ProviderError> {
        match self {
            Self::Ymm4(provider) => provider.plan_install_package(package).await,
            Self::Neoutl(provider) => provider.plan_install_package(package).await,
            Self::Freya(provider) => provider.plan_install_package(package).await,
        }
    }
}
