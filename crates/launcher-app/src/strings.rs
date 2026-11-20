use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::{Mutex, PoisonError};

use launcher_core::{Edition, Host, Kind};
use rust_i18n::t;

use crate::i18n::Locale;

/// Window title. It does not depend on the locale.
pub const APP_TITLE: &str = "Madoka Launcher";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Plugins that "recommended plugins" installs. They equal `requiredPluginIds` of the upstream `setup.json`.
/// The launcher installs without a network round trip to `setup.json`, so the IDs live here only.
pub const RECOMMENDED_PLUGIN_IDS: [&str; 2] = ["rigaya.x264guiEx", "Mr-Ojii.L-SMASH-Works"];
pub const RELEASES_URL: &str = "https://github.com/TeamNeoUtl/Madoka-launcher/releases/latest";
pub const COMMONS_GUIDE_URL: &str = "https://qa.nicovideo.jp/faq/show/847?site_domain=default";

/// Makes `t!` return strings of `locale`.
pub fn set_locale(locale: Locale) {
    locale.apply();
}

/// Returns the translation of `key` for widgets that borrow their label for `'static`.
/// Each (locale, key) pair allocates once and stays for the process lifetime.
/// The pair count is bounded by the keys in `locales/` times `Locale::ALL`.
pub fn label(key: &'static str) -> &'static str {
    static CACHE: Mutex<BTreeMap<(String, &'static str), &'static str>> =
        Mutex::new(BTreeMap::new());
    let locale = rust_i18n::locale().to_string();
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    match cache.entry((locale, key)) {
        Entry::Occupied(entry) => entry.get(),
        Entry::Vacant(entry) => entry.insert(Box::leak(t!(key).into_owned().into_boxed_str())),
    }
}

pub const fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Aviutl => "AviUtl",
        Kind::Aviutl2 => "AviUtl2",
        Kind::Ymm4 => "YMM4",
        Kind::Neoutl => "NeoUtl",
        Kind::Freya => "Freya NLE",
        Kind::Macutl => "MacUtl",
    }
}

pub fn edition_label(edition: Edition) -> Cow<'static, str> {
    match edition {
        Edition::Normal => t!("edition_normal"),
        Edition::Lite => t!("edition_lite"),
    }
}

pub const fn host_label(host: Host) -> &'static str {
    match host {
        Host::Windows => "Windows",
        Host::Linux => "Linux",
        Host::MacOs => "macOS",
    }
}

pub fn unsupported_host_badge(host: Host) -> String {
    t!("unsupported_host", host = host_label(host)).into_owned()
}
