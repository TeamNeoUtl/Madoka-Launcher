use std::path::Path;

use launcher_core::{Error, Fetcher, is_allowed_url};
use reqwest::redirect::Policy;
use tokio::io::AsyncWriteExt;
use url::Url;

/// Streams a response body to a file. Every redirect target must satisfy `is_allowed_url`.
pub struct ReqwestFetcher {
    client: reqwest::Client,
}

impl ReqwestFetcher {
    pub fn new(user_agent: &str) -> Result<Self, Error> {
        let policy = Policy::custom(|attempt| {
            if is_allowed_url(attempt.url()) {
                attempt.follow()
            } else {
                let message = format!("redirect to {} is not allowed", attempt.url());
                attempt.error(message)
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(policy)
            .build()
            .map_err(|e| Error::Fetch {
                url: String::new(),
                message: e.to_string(),
            })?;
        Ok(Self { client })
    }
}

impl Fetcher for ReqwestFetcher {
    async fn fetch(&self, url: &Url, dest: &Path) -> Result<(), Error> {
        let fail = |message: String| Error::Fetch {
            url: url.to_string(),
            message,
        };
        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| fail(e.to_string()))?;
        if !response.status().is_success() {
            return Err(fail(format!("status {}", response.status().as_u16())));
        }
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| fail(e.to_string()))?;
        while let Some(chunk) = response.chunk().await.map_err(|e| fail(e.to_string()))? {
            file.write_all(&chunk)
                .await
                .map_err(|e| fail(e.to_string()))?;
        }
        file.flush().await.map_err(|e| fail(e.to_string()))
    }
}
