//! Confirmation page of Google Drive for files above the virus-scan size limit.

use url::Url;

const DOWNLOAD_ENDPOINT: &str = "https://drive.usercontent.google.com/download";

/// Reads the download URL out of the `download-form` of a confirmation page.
/// The URL is the form action with every named input as a query parameter.
#[must_use]
pub fn confirm_url(html: &str) -> Option<Url> {
    let mut rest = html;
    while let Some(start) = rest.find("<form") {
        rest = &rest[start..];
        let tag_end = rest.find('>')?;
        let tag = &rest[..=tag_end];
        rest = &rest[tag_end + 1..];
        if attribute(tag, "id").as_deref() != Some("download-form") {
            continue;
        }
        let body = &rest[..rest.find("</form>").unwrap_or(rest.len())];
        let mut url = Url::parse(&attribute(tag, "action")?).ok()?;
        for input in tags(body, "<input") {
            if let Some(name) = attribute(input, "name") {
                let value = attribute(input, "value").unwrap_or_default();
                url.query_pairs_mut().append_pair(&name, &value);
            }
        }
        return Some(url);
    }
    None
}

/// Download URL that skips the confirmation page.
#[must_use]
pub fn fallback_url(file_id: &str) -> Option<Url> {
    Url::parse_with_params(
        DOWNLOAD_ENDPOINT,
        [("id", file_id), ("export", "download"), ("confirm", "t")],
    )
    .ok()
}

fn tags<'a>(html: &'a str, open: &'a str) -> impl Iterator<Item = &'a str> {
    html.match_indices(open).filter_map(|(start, _)| {
        let end = html[start..].find('>')?;
        Some(&html[start..=start + end])
    })
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let length = tag[start..].find('"')?;
    Some(unescape(&tag[start..start + length]))
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><body><form id="download-form" action="https://drive.usercontent.google.com/download" method="get"><input type="hidden" name="id" value="abc123"><input type="hidden" name="export" value="download"><input type="hidden" name="confirm" value="t"><input type="hidden" name="uuid" value="u-1&amp;x"></form></body></html>"#;

    #[test]
    fn reads_form_inputs() {
        let url = confirm_url(PAGE).expect("url");
        assert_eq!(url.host_str(), Some("drive.usercontent.google.com"));
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("id".to_owned(), "abc123".to_owned()),
                ("export".to_owned(), "download".to_owned()),
                ("confirm".to_owned(), "t".to_owned()),
                ("uuid".to_owned(), "u-1&x".to_owned()),
            ]
        );
    }

    #[test]
    fn ignores_other_forms() {
        let html = r#"<form id="search" action="https://example.com/"></form>"#;
        assert_eq!(confirm_url(html), None);
        assert_eq!(confirm_url("plain text"), None);
    }

    #[test]
    fn fallback_has_confirm() {
        let url = fallback_url("abc123").expect("url");
        assert_eq!(
            url.as_str(),
            "https://drive.usercontent.google.com/download?id=abc123&export=download&confirm=t"
        );
    }
}
