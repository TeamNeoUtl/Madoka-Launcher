# MLAYN-Launcher 実装計画

## 1. 目的

- 対象: AviUtl 無印 / AviUtl2 / YMM4 / NeoUtl / Freya NLE のマルチインスタンス管理。
- 機能: インスタンス作成・起動・削除、プラグイン一覧取得、プラグイン導入・更新・削除。
- 構成: Rust + Iced。Windows / Linux。macOS は対象外。
- 構造の基準: Prism Launcher (Instance / Instance Group / Meta / Download Task)。

## 2. 入力 (詳細: `docs/inputs.md`)

リポジトリ・索引はランチャーに同梱しない。実行時に取得する。

| Kind | 本体の取得元 | プラグイン索引 | 導入形式 |
|---|---|---|---|
| ymm4 | GitHub Releases (`manju-summoner/YukkuriMovieMaker4`)。asset = `^YukkuriMovieMaker_v[0-9.]+(_Lite)?\.zip$`。sha256 は API の `digest` | `https://manjubox.net/ymm4plugins.yml` (YAML、426 件、有効 412 件) | `url` が GitHub リポジトリ (296 件) の場合、Release の `.ymme` (ZIP、階層なし) を `user/plugin/<repo>/` へ全件展開。他は既定ブラウザで開く |
| aviutl2 | 公式のポータブル ZIP (配布 URL・展開後の構造は Phase 5 で確認) | `https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json` (445 件) | `installer.install` のアクション列 |
| aviutl | APM `v3/core.json` (`aviutl` と `exedit`。URL は `http`、sha384 必須) | `https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/` (285 件) | `directURL` (2 件) または GitHub `releases` URL (78 件) の ZIP を展開し、`files[].filename` へ配置。sha384 検証。他 205 件は既定ブラウザで開く |
| neoutl | Codeberg Releases (`TeamNeoUtl/NeoUtl`)。`neoutl-{linux,windows}-x86_64.zip` | なし | ZIP 展開 |
| freya | GitHub Releases (`GT-610/freya-nle`)。asset 規則は Phase 2 で確定 | なし | ZIP 展開 (形式は未確認) |

未確認の項目は `docs/inputs.md` §6。

## 3. 用語

| 用語 | 定義 |
|---|---|
| Instance | 1 つのエディタ本体 + 専用プラグイン + 専用設定のディレクトリ |
| Kind | `aviutl` / `aviutl2` / `ymm4` / `neoutl` / `freya` |
| Provider | Kind ごとの本体取得・プラグイン一覧取得・導入処理の実装 |
| Edition | ymm4 の `normal` / `lite`。他の Kind は `normal` 固定 |
| Package | プラグイン 1 件のメタデータ |
| Artifact | ダウンロード対象のファイル 1 件 |

## 4. ディレクトリ構造

### 4.1 ランチャーのデータ

```
<data_dir>/MLAYN-Launcher/
  config.toml
  cache/
    catalog/<kind>.json
    downloads/<sha256>
  instances/
    <instance_id>/
      instance.toml
      app/            # 本体。プラグインの配置先は Kind ごとに §5.5 と inputs.md に定義
```

`<data_dir>` は `directories::ProjectDirs` で決定する。

### 4.2 instance.toml

```toml
id = "uuid-v4"
name = "string"
kind = "ymm4"
edition = "normal"
app_version = "v4.56.1.0"
group = "string"          # 任意。省略時は未分類
created_at = "RFC3339"

[[plugin]]
id = "string"
version = "string"
source = "github:owner/repo"
files = ["relative/path"]
```

`files` は導入時に書き込んだ相対パスの全件。削除時に使用する。

## 5. アーキテクチャ

### 5.1 Workspace

```
Cargo.toml                      # workspace
crates/
  launcher-core/                # Iced 非依存。モデル、永続化、Installer、Fetcher trait、タスク
  launcher-providers/           # Provider 実装 4 種
  launcher-app/                 # Iced UI。bin
```

依存方向: `app → providers → core`。`core` はネットワーク型を持たない。

### 5.2 Provider trait

```rust
pub trait Provider: Send + Sync {
    fn kind(&self) -> Kind;
    async fn app_versions(&self) -> Result<Vec<AppVersion>, Error>;
    async fn catalog(&self) -> Result<Vec<Package>, Error>;
    async fn plan_install_app(&self, v: &AppVersion, host: Host) -> Result<Vec<Artifact>, Error>;
    async fn plan_install_package(&self, p: &Package) -> Result<InstallPlan, Error>;
}
```

`InstallPlan` は `Download(Vec<Artifact>)` と `OpenInBrowser(Url)` の 2 値。
導入処理は `core` の `Installer` が実行する。Provider は計画のみ返す。
`core` は `Fetcher` trait (URL → ファイル書込) を定義し、`launcher-providers` の `ReqwestFetcher` が実装する。

