use regex::Regex;
use url::Url;

use crate::release::parse_url;
use crate::{Http, ProviderError};

const HISTORY_URL: &str = "https://spring-fragrance.mints.ne.jp/aviutl/oldver2.php";

pub(crate) async fn fetch_page<H: Http>(http: &H) -> Result<Vec<u8>, ProviderError> {
    Ok(http.get(&parse_url(HISTORY_URL)?).await?)
}

/// `(version, archive URL)` of every match of `pattern` in `page`, in page order.
/// Capture group 1 is the link target, capture group 2 is the version.
pub(crate) fn parse_rows(page: &[u8], pattern: &str) -> Result<Vec<(String, Url)>, ProviderError> {
    let base = parse_url(HISTORY_URL)?;
    let text = String::from_utf8_lossy(page);
    let row = Regex::new(pattern).map_err(|e| ProviderError::CatalogShapeChanged(e.to_string()))?;
    row.captures_iter(&text)
        .map(|capture| {
            let url = base
                .join(&capture[1])
                .map_err(|_| ProviderError::InvalidUrl(capture[1].to_owned()))?;
            Ok((capture[2].to_owned(), url))
        })
        .collect()
}
