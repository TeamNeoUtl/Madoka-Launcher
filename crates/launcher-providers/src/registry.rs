use launcher_core::{AppSelection, AppVersion, Artifact, InstallPlan, Kind, Package};

use crate::{
    Aviutl2Provider, AviutlProvider, FreyaProvider, Http, NeoutlProvider, Provider, ProviderError,
    Ymm4Provider,
};

/// Kinds for which `AnyProvider::for_kind` returns a provider.
#[must_use]
pub const fn has_provider(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Aviutl | Kind::Aviutl2 | Kind::Ymm4 | Kind::Neoutl | Kind::Freya
    )
}

/// Kinds whose `Provider::catalog` returns packages.
#[must_use]
pub const fn has_catalog(kind: Kind) -> bool {
    matches!(kind, Kind::Aviutl | Kind::Aviutl2 | Kind::Ymm4)
}

/// Dispatches `Provider` calls by `Kind`. `Provider` is not object-safe.
pub enum AnyProvider<H> {
    Aviutl(AviutlProvider<H>),
    Aviutl2(Aviutl2Provider<H>),
    Ymm4(Ymm4Provider<H>),
    Neoutl(NeoutlProvider<H>),
    Freya(FreyaProvider<H>),
}

impl<H: Http> AnyProvider<H> {
    /// # Errors
    ///
    /// Returns `ProviderError::NoProvider` when `kind` has no provider.
    /// Every current `Kind` has one.
    pub const fn for_kind(kind: Kind, http: H) -> Result<Self, ProviderError> {
        match kind {
            Kind::Aviutl => Ok(Self::Aviutl(AviutlProvider::new(http))),
            Kind::Aviutl2 => Ok(Self::Aviutl2(Aviutl2Provider::new(http))),
            Kind::Ymm4 => Ok(Self::Ymm4(Ymm4Provider::new(http))),
            Kind::Neoutl => Ok(Self::Neoutl(NeoutlProvider::new(http))),
            Kind::Freya => Ok(Self::Freya(FreyaProvider::new(http))),
        }
    }
}

macro_rules! dispatch {
    ($self:expr, $provider:ident => $call:expr) => {
        match $self {
            Self::Aviutl($provider) => $call,
            Self::Aviutl2($provider) => $call,
            Self::Ymm4($provider) => $call,
            Self::Neoutl($provider) => $call,
            Self::Freya($provider) => $call,
        }
    };
}

impl<H: Http> Provider for AnyProvider<H> {
    fn kind(&self) -> Kind {
        dispatch!(self, provider => provider.kind())
    }

    async fn app_versions(&self) -> Result<Vec<AppVersion>, ProviderError> {
        dispatch!(self, provider => provider.app_versions().await)
    }

    async fn catalog(&self) -> Result<Vec<Package>, ProviderError> {
        dispatch!(self, provider => provider.catalog().await)
    }

    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError> {
        dispatch!(self, provider => provider.plan_install_app(version, selection))
    }

    async fn plan_install_package(&self, package: &Package) -> Result<InstallPlan, ProviderError> {
        dispatch!(self, provider => provider.plan_install_package(package).await)
    }
}