| Kind | app_versions | catalog |
|---|---|---|
| aviutl | APM `v3/core.json` | APM `v3/packages/*.json` |
| aviutl2 | 公式のポータブル ZIP (Phase 5 で確認) | `index.json` |
| ymm4 | GitHub Releases API。`edition` で asset を選択 | `ymm4plugins.yml` + GitHub Releases API |
| neoutl | Codeberg (Forgejo) Releases API。`prerelease = false` の最新 | なし (空配列) |
| freya | GitHub Releases API。`prerelease` を含む最新 (UI に区分を表示) | なし (空配列) |

### 5.3 起動層 (Runner) とホスト対応

Wine を使用しない。すべて直接 spawn。

| Kind | Windows | Linux |
|---|---|---|
| aviutl | 対応 | 非対応 |
| aviutl2 | 対応 | 非対応 |
| ymm4 | 対応 | 非対応 |
| neoutl | 対応 | 対応 |
| freya | 対応 | 対応 |

- 非対応の Kind は New Instance 画面で選択不可。理由を 1 行表示する。
- Freya NLE は GPU アダプタを必須とする。起動失敗時にログを表示する。
- Runner (`launcher_core::Launched`): `<instance>/app/` の深さ 2 までを探索し、Kind ごとの実行ファイル名 (大文字小文字無視) に一致する最初のファイルを `current_dir` を親ディレクトリに設定して spawn する。stdout / stderr は `<instance>/launch.log` へ書き込む。起動後 5 秒以内に失敗ステータスで終了した場合は `Failed` とし、ログ末尾 4096 byte を理由に含める。
- 実行権限 (Unix): `Archive::extract` は zip の実行ビットを持つエントリを `0o755`、それ以外を `0o644` で作成する。Runner は spawn 前に、実行ビットを持たない実行ファイルへ `mode | 0o111` を設定する。

### 5.4 タスク

- すべてのネットワーク処理・ファイル展開を `Task` として実行する。
- `Task` の状態: `Queued` / `Running{done,total,transfer}` / `Succeeded` / `Failed{reason}`。
- UI は `iced::Subscription` で状態を受信する。
- 同一 Instance への書き込みタスク (作成・削除・起動前の読込) は `InstanceLocks` (Instance 単位の `tokio::sync::Mutex`) で直列化する。`Installer` は排他制御を持たない。呼び出し側が `InstanceLocks::lock` を取得する。
- `Running{done,total}` の単位は手順数 (`u16`)。`transfer` は現在の手順のダウンロード進捗 (`Progress{done,total}` のバイト数。`total` は未報告の場合 `None`)。`Fetcher::fetch` は `&dyn ProgressSink` を受け取り、応答ヘッダ受信後と各チャンク受信後に報告する。`TaskHandle` が `ProgressSink` を実装する。状態は `TaskBoard` (共有リスト) に保持し、UI は進行中タスクがある間だけ 250 ms 周期の `Subscription` で再描画する。

### 5.5 YMM4 索引のパース

