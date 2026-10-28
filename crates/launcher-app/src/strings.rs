use launcher_core::{Edition, Kind};

pub const APP_TITLE: &str = "MLAYN Launcher";

pub const ADD_INSTANCE: &str = "インスタンス追加";
pub const REFRESH: &str = "更新";
pub const TASKS: &str = "タスク";
pub const BACK: &str = "戻る";
pub const LAUNCH: &str = "起動";
pub const OPEN_FOLDER: &str = "フォルダを開く";
pub const DELETE: &str = "削除";
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
pub const GROUP: &str = "グループ";
pub const PLUGINS: &str = "プラグイン";
pub const NAME: &str = "名前";
pub const NAME_HINT: &str = "インスタンス名";
pub const GROUP_HINT: &str = "空欄: 未分類";

pub const LOADING: &str = "取得中";
pub const LOAD_FAILED: &str = "取得失敗";
pub const RELOAD: &str = "再取得";
pub const NO_VERSION: &str = "導入可能なバージョンなし";
pub const PRERELEASE: &str = "プレリリース";

pub const UNSUPPORTED_HOST: &str = "この OS では動作しません";
pub const NO_PROVIDER: &str = "このバージョンでは未対応です";

pub const TASK_CREATE: &str = "作成";
pub const TASK_DELETE: &str = "削除";
pub const TASK_LAUNCH: &str = "起動";
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
