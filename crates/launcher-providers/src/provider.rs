use std::future::Future;

use launcher_core::{AppSelection, AppVersion, Artifact, InstallPlan, Kind, Package};

use crate::ProviderError;

/// Versions of the editor and of the plugin that the editor needs beside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Versions {
    pub app: Vec<AppVersion>,
    /// Empty when the kind has no such plugin.
    pub addon: Vec<AppVersion>,
}

pub trait Provider: Send + Sync {
    fn kind(&self) -> Kind;

    fn app_versions(&self) -> impl Future<Output = Result<Vec<AppVersion>, ProviderError>> + Send;

    /// `app_versions` and the addon versions. One request set serves both lists.
    fn versions(&self) -> impl Future<Output = Result<Versions, ProviderError>> + Send {
        async {
            Ok(Versions {
                app: self.app_versions().await?,
                addon: Vec::new(),
            })
        }
    }

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
