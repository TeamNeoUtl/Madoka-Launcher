use launcher_core::{Edition, Host, Kind};

pub const APP_TITLE: &str = "Madoka Launcher";

pub const TAB_INSTANCES: &str = "インスタンス";
pub const TAB_NEW: &str = "新規作成";
pub const TAB_TASKS: &str = "タスク";
pub const TAB_SETTINGS: &str = "設定";
pub const DARK_MODE: &str = "ダークモード";

pub const ADD_INSTANCE: &str = "インスタンス追加";
pub const REFRESH: &str = "更新";
pub const LAUNCH: &str = "起動";
pub const OPEN_FOLDER: &str = "フォルダを開く";
pub const DELETE: &str = "削除";
pub const EDIT: &str = "編集";
pub const DUPLICATE: &str = "複製";
pub const CLOSE: &str = "閉じる";
pub const BACK: &str = "戻る";
pub const SAVE: &str = "保存";
pub const DELETE_CONFIRM: &str = "削除を確定";
pub const CREATE: &str = "作成";
pub const CANCEL: &str = "取消";

pub const NO_INSTANCES: &str = "インスタンスなし";
pub const NO_SELECTION: &str = "インスタンス未選択";
pub const NO_TASKS: &str = "タスクなし";
pub const UNGROUPED: &str = "未分類";

pub const KIND: &str = "種別";
pub const EDITION: &str = "エディション";
pub const VERSION: &str = "本体バージョン";
pub const ADDON_VERSION: &str = "拡張編集バージョン";
pub const GROUP: &str = "グループ";
pub const PLUGINS: &str = "プラグイン";
pub const NAME: &str = "名前";
pub const GROUP_HINT: &str = "空欄: 未分類";

pub const LOADING: &str = "取得中";
pub const LOAD_FAILED: &str = "取得失敗";
pub const RELOAD: &str = "再取得";
pub const NO_VERSION: &str = "導入可能なバージョンなし";
pub const PRERELEASE: &str = "プレリリース";

pub const PLANNED: &str = "準備中";

pub const EDIT_TAB_VERSION: &str = "バージョン";
pub const EDIT_TAB_PLUGINS: &str = "プラグイン";
pub const EDIT_TAB_BROWSE: &str = "ブラウズ";
pub const EDIT_TAB_SETTINGS: &str = "設定";
pub const SEARCH: &str = "検索";
pub const SEARCH_HINT: &str = "名前・説明・開発者";
pub const ALL_CATEGORIES: &str = "すべて";
pub const NO_PLUGINS: &str = "導入済みプラグインなし";
pub const NO_MATCH: &str = "該当なし";
pub const NO_CATALOG: &str = "この種別にプラグイン提供元なし";
pub const INSTALL: &str = "導入";
pub const UPDATE: &str = "更新";
pub const OPEN_IN_BROWSER: &str = "ブラウザで開く";
pub const INSTALLED: &str = "導入済み";
pub const UPDATE_AVAILABLE: &str = "更新あり";
pub const NOT_INSTALLED: &str = "未導入";
pub const UNVERIFIED_WARNING: &str = "ハッシュ検証なし。導入を続行するか確認";
pub const CONTINUE: &str = "続行";
pub const FILES: &str = "ファイル数";

pub const DATA_DIRECTORY: &str = "データディレクトリ";
pub const GITHUB_TOKEN: &str = "GitHub トークン";
pub const GITHUB_TOKEN_HINT: &str = "空欄: 未認証 (60 req/h)";
pub const FONT: &str = "フォント";
pub const FONT_DEFAULT: &str = "システム既定";
pub const FONT_RESTART: &str = "フォントは再起動後に反映";
pub const DNS: &str = "DNS サーバー";
pub const DNS_HINT: &str = "空欄: AdGuard DNS (カンマ区切り)";
pub const DNS_INVALID: &str = "DNS: IP アドレスを解釈できません";
pub const DNS_RESTART: &str = "DNS は再起動後に反映";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LAUNCHER_VERSION: &str = "ランチャーのバージョン";
pub const CHECK_UPDATE: &str = "更新を確認";
pub const CHECKING_UPDATE: &str = "確認中";
pub const LAUNCHER_UP_TO_DATE: &str = "最新";
pub const LAUNCHER_UPDATING: &str = "更新中";
pub const LAUNCHER_UPDATED: &str = "更新済み。再起動後に反映";
pub const LANGUAGE: &str = "言語";
pub const LANGUAGE_VALUE: &str = "日本語";
pub const COPY_SUFFIX: &str = "のコピー";

pub const TASK_CREATE: &str = "作成";
pub const TASK_DELETE: &str = "削除";
pub const TASK_LAUNCH: &str = "起動";
pub const TASK_DUPLICATE: &str = "複製";
pub const TASK_DOWNLOAD: &str = "ダウンロード";
pub const TASK_INSTALL: &str = "導入";
pub const TASK_REMOVE: &str = "プラグイン削除";
pub const TASK_SELF_UPDATE: &str = "ランチャー更新";
pub const ACTIVE_TASKS: &str = "進行中のタスク";
pub const QUEUED: &str = "待機中";
pub const RUNNING: &str = "実行中";
pub const SUCCEEDED: &str = "成功";
pub const FAILED: &str = "失敗";

pub const fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Aviutl => "AviUtl",
        Kind::Aviutl2 => "AviUtl2",
        Kind::Ymm4 => "YMM4",
        Kind::Neoutl => "NeoUtl",
        Kind::Freya => "Freya NLE",
    }
}

pub const fn edition_label(edition: Edition) -> &'static str {
    match edition {
        Edition::Normal => "通常版",
        Edition::Lite => "Lite 版",
    }
}

pub const fn host_label(host: Host) -> &'static str {
    match host {
        Host::Windows => "Windows",
        Host::Linux => "Linux",
    }
}

pub fn unsupported_host_badge(host: Host) -> String {
    format!("{} 未対応", host_label(host))
}
