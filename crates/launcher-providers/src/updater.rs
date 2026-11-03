use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use launcher_core::{Progress, ProgressSink};
use self_update::backends::github::Update;
use tracing::info;

use crate::ProviderError;
use crate::dns::Dns;
use crate::fetch::allowed_redirects;
use crate::http::CONNECT_TIMEOUT;

const REPO_OWNER: &str = "TeamNeoUtl";
const REPO_NAME: &str = "Madoka-launcher";
const LAUNCHER_BIN: &str = "Madoka-Launcher";
const BROWSER_BIN: &str = "Madoka-Browser";
const TARGET: &str = if cfg!(windows) {
    "windows-x86_64"
} else {
    "linux-x86_64"
};
/// Layout of the release archive built by `release.yml`.
const BIN_PATH_IN_ARCHIVE: &str = "Madoka-launcher-v{{ version }}-{{ target }}/{{ bin }}";

/// Checks the GitHub Releases of the launcher and replaces the executables.
/// Methods block the calling thread. Run them on a blocking worker.
#[derive(Clone)]
pub struct SelfUpdater {
    user_agent: String,
    dns: Dns,
    token: Option<String>,
    current_version: String,
    api_base_url: Option<String>,
}

impl SelfUpdater {
    #[must_use]
    pub fn new(user_agent: &str, dns: Dns, token: Option<String>, current_version: &str) -> Self {
        Self {
            user_agent: user_agent.to_owned(),
            dns,
            token,
            current_version: current_version.to_owned(),
            api_base_url: None,
        }
    }

    /// Requests `url` instead of `https://api.github.com`.
    #[must_use]
    pub fn with_api_base_url(mut self, url: &str) -> Self {
        self.api_base_url = Some(url.to_owned());
        self
    }

    /// Returns the version of the latest release when it is newer than the running version.
    ///
    /// # Errors
    ///
    /// Returns `ProviderError::SelfUpdate` when the request fails or the release tag is not a version.
    pub fn check(&self) -> Result<Option<String>, ProviderError> {
        let releases = self
            .configure(LAUNCHER_BIN, None, None, None)?
            .get_latest_release()
            .map_err(failed)?;
        let newer = releases.is_update_available().map_err(failed)?;
        let version = releases
            .latest()
            .filter(|_| newer)
            .map(|release| release.version().to_owned());
        info!(current = %self.current_version, latest = ?version, "self update check");
        Ok(version)
    }

    /// Installs release `version`. `Madoka-Browser` is replaced before `Madoka-Launcher`.
    /// Each download is verified against the digest that GitHub publishes for the asset.
    ///
    /// # Errors
    ///
    /// Returns `ProviderError::SelfUpdate` when a download, a verification, or a replacement fails.
    pub fn update(
        &self,
        version: &str,
        progress: &Arc<dyn ProgressSink>,
    ) -> Result<(), ProviderError> {
        let tag = format!("v{version}");
        let browser = std::env::current_exe()
            .map_err(failed)?
            .with_file_name(browser_file_name());
        for (bin, install_path) in [(BROWSER_BIN, Some(browser)), (LAUNCHER_BIN, None)] {
            info!(bin, %tag, "self update install");
            self.configure(bin, install_path, Some(&tag), Some(progress))?
                .update_extended()
                .map_err(failed)?;
        }
        Ok(())
    }

    fn configure(
        &self,
        bin: &str,
        install_path: Option<PathBuf>,
        tag: Option<&str>,
        progress: Option<&Arc<dyn ProgressSink>>,
    ) -> Result<Update, ProviderError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(&self.user_agent)
            .redirect(allowed_redirects())
            .dns_resolver(Arc::new(self.dns.clone()))
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(failed)?;
        let mut builder = Update::configure();
        builder
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .target(TARGET)
            .bin_name(bin)
            .bin_path_in_archive(BIN_PATH_IN_ARCHIVE)
            .current_version(self.current_version.as_str())
            .reqwest_client(client)
            .check_install_path_writable(true)
            .show_download_progress(false)
            .unattended();
        if let Some(url) = &self.api_base_url {
            builder.api_base_url(url.as_str());
        }
        if let Some(token) = &self.token {
            builder.auth_token(token.as_str());
        }
        if let Some(path) = install_path {
            builder.bin_install_path(path);
        }
        if let Some(tag) = tag {
            builder.release_tag(tag);
        }
        if let Some(progress) = progress {
            let progress = Arc::clone(progress);
            builder.progress_callback(move |done, total| progress.report(Progress { done, total }));
        }
        builder.build().map_err(failed)
    }
}

const fn browser_file_name() -> &'static str {
    if cfg!(windows) {
        "Madoka-Browser.exe"
    } else {
        BROWSER_BIN
    }
}

fn failed(error: impl Display) -> ProviderError {
    ProviderError::SelfUpdate(error.to_string())
}
