use launcher_core::{Artifact, InstallPlan, Package, Recipe, StagedPlan, is_allowed_url};
use percent_encoding::percent_decode_str;
use url::Url;

const FALLBACK_NAME: &str = "download";

/// Artifact for a URL that names the file in its last path segment.
pub(crate) fn direct_artifact(url: &Url) -> Artifact {
    let segment = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or_default();
    let decoded = percent_decode_str(segment).decode_utf8_lossy().into_owned();
    let name = if decoded.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        decoded
    };
    Artifact {
        name,
        url: url.clone(),
        size: None,
        hash: None,
    }
}

/// Builds the plan of `recipe` for the resolved `artifact`.
/// The browser opens instead when the host is not allowed or an `http` URL has no hash.
pub(crate) fn finish(package: &Package, recipe: &Recipe, mut artifact: Artifact) -> InstallPlan {
    artifact.hash = recipe.hash.clone().or(artifact.hash);
    let plain_http_without_hash = artifact.url.scheme() == "http" && artifact.hash.is_none();
    if !is_allowed_url(&artifact.url) || plain_http_without_hash {
        return InstallPlan::OpenInBrowser(package.page_url.clone());
    }
    InstallPlan::Staged(StagedPlan {
        artifact,
        version: recipe.version.clone(),
        steps: recipe.steps.clone(),
        checks: recipe.checks.clone(),
    })
}
