# MLAYN-Launcher 実装計画

## 1. 目的

- 対象: AviUtl 無印 / AviUtl2 / YMM4 / NeoUtl のマルチインスタンス管理。
- 機能: インスタンス作成・起動・削除、プラグイン一覧取得、プラグイン導入・更新・削除。
- 構成: Rust + Iced。Windows / Linux / macOS。
- 構造の基準: Prism Launcher (Instance / Instance Group / Meta / Download Task)。

## 2. 入力 (詳細: `docs/inputs.md`)

リポジトリ・索引はランチャーに同梱しない。実行時に取得する。

| Kind | 本体の取得元 | プラグイン索引 | 導入形式 |
|---|---|---|---|
| ymm4 | GitHub Releases (`manju-summoner/YukkuriMovieMaker4`) | `https://manjubox.net/ymm4plugins.yml` (YAML、426 件、有効 412 件) | `url` が GitHub リポジトリ (296 件) の場合、Release の `.ymme` (ZIP) を `user/plugin/<stem>/` へ展開。他は既定ブラウザで開く |
| aviutl2 | 索引の `Kenkun.AviUtlExEdit2` (type `本体`)。`direct` の setup 実行ファイル | `https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json` (445 件) | `installer.install` のアクション列 |
| aviutl | APM `v3/core.json` | `https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/` | `files[].source → destination` の複写。sha384 検証 |
| neoutl | Codeberg Releases (`TeamNeoUtl/NeoUtl`)。`neoutl-{linux,windows}-x86_64.zip` | なし | ZIP 展開 |

未確認の項目は `docs/inputs.md` §6。

## 3. 用語

| 用語 | 定義 |
|---|---|
| Instance | 1 つのエディタ本体 + 専用プラグイン + 専用設定のディレクトリ |
| Kind | `aviutl` / `aviutl2` / `ymm4` / `neoutl` |
| Provider | Kind ごとの本体取得・プラグイン一覧取得・導入処理の実装 |
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
      app/            # 本体
      plugins/        # aviutl2: app/data/Plugin を使用しない場合のみ。aviutl は app/ 直下、ymm4 は app/user/plugin
      prefix/         # Wine prefix (Linux / macOS のみ)
```

`<data_dir>` は `directories::ProjectDirs` で決定する。

### 4.2 instance.toml

```toml
id = "uuid-v4"
name = "string"
kind = "ymm4"
app_version = "v4.56.1.0"
group = "string"
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
  launcher-core/                # Iced 非依存。モデル、永続化、タスク
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

| Kind | app_versions | catalog |
|---|---|---|
| aviutl | APM `v3/core.json` | APM `v3/packages/*.json` |
| aviutl2 | 索引の `Kenkun.AviUtlExEdit2` (`direct` の setup 実行ファイル) | `index.json` |
| ymm4 | GitHub Releases API | `ymm4plugins.yml` + GitHub Releases API |
| neoutl | Codeberg (Forgejo) Releases API。`prerelease = false` の最新 | なし (空配列) |

### 5.3 起動層 (Runner)

| Host OS | Kind | 起動方法 |
|---|---|---|
| Windows | 全種 | 直接 spawn |
| Linux / macOS | neoutl | 直接 spawn (NeoUtl がネイティブ対応する場合) |
| Linux / macOS | aviutl / aviutl2 / ymm4 | `wine` + `WINEPREFIX=instances/<id>/prefix` |

- Wine 実行ファイルのパスは `config.toml` の `wine_path` に保持する。
- 未設定かつ `which wine` が失敗した場合、起動ボタンを無効化し、理由を 1 行表示する。
- Wine prefix の初期化は Phase 6 で実装する。

### 5.4 タスク

- すべてのネットワーク処理・ファイル展開を `Task` として実行する。
- `Task` の状態: `Queued` / `Running{done,total}` / `Succeeded` / `Failed{reason}`。
- UI は `iced::Subscription` で状態を受信する。
- 同一 Instance への書き込みタスクは直列化する (Instance 単位の Mutex)。

### 5.5 YMM4 索引のパース

- 入力: `ymm4plugins.yml`。最上位は list。
- 必須キー: `type` `date` `name` `url` `author` `authorUrl` `description`。任意: `links` `isEnabled`。
- `type` はカンマ区切りで複数値を持つ。`Vec<Category>` に分割する。
- `isEnabled = false` のエントリは非表示。
- 導入可能判定: `url` が `https://github.com/<owner>/<repo>` (パス 2 要素) の場合のみ `Download`。それ以外は `OpenInBrowser`。
- 破損検出: 件数 0、または必須キー欠落が 1 件以上の場合は `Err(CatalogShapeChanged)`。キャッシュを上書きしない。

## 6. 安全性

