//! Locale selection. Translations live in `locales/<code>.yml` and load through `rust-i18n`.
//! `ja` is the fallback of every missing key (see `rust_i18n::i18n!` in `main.rs`).

use serde::{Deserialize, Serialize};

/// Supported locales. `as_str` equals the stem of the file in `locales/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
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

    /// Converts an OS locale tag (e.g. `ja-JP`, `zh_TW.UTF-8`, `zh-Hant-HK`) to a `Locale`.
    /// `zh` with the script `Hant` or the region `TW`, `HK`, `MO` selects `ZhTw`; other `zh` selects `ZhCn`.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let mut parts = tag.split(['-', '_', '.', '@']);
        match parts.next()?.to_ascii_lowercase().as_str() {
            "ja" => Some(Self::Ja),
            "en" => Some(Self::En),
            "ko" => Some(Self::Ko),
            "zh" => {
                let traditional = parts.any(|part| {
                    ["hant", "tw", "hk", "mo"]
                        .iter()
                        .any(|marker| part.eq_ignore_ascii_case(marker))
                });
                Some(if traditional { Self::ZhTw } else { Self::ZhCn })
            }
            _ => None,
        }
    }

    /// Returns the OS locale. `En` is returned when the OS locale is unavailable or unsupported.
    pub fn detect() -> Self {
        sys_locale::get_locale()
            .as_deref()
            .and_then(Self::from_tag)
            .unwrap_or(Self::En)
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

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn from_tag_maps_supported_languages() {
        let cases = [
            ("ja-JP", Some(Locale::Ja)),
            ("ja_JP.UTF-8", Some(Locale::Ja)),
            ("en-US", Some(Locale::En)),
            ("ko-KR", Some(Locale::Ko)),
            ("zh-CN", Some(Locale::ZhCn)),
            ("zh-Hans-CN", Some(Locale::ZhCn)),
            ("zh", Some(Locale::ZhCn)),
            ("zh-TW", Some(Locale::ZhTw)),
            ("zh_HK.UTF-8", Some(Locale::ZhTw)),
            ("zh-Hant", Some(Locale::ZhTw)),
            ("fr-FR", None),
            ("C", None),
            ("", None),
        ];
        for (tag, expected) in cases {
            assert_eq!(Locale::from_tag(tag), expected, "{tag}");
        }
    }

    #[test]
    fn as_str_round_trips_through_from_str() {
        for locale in Locale::ALL {
            assert_eq!(Locale::from_str(locale.as_str()), Some(locale));
        }
    }
}
