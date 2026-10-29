use std::future::Future;
use std::path::Path;

use url::Url;

use crate::Error;

pub const ALLOWED_HOSTS: [&str; 5] = [
    "github.com",
    "api.github.com",
    "codeberg.org",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Accepts `https` URLs whose host is listed in `ALLOWED_HOSTS`.
pub fn is_allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
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
