use std::future::Future;

use launcher_core::{AppSelection, AppVersion, Artifact, InstallPlan, Kind, Package};

use crate::ProviderError;

pub trait Provider: Send + Sync {
    fn kind(&self) -> Kind;

    fn app_versions(&self) -> impl Future<Output = Result<Vec<AppVersion>, ProviderError>> + Send;

    fn catalog(&self) -> impl Future<Output = Result<Vec<Package>, ProviderError>> + Send;

    /// # Errors
    ///
    /// Returns `ProviderError::UnsupportedHost` when `selection.host` cannot run the kind,
    /// and `ProviderError::NoMatchingAsset` when `version` has no artifact for `selection`.
    fn plan_install_app(
        &self,
        version: &AppVersion,
        selection: AppSelection,
    ) -> Result<Vec<Artifact>, ProviderError>;

    fn plan_install_package(
        &self,
        package: &Package,
    ) -> impl Future<Output = Result<InstallPlan, ProviderError>> + Send;
}
