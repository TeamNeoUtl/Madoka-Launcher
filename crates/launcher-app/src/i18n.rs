//! Locale selection. Translations live in `locales/<code>.yml` and load through `rust-i18n`.
//! `ja` is the fallback of every missing key (see `rust_i18n::i18n!` in `main.rs`).

use serde::{Deserialize, Serialize};

/// Supported locales. `as_str` equals the stem of the file in `locales/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    Ja,
    En,
    Ko,
    ZhCn,
    ZhTw,
}

impl Locale {
    pub const ALL: [Self; 5] = [Self::Ja, Self::En, Self::Ko, Self::ZhCn, Self::ZhTw];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Ja => "日本語",
            Self::En => "English",
            Self::Ko => "한국어",
            Self::ZhCn => "简体中文",
            Self::ZhTw => "繁體中文",
        }
    }

    /// Converts a locale string (as stored in `config.toml`) to a `Locale`.
    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|locale| locale.as_str() == s)
    }

    /// Returns the config string representation of this locale.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
            Self::Ko => "ko",
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
        }
    }

    /// Makes `t!` return strings of this locale.
    pub fn apply(self) {
        rust_i18n::set_locale(self.as_str());
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
