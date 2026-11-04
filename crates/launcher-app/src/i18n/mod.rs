#![allow(dead_code)]

//! Internationalization (i18n) support for Madoka Launcher.
//!
//! Provides a `Locale` enum and a `t()` function that returns the
//! localized string for a given key.

use serde::{Deserialize, Serialize};

mod en;
mod ja;
mod ko;
mod zh_cn;
mod zh_tw;

/// Supported locales.
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
    pub const ALL: [Locale; 5] = [
        Locale::Ja,
        Locale::En,
        Locale::Ko,
        Locale::ZhCn,
        Locale::ZhTw,
    ];

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
        match s {
            "ja" => Some(Self::Ja),
            "en" => Some(Self::En),
            "ko" => Some(Self::Ko),
            "zh-CN" => Some(Self::ZhCn),
            "zh-TW" => Some(Self::ZhTw),
            _ => None,
        }
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
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Returns the localized string for `key` in `locale`.
pub fn t(locale: Locale, key: Key) -> &'static str {
    match locale {
        Locale::Ja => ja::get(key),
        Locale::En => en::get(key),
        Locale::Ko => ko::get(key),
        Locale::ZhCn => zh_cn::get(key),
        Locale::ZhTw => zh_tw::get(key),
    }
}

/// All translatable string keys used in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    AppTitle,
    TabInstances,
    TabNew,
    TabTasks,
    TabSettings,
    DarkMode,
    AddInstance,
    Refresh,
    Launch,
    OpenFolder,
    Delete,
    Edit,
    Duplicate,
    Close,
    Back,
    Save,
    DeleteConfirm,
    Create,
    Cancel,
    NoInstances,
    NoSelection,
    NoTasks,
    Ungrouped,
    Kind,
    Edition,
    Version,
    AddonVersion,
    Group,
    Plugins,
    Name,
    GroupHint,
    Loading,
    LoadFailed,
    Reload,
    NoVersion,
    Prerelease,
    Planned,
    EditTabVersion,
    EditTabPlugins,
    EditTabBrowse,
    EditTabSettings,
    Search,
    SearchHint,
    AllCategories,
    NoPlugins,
    NoMatch,
    NoCatalog,
    Install,
    Update,
    UpdateAll,
    OpenInBrowser,
    Installed,
    UpdateAvailable,
    NotInstalled,
    UnverifiedWarning,
    Continue,
    Files,
    InstanceRunningError,
    RecommendedPlugins,
    EditTabCommons,
    CommonsCopyAll,
    CommonsCopied,
    CommonsGuide,
    CommonsNoIds,
    SelectAll,
    DeselectAll,
    JsonDirectRegister,
    Paused,
    PauseUpdate,
    ResumeUpdate,
    ExportConfig,
    ImportConfig,
    UpdateSummary,
    SortPopularity,
    SortTrend,
    SortNewest,
    SortUpdated,
    StatusAll,
    StatusInstalled,
    StatusNotInstalled,
    DeprecatedWarning,
    DataDirectory,
    GithubToken,
    GithubTokenHint,
    Font,
    FontDefault,
    FontRestart,
    Dns,
    DnsHint,
    DnsInvalid,
    DnsRestart,
    LauncherVersion,
    CheckUpdate,
    CheckingUpdate,
    LauncherUpToDate,
    LauncherUpdating,
    LauncherUpdated,
    Language,
    CopySuffix,
    TaskCreate,
    TaskDelete,
    TaskLaunch,
    TaskDuplicate,
    TaskDownload,
    TaskInstall,
    TaskRemove,
    TaskUpdateAll,
    TaskSelfUpdate,
    ActiveTasks,
    Queued,
    Running,
    Succeeded,
    Failed,
}
