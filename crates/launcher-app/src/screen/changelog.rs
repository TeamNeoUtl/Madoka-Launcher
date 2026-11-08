//! Splits a Markdown changelog into versions and extracts the range that an update brings.

/// One version section: the version in its heading and the Markdown of the whole section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section<'a> {
    pub version: Option<String>,
    pub text: &'a str,
}

/// Version found in a heading line, e.g. `## [1.2.3] - 2024-01-01` or `### v1.2`.
fn heading_version(line: &str) -> Option<Option<String>> {
    let rest = line.trim_start();
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    if !(1..=4).contains(&hashes) {
        return None;
    }
    let title = rest[hashes..].trim();
    if title.is_empty() {
        return None;
    }
    let version = title
        .split(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '(' | ')' | ':' | ','))
        .map(|word| word.trim_start_matches(['v', 'V']))
        .find(|word| {
            word.chars().next().is_some_and(|c| c.is_ascii_digit())
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                && !word.chars().all(|c| c.is_ascii_digit() && word.len() == 4)
        })
        .map(str::to_owned);
    Some(version)
}

/// Sections that start at a heading with a version. Text before the first one is dropped.
pub fn split(changelog: &str) -> Vec<Section<'_>> {
    let mut starts: Vec<(usize, String)> = Vec::new();
    let mut offset = 0;
    let mut fenced = false;
    for line in changelog.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && let Some(Some(version)) = heading_version(line) {
            starts.push((offset, version));
        }
        offset += line.len();
    }
    let mut sections = Vec::with_capacity(starts.len());
    for (index, (start, version)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(changelog.len(), |next| next.0);
        sections.push(Section {
            version: Some(version.clone()),
            text: &changelog[*start..end],
        });
    }
    sections
}

fn normalize(version: &str) -> &str {
    version.trim().trim_start_matches(['v', 'V'])
}

/// Compares dotted versions numerically where parts are numbers.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| -> Vec<u64> {
        normalize(v)
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a).cmp(&parts(b))
}

/// Sections whose version is newer than `installed`.
/// `None` when the changelog has no version sections.
pub fn since<'a>(changelog: &'a str, installed: &str) -> Option<Vec<Section<'a>>> {
    let sections = split(changelog);
    if sections.is_empty() {
        return None;
    }
    Some(
        sections
            .into_iter()
            .filter(|section| {
                section.version.as_deref().is_some_and(|version| {
                    compare(version, installed) == std::cmp::Ordering::Greater
                })
            })
            .collect(),
    )
}

/// The changelog text of the versions newer than `installed`.
/// The whole `changelog` when it has no version sections.
pub fn range_text(changelog: &str, installed: &str) -> String {
    match since(changelog, installed) {
        Some(sections) => sections.iter().map(|section| section.text).collect(),
        None => changelog.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str =
        "# Changelog\n\n## [1.3.0] - 2024-03-01\n- c\n\n## v1.2.0\n- b\n\n## 1.0.0 (2023)\n- a\n";

    #[test]
    fn splits_per_version() {
        let sections = split(LOG);
        let versions: Vec<_> = sections.iter().map(|s| s.version.as_deref()).collect();
        assert_eq!(versions, [Some("1.3.0"), Some("1.2.0"), Some("1.0.0")]);
        assert!(sections[1].text.contains("- b"));
        assert!(!sections[1].text.contains("- c"));
    }

    #[test]
    fn extracts_range_after_installed() {
        let text = range_text(LOG, "1.2.0");
        assert!(text.contains("- c"));
        assert!(!text.contains("- b"));
        assert!(range_text(LOG, "v1.0.0").contains("- b"));
    }

    #[test]
    fn plain_text_is_returned_whole() {
        assert_eq!(range_text("fixed bugs", "1.0"), "fixed bugs");
    }
}
