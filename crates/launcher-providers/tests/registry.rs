use launcher_core::Kind;
use launcher_providers::{AnyProvider, Http, HttpError, Provider, ProviderError, has_provider};
use url::Url;

struct NoHttp;

impl Http for NoHttp {
    async fn get(&self, url: &Url) -> Result<Vec<u8>, HttpError> {
        Err(HttpError::Transport {
            url: url.to_string(),
            message: "unused".to_owned(),
        })
    }
}

const ALL_KINDS: [Kind; 5] = [
    Kind::Aviutl,
    Kind::Aviutl2,
    Kind::Ymm4,
    Kind::Neoutl,
    Kind::Freya,
];

#[test]
fn provider_kind_matches_requested_kind() {
    for kind in [Kind::Ymm4, Kind::Neoutl, Kind::Freya] {
        let provider = AnyProvider::for_kind(kind, NoHttp).expect("provider exists");
        assert_eq!(provider.kind(), kind);
    }
}

#[test]
fn aviutl_kinds_have_no_provider() {
    for kind in [Kind::Aviutl, Kind::Aviutl2] {
        let result = AnyProvider::for_kind(kind, NoHttp);
        assert!(matches!(result, Err(ProviderError::NoProvider(k)) if k == kind));
    }
}

#[test]
fn has_provider_agrees_with_for_kind() {
    for kind in ALL_KINDS {
        assert_eq!(
            has_provider(kind),
            AnyProvider::for_kind(kind, NoHttp).is_ok()
        );
    }
}