- 入力: `ymm4plugins.yml`。最上位は list。
- 必須キー: `type` `date` `name` `url` `author` `authorUrl` `description`。任意: `links` `isEnabled`。
- `type` はカンマ区切りで複数値を持つ。`Vec<Category>` に分割する。
- `isEnabled = false` のエントリは非表示。
- 導入可能判定: `url` が `https://github.com/<owner>/<repo>` (パス 2 要素) の場合のみ `Download`。それ以外は `OpenInBrowser`。
- `.ymme` の展開: `<instance>/app/user/plugin/<repo>/` を作成し、全エントリを直下へ展開する。エントリ名に `/` `\` `..` を含む場合は `Failed`。展開した相対パスを `files` に記録する。
- 破損検出: 件数 0、または必須キー欠落が 1 件以上の場合は `Err(CatalogShapeChanged)`。キャッシュを上書きしない。

## 6. 安全性

- `#![forbid(unsafe_code)]` を全 crate に設定する。unsafe は 0 件。
- zip 展開: 各エントリの正規化後パスが展開先ディレクトリの配下であることを検証する。違反時はタスク全体を `Failed` にし、書き込み済みファイルを削除する。
- ハッシュ: GitHub asset は `digest` (sha256)、APM は sha384、AviUtl2 索引は XXH3-128 を検証する。APM の `core.json` は URL が `http` のため、sha384 検証を必須とし、不一致は `Failed`。ハッシュを持たない項目は導入前に警告を 1 行表示する。提供しない場合は取得後に計算して `instance.toml` に記録しない (記録先が `files` のみのため)。
- ダウンロード先ホスト許可リスト (`launcher_core::ALLOWED_HOSTS`、`https` のみ): `github.com`, `api.github.com`, `codeberg.org`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com`。リダイレクト先も `ReqwestFetcher` が同じ関数で検証する。`spring-fragrance.mints.ne.jp` (`http`) とカタログ Provider が指定するホストは Phase 5 で追加する。
- AviUtl2 本体: セットアップ実行ファイルを使用しない。ポータブル ZIP を `<instance>/app/` へ展開する。インスタンス間でファイル・設定・データを共有しない。
- 索引の `run` アクション: 実行前に `path` と `args` を表示して確認を取る。`elevate = true` はランチャーでは昇格せず、確認ダイアログに表示する。
- HTTP: `reqwest` + `rustls`。User-Agent は `MLAYN-Launcher/<version>`。GitHub API は未認証 60 req/h。ETag で条件付き取得し、`config.toml` の任意トークンで上限を引き上げる。
- タイムアウト: 接続 15 秒。API 取得は全体 30 秒。本体ダウンロードは無通信 30 秒。超過は `Failed` とし、原因 (エラーの連鎖) を理由に含める。
- ログ (`tracing`): 出力先は標準エラーと `<data_dir>/logs/launcher.log`。起動ごとに前回分を `launcher.previous.log` へ退避する。既定フィルタは `warn` + 本プロジェクトの crate と `reqwest` / `hyper_util` が `debug`。環境変数 `MLAYN_LOG` で上書きする。記録対象: 起動時のデータディレクトリ、通信 (URL・ステータス・バイト数・経過時間・リダイレクト・8 MiB ごとの進捗)、Installer の各手順、Instance の作成・削除、ロック待ち時間、タスク状態の遷移、起動したプロセスの pid と終了状態、パニック。

## 7. UI (Iced)

### 7.1 画面 (Prism Launcher の構成)

| 画面 | 内容 |
|---|---|
| 共通 | メニューバー (設定 > テーマ。`iced::Theme` の全組込テーマを系統別に表示。選択は `config.toml` の `theme` に保存)。タブバー (インスタンス / 新規作成 / タスク。進行中タスク数を表示) |
| Main | ツールバー (インスタンス追加 / 更新)。Group 見出し付きのインスタンスグリッド (アイコン + 名前)。右サイドバー (選択中のインスタンスの 起動 / 編集 / フォルダを開く / 削除 / 複製)。右クリックメニュー。ステータスバー (進行中タスク) |
| New Instance | 左のリスト: Kind (YMM4 / AviUtl / AviUtl2 / NeoUtl / Freya NLE)。右: 名前・Group・本体バージョン (YMM4 は Edition)。非対応 Kind は選択不可で理由を表示 |
| Edit Instance | 左のタブ: バージョン / プラグイン / 設定。プラグインタブ: 導入済み一覧 + ダウンロード (カタログ検索) |
| Tasks | 進行中・完了タスク一覧 |
| Settings | データディレクトリ、GitHub トークン、言語 |

### 7.2 状態管理

- `App` 1 つの `State`。`Message` enum は画面ごとにネスト (`Message::Main(..)`, `Message::Instance(..)`)。
- 副作用はすべて `Task` / `Subscription` 経由。`update` 内でブロッキング処理を行わない。

### 7.2.1 見た目

- 角丸は 16 px (カード、ボタン、入力欄、進捗バー)。メニュー項目は 8 px。
- 区画はカード (背景: テーマの `background.weak`、枠: `background.strong`) で分ける。インスタンス、サイドバー、フォーム、タスクはカード。
- バッジ (`iced_aw::Badge`): Kind、未対応 OS (`<OS> 未対応`)、提供元の準備中 (`準備中`)、タスクの状態、プレリリース。
- `iced_aw` の使用範囲: `TabBar` / `MenuBar` / `Badge` (feature: `tab_bar` `menu` `badge`)。

### 7.3 カタログ表示

- 検索: 名前・説明・開発者の部分一致 (大文字小文字無視)。
- フィルタ: カテゴリ (YMM4 の 18 分類)。
- 行表示: 名前、開発者、説明 1 行、導入状態 (`導入済み` / `更新あり` / `未導入`)、操作ボタン (`導入` / `ブラウザで開く`)。
- 仮想化: 全件を `scrollable` + 可視範囲のみ生成で描画する。

## 8. フェーズ

| Phase | 内容 | 完了条件 |
|---|---|---|
| 0 | 入力調査 | 完了。`docs/inputs.md` に記載。未確認 8 件は `docs/inputs.md` §8 |
| 1 | `launcher-core`: モデル、`instance.toml` 読み書き、Instance CRUD | 完了。`cargo test` 8 件成功 (作成→保存→読込→削除、パス検証、一覧、Kind×Host) |
| 2 | Provider: YMM4 + NeoUtl + Freya NLE (本体取得とカタログ) | `cargo test` で実ネットワークを使わずに、索引のパース規則 (§5.5) と asset 選択規則を検証して成功 |
| 3 | Installer: ダウンロード、展開、ファイル記録、削除 | 完了。`launcher-core` の `cargo test` 23 件成功 (Installer 15 件: zip-slip は `Failed` かつ書込 0 件、正常 fixture は `files` 全件記録、書込失敗時のロールバック、再導入時の旧ファイル削除、ハッシュ不一致、許可リスト外ホスト、キャッシュ再利用) |
| 4 | UI: メニューバー (テーマ) / タブバー / Main (Group 別カードグリッド、サイドバー: 起動 / フォルダを開く / 削除、ステータスバー) / New Instance (未対応バッジ) / Tasks (バイト進捗) | 完了 (ユーザー判断)。`clippy --all-targets -D warnings` 警告 0 件、`cargo test` 成功 (core 34 件、providers 22 件)。Linux (NeoUtl): 作成は成功 (ZIP 約 114 MB の取得 74 秒、展開 1 秒)。起動は実行権限なしで `Permission denied` となり Phase 6 の範囲。実行権限の付与、バイト進捗、iced_aw による UI 刷新を追加 (ローカル検証待ち)。Windows 実機での作成・起動、YMM4 / Freya NLE の確認は未実施 |
| 5 | UI: Browse / Plugins / Edit Instance / Settings、Main の複製・右クリックメニュー、Provider: AviUtl2 + AviUtl 無印 | 4 Kind すべてでカタログ表示とプラグイン導入が成功 |
| 6 | Linux 対応 | Linux 上で NeoUtl と Freya NLE のインスタンスを作成して起動できる |
| 7 | 配布: GitHub Actions で 2 OS ビルド、リリース成果物生成 | tag push で Windows と Linux の成果物が Release に添付される |

各 Phase は 1 PR。Phase 完了時点で `main` はビルドとテストが成功している状態を保つ。

## 9. テスト

| 種別 | 対象 | 方法 |
|---|---|---|
| 単体 | モデル、パス検証、バージョン比較 | `cargo test` |
| 規則 | `ymm4plugins.yml` / `index.json` / APM `packages/*.json` のパース、asset 選択、導入可否判定 | テスト内で生成した最小の入力を検証。実データのスナップショット・件数・特定の値は検証しない |
| 統合 | Installer | `tempfile` 上で zip を展開、`files` と実体を照合 |
| 実データ | 索引・Release API の形状 | `#[ignore]` のネットワークテスト。件数ではなく不変条件 (ID の一意、必須項目の存在、各ホストで導入可能な版の存在) を検証 |
| CI | fmt / clippy / test | `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` を Windows と Linux で実行 |

ネットワークを使うテストは `#[ignore]` を付け、CI の定期ジョブでのみ実行する。

## 10. 依存クレート

| 用途 | クレート |
|---|---|
| UI | iced, iced_aw |
| 非同期 | tokio |
| HTTP | reqwest (rustls) |
| 直列化 | serde, serde_json, toml |
| YAML | serde_yaml_ng |
| 正規表現 | regex |
| ハッシュ (XXH3-128) | xxhash-rust |
| 7z 展開 | sevenz-rust2 |
| 展開 | zip |
| ハッシュ (sha256/384/512) | sha2 |
| パス | directories |
| エラー | thiserror |
| ログ | tracing, tracing-subscriber |
| ID | uuid |
| 版比較 | semver |
| 時刻 (RFC3339) | time |
| 一時領域 (test) | tempfile |

## 11. 未決事項

| # | 内容 | 既定値 |
|---|---|---|
| 1 | Wine | 使用しない (決定済) |
| 2 | BOOTH / Google Drive / note 配布プラグインの扱い | ブラウザで開く。自動導入しない |
| 3 | カタログ更新間隔 | 起動時 + 手動更新。キャッシュ有効期間 24 時間 |
| 4 | 言語 | 日本語のみ。文字列は定数モジュールに集約 |
| 5 | APM データ (CC BY-NC-SA 4.0)、NeoUtl (AGPL)、各索引の同梱 | 同梱しない。実行時に取得 (決定済) |
| 6 | macOS | 対象外 (決定済) |
| 7 | AviUtl2 のインスタンス分離 | `instances/<id>/` 内で完全に独立。共有しない (決定済)。ポータブル ZIP のデータ配置は Phase 5 で確認 |
| 8 | ダウンロードのバイト進捗 | 表示する (決定済)。`Fetcher::fetch` が `ProgressSink` へ報告する (§5.4) |
