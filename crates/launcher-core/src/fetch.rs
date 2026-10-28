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

/// Writes the body of `url` to `dest`. Implementations apply `is_allowed_url` to every redirect target.
pub trait Fetcher: Send + Sync {
    fn fetch(&self, url: &Url, dest: &Path) -> impl Future<Output = Result<(), Error>> + Send;
}
