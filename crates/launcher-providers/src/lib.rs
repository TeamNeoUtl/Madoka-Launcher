pub mod au2pkg;
mod aviutl;
mod aviutl2;
mod cache;
mod dns;
mod drive;
mod error;
mod fetch;
mod freya;
mod history;
mod http;
mod neoutl;
mod provider;
mod recipe;
mod registry;
mod release;
mod updater;
mod ymm4;

pub use aviutl::{AviutlProvider, parse_apm_packages, parse_aviutl_addons, parse_aviutl_apps};
pub use aviutl2::{
    Aviutl2Provider, parse_aviutl2_app, parse_aviutl2_catalog, parse_aviutl2_history,
};
pub use cache::{CACHE_TTL, load_any, load_fresh, store as store_catalog};
pub use dns::{ADGUARD_DNS, Dns};
pub use error::ProviderError;
pub use fetch::ReqwestFetcher;
pub use freya::FreyaProvider;
pub use http::{Http, HttpError, ReqwestHttp};
pub use neoutl::NeoutlProvider;
pub use provider::{Provider, Versions};
pub use registry::{MadokaProvider, has_catalog, has_provider};
pub use updater::SelfUpdater;
pub use ymm4::{Ymm4Provider, parse_ymm4_catalog};
