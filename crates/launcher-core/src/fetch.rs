use std::future::Future;
use std::path::Path;

use url::Url;

use crate::Error;

/// Hosts that `https` downloads may use.
pub const ALLOWED_HOSTS: [&str; 6] = [
    "github.com",
    "api.github.com",
    "codeberg.org",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "spring-fragrance.mints.ne.jp",
];

/// Hosts that `http` downloads may use. The installer requires a hash for such a download.
pub const HTTP_HOSTS: [&str; 1] = ["spring-fragrance.mints.ne.jp"];

/// Accepts `https` URLs whose host is listed in `ALLOWED_HOSTS`
/// and `http` URLs whose host is listed in `HTTP_HOSTS`.
#[must_use]
pub fn is_allowed_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    match url.scheme() {
        "https" => ALLOWED_HOSTS.contains(&host),
        "http" => HTTP_HOSTS.contains(&host),
        _ => false,
    }
}

/// Bytes received so far. `total` is `None` when the server does not report a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: Option<u64>,
}

pub trait ProgressSink: Send + Sync {
    fn report(&self, progress: Progress);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn report(&self, _progress: Progress) {}
}

/// Writes the body of `url` to `dest` and reports progress after the response header and after
/// every received chunk. Implementations apply `is_allowed_url` to every redirect target.
pub trait Fetcher: Send + Sync {
    fn fetch(
        &self,
        url: &Url,
        dest: &Path,
        progress: &dyn ProgressSink,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}
