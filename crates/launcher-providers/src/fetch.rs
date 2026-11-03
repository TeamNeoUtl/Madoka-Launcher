use std::path::Path;
use std::time::{Duration, Instant};

use launcher_core::{Error, Fetcher, Progress, ProgressSink, is_allowed_url};
use reqwest::redirect::Policy;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};
use url::Url;

use crate::dns::Dns;
use crate::http::{CONNECT_TIMEOUT, describe};

/// A transfer that receives no data for this long fails.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 10;
const PROGRESS_STEP: u64 = 8 * 1024 * 1024;

/// Streams a response body to a file. Every redirect target must satisfy `is_allowed_url`.
pub struct ReqwestFetcher {
    client: reqwest::Client,
}

impl ReqwestFetcher {
    /// Uses the default `Dns`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Fetch` when the HTTP client cannot be built.
    pub fn new(user_agent: &str) -> Result<Self, Error> {
        Self::with_dns(user_agent, Dns::default())
    }

    /// Allows redirects to hosts that `is_allowed_url` accepts.
    ///
    /// # Errors
    ///
    /// Returns `Error::Fetch` when the HTTP client cannot be built.
    pub fn with_dns(user_agent: &str, dns: Dns) -> Result<Self, Error> {
        let policy = Policy::custom(|attempt| {
            if is_allowed_url(attempt.url()) {
                debug!(to = %attempt.url(), "redirect");
                attempt.follow()
            } else {
                let message = format!("redirect to {} is not allowed", attempt.url());
                warn!(to = %attempt.url(), "redirect rejected");
                attempt.error(message)
            }
        });
        Self::build(user_agent, dns, policy)
    }

    /// Follows redirects to any host. The caller holds the user's consent for the first URL.
    ///
    /// # Errors
    ///
    /// Returns `Error::Fetch` when the HTTP client cannot be built.
    pub fn unrestricted(user_agent: &str, dns: Dns) -> Result<Self, Error> {
        Self::build(user_agent, dns, Policy::limited(MAX_REDIRECTS))
    }

    fn build(user_agent: &str, dns: Dns, policy: Policy) -> Result<Self, Error> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(policy)
            .dns_resolver(dns)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|e| Error::Fetch {
                url: String::new(),
                message: describe(&e),
            })?;
        Ok(Self { client })
    }
}

impl Fetcher for ReqwestFetcher {
    async fn fetch(
        &self,
        url: &Url,
        dest: &Path,
        progress: &dyn ProgressSink,
    ) -> Result<(), Error> {
        let fail = |message: String| Error::Fetch {
            url: url.to_string(),
            message,
        };
        let started = Instant::now();
        info!(%url, dest = %dest.display(), "fetch start");
        let mut response = self.client.get(url.clone()).send().await.map_err(|e| {
            warn!(%url, error = %describe(&e), "fetch request failed");
            fail(describe(&e))
        })?;
        let status = response.status();
        info!(
            %url,
            status = status.as_u16(),
            content_length = ?response.content_length(),
            elapsed_ms = started.elapsed().as_millis(),
            "fetch response"
        );
        if !status.is_success() {
            return Err(fail(format!("status {}", status.as_u16())));
        }
        let total = response.content_length();
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| fail(e.to_string()))?;
        let mut written: u64 = 0;
        progress.report(Progress { done: 0, total });
        let mut next_report = PROGRESS_STEP;
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            warn!(%url, written, error = %describe(&e), "fetch body failed");
            fail(describe(&e))
        })? {
            file.write_all(&chunk)
                .await
                .map_err(|e| fail(e.to_string()))?;
            written += chunk.len() as u64;
            progress.report(Progress {
                done: written,
                total,
            });
            if written >= next_report {
                debug!(%url, written, elapsed_ms = started.elapsed().as_millis(), "fetch progress");
                next_report += PROGRESS_STEP;
            }
        }
        file.flush().await.map_err(|e| fail(e.to_string()))?;
        info!(%url, written, elapsed_ms = started.elapsed().as_millis(), "fetch done");
        Ok(())
    }
}
