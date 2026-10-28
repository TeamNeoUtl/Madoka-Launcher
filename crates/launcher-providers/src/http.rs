use std::error::Error as _;
use std::future::Future;
use std::time::{Duration, Instant};

use tracing::{debug, warn};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("status {status} for {url}")]
    Status { status: u16, url: String },
    #[error("transport error for {url}: {message}")]
    Transport { url: String, message: String },
}

pub trait Http: Send + Sync {
    fn get(&self, url: &Url) -> impl Future<Output = Result<Vec<u8>, HttpError>> + Send;
}

const GITHUB_API_HOST: &str = "api.github.com";
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Joins the message of `error` and of every error in its source chain.
pub(crate) fn describe(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    if error.is_timeout() {
        text.insert_str(0, "timeout: ");
    }
    text
}

#[derive(Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
    github_token: Option<String>,
}

impl ReqwestHttp {
    pub fn new(user_agent: &str, github_token: Option<String>) -> Result<Self, HttpError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| HttpError::Transport {
                url: String::new(),
                message: describe(&e),
            })?;
        Ok(Self {
            client,
            github_token,
        })
    }
}

impl Http for ReqwestHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        let started = Instant::now();
        let transport = |e: reqwest::Error| {
            let message = describe(&e);
            warn!(%url, error = %message, elapsed_ms = started.elapsed().as_millis(), "http get failed");
            HttpError::Transport {
                url: url.to_string(),
                message,
            }
        };
        debug!(%url, "http get");
        let mut request = self.client.get(url.clone());
        if url.host_str() == Some(GITHUB_API_HOST) {
            request = request.header("Accept", "application/vnd.github+json");
            if let Some(token) = &self.github_token {
                request = request.bearer_auth(token);
            }
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        if !status.is_success() {
            warn!(%url, status = status.as_u16(), "http get returned an error status");
            return Err(HttpError::Status {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        let bytes = response.bytes().await.map_err(transport)?;
        debug!(
            %url,
            status = status.as_u16(),
            bytes = bytes.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "http get done"
        );
        Ok(bytes.to_vec())
    }
}