- `#![forbid(unsafe_code)]` を全 crate に設定する。unsafe は 0 件。
- zip 展開: 各エントリの正規化後パスが展開先ディレクトリの配下であることを検証する。違反時はタスク全体を `Failed` にし、書き込み済みファイルを削除する。
- ハッシュ: カタログがハッシュを提供する場合は SHA-256 を検証する。提供しない場合は取得後に計算して `instance.toml` に記録しない (記録先が `files` のみのため)。
- ダウンロード先ホスト許可リスト: `github.com`, `api.github.com`, `codeberg.org`, `objects.githubusercontent.com`, およびカタログ Provider が指定するホスト。リダイレクト先もリストで検証する。
- 索引の `run` アクション: 実行前に `path` と `args` を表示して確認を取る。`elevate = true` はランチャーでは昇格せず、確認ダイアログに表示する。
- HTTP: `reqwest` + `rustls`。User-Agent は `MLAYN-Launcher/<version>`。GitHub API は未認証 60 req/h。ETag で条件付き取得し、`config.toml` の任意トークンで上限を引き上げる。

## 7. UI (Iced)

### 7.1 画面

| 画面 | 内容 |
|---|---|
| Main | Instance グリッド (Group 単位)。起動 / 編集 / 削除 / 新規 |
| New Instance | Kind 選択 → 名前 → 本体バージョン選択 → 作成 |
| Instance | タブ 3 つ: Plugins (導入済み) / Browse (カタログ) / Settings |
| Tasks | 進行中タスク一覧 |
| Settings | データディレクトリ、wine_path、GitHub トークン、言語 |

### 7.2 状態管理

- `App` 1 つの `State`。`Message` enum は画面ごとにネスト (`Message::Main(..)`, `Message::Instance(..)`)。
- 副作用はすべて `Task` / `Subscription` 経由。`update` 内でブロッキング処理を行わない。

### 7.3 カタログ表示

- 検索: 名前・説明・開発者の部分一致 (大文字小文字無視)。
- フィルタ: カテゴリ (YMM4 の 18 分類)。
- 行表示: 名前、開発者、説明 1 行、導入状態 (`導入済み` / `更新あり` / `未導入`)、操作ボタン (`導入` / `ブラウザで開く`)。
- 仮想化: 全件を `scrollable` + 可視範囲のみ生成で描画する。

## 8. フェーズ

| Phase | 内容 | 完了条件 |
|---|---|---|
| 0 | 入力調査 | 完了。`docs/inputs.md` に記載。未確認 8 件は `docs/inputs.md` §6 |
| 1 | `launcher-core`: モデル、`instance.toml` 読み書き、Instance CRUD | 単体テスト: 作成→保存→読込→削除が成功 |
| 2 | Provider: YMM4 + NeoUtl (本体取得とカタログ) | `cargo test` で実ネットワークを使わない `ymm4plugins.yml` fixture の全件パース成功 (件数は実ファイルで確定) |
| 3 | Installer: ダウンロード、展開、ファイル記録、削除 | zip-slip fixture が `Failed`、正常 fixture が `files` 全件記録 |
| 4 | UI: Main / New Instance / Tasks | YMM4 と NeoUtl のインスタンスを UI から作成して起動できる (Windows) |
| 5 | UI: Browse / Plugins、Provider: AviUtl2 + AviUtl 無印 | 4 Kind すべてでカタログ表示とプラグイン導入が成功 |
| 6 | Runner: Wine 対応 (Linux / macOS) | Linux 上で YMM4 インスタンスが prefix 作成後に起動 |
| 7 | 配布: GitHub Actions で 3 OS ビルド、リリース成果物生成 | tag push で 3 OS の成果物が Release に添付される |

各 Phase は 1 PR。Phase 完了時点で `main` はビルドとテストが成功している状態を保つ。

## 9. テスト

| 種別 | 対象 | 方法 |
|---|---|---|
| 単体 | モデル、パス検証、バージョン比較 | `cargo test` |
| Fixture | `ymm4plugins.yml` / `index.json` / APM `packages/*.json` のパース | 取得済みファイルを `tests/fixtures/` に保存し比較 |
| 統合 | Installer | `tempfile` 上で zip を展開、`files` と実体を照合 |
| CI | fmt / clippy / test | `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` を 3 OS で実行 |

ネットワークを使うテストは `#[ignore]` を付け、CI の定期ジョブでのみ実行する。

## 10. 依存クレート

| 用途 | クレート |
|---|---|
| UI | iced |
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
| 一時領域 (test) | tempfile |

## 11. 未決事項

| # | 内容 | 既定値 |
|---|---|---|
| 1 | YMM4 を Linux / macOS で Wine 起動することの許容 | 許容 (Phase 6) |
| 2 | BOOTH / Google Drive / note 配布プラグインの扱い | ブラウザで開く。自動導入しない |
| 3 | カタログ更新間隔 | 起動時 + 手動更新。キャッシュ有効期間 24 時間 |
| 4 | 言語 | 日本語のみ。文字列は定数モジュールに集約 |
| 5 | APM データ (CC BY-NC-SA 4.0)、NeoUtl (AGPL)、各索引の同梱 | 同梱しない。実行時に取得 (決定済) |
| 6 | macOS ホストでの NeoUtl | 作成を無効化 (Stable に macOS asset なし) |
