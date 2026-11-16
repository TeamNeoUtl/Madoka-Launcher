use launcher_core::Kind;
use launcher_providers::{Http, HttpError, MadokaProvider, Provider, has_provider};
use url::Url;

struct NoHttp;

impl Http for NoHttp {
    fn get(&self, url: &Url) -> impl Future<Output = Result<Vec<u8>, HttpError>> + Send {
        std::future::ready(Err(HttpError::Transport {
            url: url.to_string(),
            message: "unused".to_owned(),
        }))
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
    for kind in ALL_KINDS {
        let provider = MadokaProvider::for_kind(kind, NoHttp).expect("provider exists");
        assert_eq!(provider.kind(), kind);
    }
}

#[test]
fn has_provider_agrees_with_for_kind() {
    for kind in ALL_KINDS {
        assert_eq!(
            has_provider(kind),
            MadokaProvider::for_kind(kind, NoHttp).is_ok()
        );
    }
}
