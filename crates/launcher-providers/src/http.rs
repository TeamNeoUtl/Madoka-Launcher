use std::future::Future;

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

#[derive(Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
    github_token: Option<String>,
}

impl ReqwestHttp {
    pub fn new(user_agent: &str, github_token: Option<String>) -> Result<Self, HttpError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .map_err(|e| HttpError::Transport {
                url: String::new(),
                message: e.to_string(),
            })?;
        Ok(Self {
            client,
            github_token,
        })
    }
}

impl Http for ReqwestHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        let transport = |e: reqwest::Error| HttpError::Transport {
            url: url.to_string(),
            message: e.to_string(),
        };
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
            return Err(HttpError::Status {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        Ok(response.bytes().await.map_err(transport)?.to_vec())
    }
}
