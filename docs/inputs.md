# Phase 0 入力調査 (2026-10-28 更新)

凡例: 確認済 = 取得した一次資料に記載あり。推定 = 資料から導出。未確認 = 取得不能。

## 0. 決定事項と取得状況

| 項目 | 内容 | 決定日 |
|---|---|---|
| プロジェクト名 | `MLAYN-Launcher` | 10-27 |
| YMM4 本体の取得元 | GitHub Releases (`manju-summoner/YukkuriMovieMaker4`) | 10-27 |
| 索引・データリポジトリ | 同梱しない。実行時に取得 | 10-27 |
| `.ymme` | 階層を持つ `.ymme` は存在しない (ユーザー申告)。専用ディレクトリを作成し、全エントリを収集して配置 | 10-28 |
| AviUtl2 の Wine | 動作しない。Wine を使用しない | 10-28 |
| AviUtl2 の導入 | 公式のポータブル ZIP を `instances/<id>/app/` へ展開。セットアップ実行ファイルは使用しない。インスタンス間は完全独立 | 10-28 |
| Linux 対応 Kind | NeoUtl と Freya NLE のみ | 10-28 |
| 表記 | 旧名称を `Freya NLE` に置換 | 10-28 |

実行環境から取得できた範囲:

| 対象 | 状態 |
|---|---|
| `ymm4plugins.yml` (426 件)、`index.json` (445 件)、`YMM4LiquidGlass.ymme` | 添付の実体で確認 |
| `apm-data-source.zip` (331 ファイル。YAML ソースと v3 仕様書) | 添付の実体で確認。生成物の v3 JSON は未取得 |
| YMM4 Releases ページ | ユーザー貼付で確認 (asset 名・sha256) |
| AviUtl2 セットアップのオプション | `docs.aviutl2.jp/usage` で確認 |
| Freya NLE | `github.com/GT-610/freya-nle` のトップページで確認。asset 名は未確認 |
| Forgejo (Codeberg) API | シェルから 403。実応答は未取得 |
| `manjubox.net` | シェルから 403 |

## 1. YMM4

### 1.1 プラグイン索引 `https://manjubox.net/ymm4plugins.yml` (確認済、実体)

| 項目 | 値 |
|---|---|
| 形式 | YAML。最上位は list。ラッパーキーなし |
| 総件数 | 426 |
| `isEnabled: false` | 14 件 |
| 表示件数 | 412 (426 − 14)。一覧ページの「すべて(412)」と一致 |
| 並び順 | `date` 降順 |
| ライセンス表記 | 未確認 |

`isEnabled: false` の 14 件は、一覧ページの「リリース前 / 配布終了 等(14)」に対応すると推定。

| キー | 件数 | 型 | 備考 |
|---|---|---|---|
| `type` | 426 | string | カンマ区切りで複数値 10 件 |
| `date` | 426 | `YYYY-MM-DD` | |
| `name` | 426 | string | |
| `url` | 426 | string | |
| `author` | 426 | string | 絵文字は `\U0001F363` 形式のエスケープ |
| `authorUrl` | 426 | string | |
| `description` | 426 | string | |
| `links` | 405 | list of string | 任意 |
| `isEnabled` | 14 | bool | 任意。値はすべて `false` |

`type` の単一値 (416 件): 映像エフェクト 198 / ツール 84 / 図形 43 / その他 33 / 音声エフェクト 21 / 音声合成 11 / 立ち絵 6 / 動画出力 6 / 場面切り替え 3 / 動画読み込み 3 / 波形 2 / テキスト補完 2 / 文字起こし 1 / 模様 1 / 音声読み込み 1 / 画像読み込み 1。

`url` のホスト (上位): github.com 296 / drive.google.com 25 / benikazura.booth.pm 19 / ymm4-info.net 12 / kahenshiki.booth.pm 8 / hidechan.booth.pm 8 / guggusu.booth.pm 6 / sodane001.booth.pm 5 / leftcomtroller.booth.pm 4 / その他。
`github.com` の 296 件は、すべて `https://github.com/<owner>/<repo>` の形式。

### 1.2 導入可否

| `url` | 動作 |
|---|---|
| `https://github.com/<owner>/<repo>` | GitHub Releases API の最新 Release から `.ymme` を取得。asset に `.ymme` がない場合は `OpenInBrowser` |
| それ以外 | 既定ブラウザで開く |

`.ymme` が Release asset にある割合は未確認 (296 件を API で走査する必要がある)。

### 1.3 `.ymme` (確認済、実体 1 件 + ユーザー申告)

`YMM4LiquidGlass.ymme` (114,848 bytes) の内容:

| エントリ | サイズ | 内容 |
|---|---|---|
| `YMM4LiquidGlass.dll` | 74,752 | プラグイン本体 |
| `ComputeSharp.D2D1.dll` | 188,416 | 依存ライブラリ |
| `readme.txt` | 1,687 | 説明 |

- ZIP。フォルダ階層なし。マニフェストなし。
- 階層を持つ `.ymme` は存在しない (ユーザー申告)。

配置規則:

1. `<instance>/app/user/plugin/<repo>/` を作成する (`<repo>` = GitHub リポジトリ名)。
2. `.ymme` の全エントリをそのディレクトリ直下へ展開する。
3. 展開した相対パスを `instance.toml` の `files` に記録する。
4. エントリ名に `/` `\` `..` を含む場合は `Failed` (zip-slip 検証と同一)。

削除時は `files` の全件を削除し、空になったディレクトリを削除する。

### 1.4 YMM4 本体 (確認済、Releases ページ)

| 項目 | 内容 |
|---|---|
| 取得元 | GitHub Releases `manju-summoner/YukkuriMovieMaker4` |
| 最新 tag | `v4.56.1.0` (2026-09-05) |
| Release 数 | 約 190 |
| Asset | 各 Release に 2 件 + 自動生成のソースアーカイブ 2 件 (表示上 "Assets 4") |
| 通常版 | `YukkuriMovieMaker_v<version>.zip` (494 MB) |
| Lite 版 | `YukkuriMovieMaker_v<version>_Lite.zip` (492 MB) |
| 検証 | GitHub が asset ごとに `sha256:<hex>` を提示 (REST API の `digest` フィールド) |
| 展開後 | フォルダ内の `YukkuriMovieMaker.exe` を実行。インストーラーなし |
| 対応 OS | Windows のみ |

`v4.56.1.0` の sha256:

| Asset | sha256 |
|---|---|
| `YukkuriMovieMaker_v4.56.1.0.zip` | `34b5f4c737345f1b82507381af514fc14b629f35e9ba778c25434f235fdb35cf` |
| `YukkuriMovieMaker_v4.56.1.0_Lite.zip` | `49c0ed689f545737b7ce939971bfc625962e00791c57883dc8e6f058aa336c5a` |

asset 選択規則: 正規表現 `^YukkuriMovieMaker_v[0-9.]+(_Lite)?\.zip$` に一致する asset のみを対象とする。`_Lite` の有無を `edition` (`normal` / `lite`) として `instance.toml` に記録する。ソースアーカイブは対象外。

Lite 版の展開後の構造は未確認 (`YukkuriMovieMaker.exe` の有無)。

## 2. AviUtl2 カタログ

### 2.1 リポジトリ (確認済)

| 対象 | 場所 |
|---|---|
| アプリ | `github.com/Neosku/aviutl2-catalog` (MIT。NSIS インストーラー。Windows x64) |
| データ | `github.com/Neosku/aviutl2-catalog-data` |
| 索引 | `https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/index.json` |

### 2.2 索引 `index.json` (確認済、実体)

| 項目 | 値 |
|---|---|
| 形式 | JSON。最上位は array |
| 件数 | 445。`id` は一意 |
| サイズ | 1.86 MB |
| `version[]` が空の件 | 0 |

`type` の分布: スクリプト 248 / 汎用プラグイン 95 / フィルタプラグイン 43 / 出力プラグイン 27 / MOD 12 / 入力プラグイン 8 / 言語ファイル 5 / プリセット 2 / スクリプトモジュール 2 / 外部プログラム 2 / 本体 1。

`installer.source` の分布: github 309 / GoogleDrive 59 / direct 57 / booth 20。

`installer.install` のアクション: copy 859 / download 446 / extract 319 / delete 43 / extract_sfx 8 / run_auo_setup 8 / run 5。
最頻の列: `download → extract → copy` 144 / `download → copy` 113 / `download → extract → copy ×2` 68。

スキーマ (aviutl2-cli 0.9.0-preview.0 の定義に、実体で確認した項目を追加):

```
Entry {
  id, name, type, summary, author, repoURL: string
  description: string            // 相対パス "./md/<id>.md" (リポジトリ内)
  originalAuthor?: string
  niconiCommonsId?: string
  popularity: number             // 実体に存在。aviutl2-cli の定義にはない
  trend: number                  // 同上
  "latest-version": string
  licenses: [{ type, isCustom, copyrights: [{years, holder}], licenseBody? }]
  tags: [string]
  dependencies: [string]         // Entry.id
  images: [{ thumbnail?: "./image/<id>_thumbnail.png", infoImg?: [string] }]   // 相対パス
  installer: { source, install: [Action], uninstall: [Action] }
  version: [{ version, release_date, file: [{ path, XXH3_128 }] }]
}
source = { direct: url } | { booth: string } | { github: { owner, repo, pattern: regex } } | { GoogleDrive: { id } }
Action = download | extract | extract_sfx | copy{from,to} | delete{path} | run{path,args[],elevate?} | run_auo_setup{path}
```

`description` と `images` の相対パスの基準は `https://raw.githubusercontent.com/Neosku/aviutl2-catalog-data/refs/heads/main/` (推定)。

### 2.3 プレースホルダ (確認済、実体の出現数)

| 名前 | 出現 | 解決先 |
|---|---|---|
| `{scriptsDir}` | 873 | `{dataDir}/Script` |
| `{tmp}` | 868 | 作業用の一時ディレクトリ |
| `{pluginsDir}` | 678 | `{dataDir}/Plugin` |
| `{dataDir}` | 319 | AviUtl2 のデータディレクトリ |
| `{appDir}` | 2 | 本体のインストール先 (`aviutl2.exe` の位置) |
| `{download}` | 1 | ダウンロードしたファイルのパス |

`{appDir}` は aviutl2-cli の実装に存在しない。公式カタログ固有のプレースホルダ。

`copy` の `to`: `{scriptsDir}` 264 / `{pluginsDir}` 124 / `{dataDir}/Language` 112 / `{dataDir}/Preset` 24 / その他は上記の下位ディレクトリ。
`copy` の `from` は、拡張子付きファイルが多数。拡張子なしの 85 件はディレクトリ (例 `{tmp}/Plugin` → `{pluginsDir}`)。ディレクトリの場合の挙動は、内容のマージと推定 (uninstall が `{pluginsDir}/al2_jd.aui2` を直接削除するため)。

### 2.4 `run` アクション (確認済、全 5 件)

| id | `path` | `args` | `elevate` |
|---|---|---|---|
| `Kenkun.AviUtlExEdit2` (本体) | `{download}` | `-F -S -I "{appDir}"` | true |
| `sagami121.AudioEnc` | `{tmp}/AudioEnc_Setup.exe` | `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR="{pluginsDir}"` | なし |
| `cleaning.sam3_bb_gb_generator` | `{pluginsDir}/SAM3/python/uv.exe` | `--directory "{pluginsDir}/SAM3/python" sync --locked` | なし |
| `ar-ca-na.open-last-project` | `{dataDir}/aviutl2-open-last.exe` | `--set-app "{appDir}"` | なし |
| `ar-ca-na.aup2-relink` | `{dataDir}/aup2リンク修復.exe` | `--install` | なし |

- すべて Windows 実行ファイル。Linux では実行しない (Linux 非対応の Kind)。
- 索引は任意の実行ファイルを起動できる。実行前に確認ダイアログで `path` と `args` を表示する (PLAN §6)。

### 2.5 本体エントリ (確認済、実体 + 公式ドキュメント)

| 項目 | 値 |
|---|---|
| `id` | `Kenkun.AviUtlExEdit2` |
| `latest-version` | `v2.1.15` |
| `source` | `direct: https://spring-fragrance.mints.ne.jp/aviutl/AviUtl2_v2.1.15_setup.exe` |
| `install` | `download` → `run {download} -F -S -I "{appDir}"` (`elevate: true`) |
| `uninstall` | `run {appDir}/aviutl2_setup.exe -uninstall` (`elevate: true`) |
| 検証 | `version[].file[]` の `{appDir}/aviutl2.exe` の XXH3-128 |

セットアップのオプション (`docs.aviutl2.jp/usage`):

| オプション | 意味 | 制約 |
|---|---|---|
| なし | インストールウィザード | |
| `-I <path>` | インストール実行 | 管理者権限が必要 |
| `-U` | アンインストール実行 | 管理者権限が必要 |
| `-uninstall` | アンインストールウィザード | |
| `-F` | `.aup2` のファイル関連付け | `-I` より前に指定 |
| `-S` | デスクトップショートカット作成 | `-I` より前に指定 |
| `-L` | 言語ファイルのコピー | `-I` より前に指定 |

データフォルダ: 公式ドキュメントと二次資料は `C:\ProgramData\aviutl2` を記載する (`Alias` `Backup` `Language` ほか、`keyboard.ini`、`style.conf`)。`-I` で指定したインストール先とは独立。ポータブル動作 (実行ファイル隣接のデータフォルダ) の記述は、確認した 3 ページに存在しない。

ポータブル ZIP (ユーザー申告) を使用する場合の帰結:

- セットアップ実行ファイル、昇格要求、`-F` `-S` は不要。
- `{dataDir}` をインスタンス内に置けるかは、ZIP の実物で確認する (§8 項目 4)。
- Wine では動作しない (ユーザー申告)。Linux では AviUtl2 を扱わない。

### 2.6 単一インスタンス前提の箇所

| 項目 | 内容 | 状態 |
|---|---|---|
| `{dataDir}` | OS 全体で 1 つの固定パス | 確認済 |
| `{appDir}` | `-I` で任意に指定可能 | 確認済 |
| `{tmp}` | 実装側で解決 | 設計 |

マルチインスタンス化の範囲: `{appDir}` `{dataDir}` `{tmp}` をインスタンスごとに解決する。共有しない (決定済)。

## 3. APM (AviUtl 無印)

### 3.1 リポジトリ (確認済)

| 対象 | 場所 | ライセンス |
|---|---|---|
| アプリ | `github.com/team-apm/apm` (TypeScript) | MIT |
| データ | `github.com/team-apm/apm-data` | **CC BY-NC-SA 4.0** |
| スキーマ | `github.com/team-apm/apm-schema` (v3.3.0) | CC BY 4.0 |

配信: `https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/` (jsDelivr)。
更新状況: apm-data リポジトリの更新日表示は 2024-11-02。現在の更新頻度は未確認。

### 3.2 ファイル構成 (確認済、仕様書 `v3/SPECIFICATION.md` + ソース)

配信 (`v3/`):

| パス | 内容 |
|---|---|
| `list.json` | マニフェスト。`core` `convert` `packages[]` `scripts[]` の各 `DataFileInfo { path, modified }` |
| `core.json` | `{ version: 3, aviutl, exedit }` |
| `packages/<developerID>.json` | `{ version: 3, packages: [...] }` |
| `scripts.json` | `{ webpage[], scripts[] }` |
| `convert.json` | 旧 ID → 新 ID の対応 |
| `*.min.json` | 同内容の圧縮版 |

ソース (`src/`): `core/aviutl.yaml` `core/exedit.yaml` `packages/<dev>/<name>/package.yaml` (285 件) `convert.yaml` `scripts/{scripts,webpage}.yaml`。`util/release.ts` が `v3/` を生成する。

パッケージ ID: `<developerID>/<name>`。

### 3.3 パッケージ項目 (確認済、仕様書 + 285 件の実体)

先行版の記述 (`displayName` `{ja,en}`、`files.source/destination`、`downloadURLs[].directURL`) は二次資料に基づき、仕様書と不一致。以下に訂正する。

| キー | 型 | 必須 | 件数 /285 |
|---|---|---|---|
| `id` | string | 必須 | 285 |
| `name` | string (25 字以内) | 必須 | 285 |
| `overview` | string (35 字以内) | 必須 | 285 |
| `description` | string | 必須 | 285 |
| `developer` | string | 必須 | 285 |
| `pageURL` | string | 必須 | 285 |
| `downloadURLs` | string[] (ダウンロードページ) | 必須 | 285 |
| `latestVersion` | string | 必須 | 285 |
| `files` | `FileInfo[]` | 必須 | 285 |
| `releases` | `ReleaseData[]` | 任意 | 214 |
| `nicommons` | string | 任意 | 153 |
| `dependencies` | string[] (`a\|b` は選択肢) | 任意 | 69 |
| `originalDeveloper` | string | 任意 | 16 |
| `isHidden` | bool | 任意 | 4 |
| `conflicts` | string[] (未実装) | 任意 | 3 |
| `isContinuous` | bool (仕様書では `@continuous`) | 任意 | 3 |
| `directURL` | string | 任意 | 2 |
| `installer` `installerArg` | string | 任意 | 0 |

`FileInfo { filename, archivePath?, isDirectory?, isUninstallOnly?, isInstallOnly?, isObsolete? }` (787 件。`archivePath` 364、`isDirectory` 56、`isUninstallOnly` 29、`isObsolete` 25、`isInstallOnly` 17)。
`ReleaseData { version, integrity: { archive, file: [{ target, hash }] } }`。`integrity` は全 783 件が `sha384-<base64>`。

- `filename`: インストール先 (`aviutl.exe` のあるフォルダ) からの相対パス。
- `archivePath`: アーカイブ内の相対パス。2 例 (`SigColorFastAviUtl.auf` + `SigColorFastAviUtl/`、`plugins/SortEffect.auf` + `plugins/`) が単一の解釈で整合しない。未確認。
- ダウンロードの直接 URL を持つ項目は `directURL` のみ (2 件)。
- 285 件中 71 件は `releases` を持たず、ハッシュ検証ができない。

### 3.4 導入可否 (確認済、`util/check-update.ts` の挙動から導出)

| 条件 | 件数 | 動作 |
|---|---|---|
| `directURL` あり | 2 | その URL を取得 |
| `downloadURLs` に `https://github.com/<owner>/<repo>/releases...` あり | 78 | 最新 Release を取得。`.zip` asset が 1 件ならそれを使用。2 件以上なら `util/archive-name-pattern.json` (44 件。キー = パッケージ ID、値 = 正規表現) の `^<pattern>\.zip$` に一致する asset。一致なしは `OpenInBrowser` |
| 上記以外 | 205 | `OpenInBrowser` |

`downloadURLs` のホスト (285 件の全 URL): `tim3.web.fc2.com` 182 / `github.com` 79 / `hazumurhythm.com` 23 / `aoytsk.blog.jp` 22 / `drive.google.com` 12 / `videoinfo.tenchi.ne.jp` 10 / その他。
`latestVersion` が `最新` の項目は版比較を行わない。

### 3.5 `core.json` (確認済、ソース YAML)

| 対象 | `latestVersion` | `releases[].url` の例 | 検証 |
|---|---|---|---|
| `aviutl` | `1.10` | `http://spring-fragrance.mints.ne.jp/aviutl/aviutl110.zip` | sha384 (archive と `aviutl.exe`) |
| `exedit` | `0.92` | `http://spring-fragrance.mints.ne.jp/aviutl/exedit93rc1.zip` ほか | sha384 |

- `releases[]` は新しい順。`exedit` は `releases[0]` が `0.93rc1`、`latestVersion` が `0.92`。既定の選択は `latestVersion`。
- URL のスキームは `http`。取得時に sha384 検証を必須とする。
- `aviutl.exe` の `isUninstallOnly` ファイル: `aviutl.aup` `aviutl.ini` `aviutl.key` `aviutl.sav`。

### 3.6 `scripts.json` / `convert.json` (確認済、ソース YAML)

- `convert`: 旧 ID をキー、新 ID を値とする対応 (例 `InputPipePlugin` → `amate/InputPipePlugin`)。
- `scripts[]`: `match` (URL パターン) に対して、`folder` `developer` `dependencies` または `redirect` (パッケージ ID) を返す。後ろの要素が優先。スクリプト配布サイトの URL から導入先 (`script/<folder>`) を決定する用途。ランチャーの対象外 (Phase 5 の範囲外)。

### 3.7 帰結

- ライセンス: データは非営利限定 (CC BY-NC-SA 4.0)。ランチャー本体 (GPL-3.0) とデータは別配布物。ランチャーはデータを同梱せず実行時に取得する。
- 導入先はインスタンスの `app/` (= `aviutl.exe` の位置)。

## 4. NeoUtl (Codeberg)

### 4.1 リポジトリ (確認済)

| 項目 | 値 |
|---|---|
| Codeberg | `https://codeberg.org/TeamNeoUtl/NeoUtl` |
| GitHub ミラー | `https://github.com/taisho-guy/NeoUtl` |
| 公式サイト | `https://neoutl.taisho-guy.org` |
| ライセンス | AGPL (本文) / Wiki は CC BY-SA 4.0 |
| API | Forgejo。`https://codeberg.org/api/v1`。仕様 `https://codeberg.org/swagger.v1.json`。実呼び出しはシェルから 403 |

### 4.2 Release (確認済)

| tag | 区分 | 日付 | asset |
|---|---|---|---|
| `0.8.11` | Stable | 2026-09-26 | `neoutl-linux-x86_64.zip` (109 MiB) / `neoutl-windows-x86_64.zip` (103 MiB) |
| `0.0.95-Unstable` | Pre-release | 2026-05-28 | Linux x86_64 / macOS ARM64 / Windows x86_64 の各 1 件 (ファイル名は改名前の名称。Provider の選択対象外) |

- asset 名規則 (Stable): `neoutl-<os>-<arch>.zip`。`<os>` = `linux` / `windows`、`<arch>` = `x86_64`。
- macOS の Stable asset: なし。
- Release 数: 2 (tag は 216)。

### 4.3 導入手順 (確認済。Wiki は改名前の記述を含むと自己申告)

| OS | 内容 |
|---|---|
| Windows | ZIP 展開。依存同梱。`NeoUtl` を実行 |
| Linux | 依存 (Qt6 全般 / LuaJIT / Vulkan 実装 / FFmpeg / Carla) を別途導入。ZIP 展開後 `NeoUtl` に実行権限を付与して実行 |
| macOS | 現行 Stable の提供なし |

- 対象: Linux x86_64 / Windows x86_64。
- プラグイン: AviUtl のプラグインとは非互換。互換レイヤーの予定なし。拡張方式は WGSL / Rust (比較表の記述)。
- 技術スタック表記: Home は Slint / wgpu、Installation は Qt6。Wiki 内で不一致。

### 4.4 Forgejo API の応答 (一般仕様。NeoUtl への実呼び出しは未取得)

| 項目 | 内容 |
|---|---|
| ベース | `https://codeberg.org/api/v1` |
| 形式 | `application/json` |
| Release 一覧 | `GET /repos/{owner}/{repo}/releases` (Gitea 互換) |
| ページネーション | `Link` ヘッダー (`rel="next"` `rel="last"`)、`X-Total-Count` ヘッダー |
| エラー | `{ "message": string, "url": string }` |

Release 項目の使用フィールド (Gitea 互換スキーマの一般仕様。NeoUtl の実応答で未確認): `tag_name` `draft` `prerelease` `published_at` `assets[].name` `assets[].size` `assets[].browser_download_url`。
Gitea 互換の `assets[]` は `digest` を持たない。ハッシュ検証は、配布物にチェックサムファイルがある場合に限る (有無は未確認)。

### 4.5 帰結

- Provider は Forgejo Releases から `draft = false` かつ `prerelease = false` の最新を選び、asset 名 `neoutl-{os}-{arch}.zip` に一致するものを取得する。
- 対象 OS: Windows / Linux。macOS は対象外。
- Linux ホストでは起動前に依存の存在確認を行わない (配布元の依存一覧は更新される)。起動失敗時にログを表示する。

## 5. Freya NLE

### 5.1 リポジトリ (確認済、トップページ)

| 項目 | 内容 |
|---|---|
| GitHub | `https://github.com/GT-610/freya-nle` (改名前の URL は GitHub がリダイレクト) |
| ライセンス | AGPLv3 |
| 構成 | Rust / Slint / wgpu |
| 対応 OS (README 記載) | Linux / Windows / macOS |
| 起動条件 | GPU アダプタ必須。WARP 等のソフトウェアアダプタは起動時に非対応と報告 |
| プロジェクトファイル | `.aviqtl` `.aviqtlfrag` (改名後も読み込み可) |
| 拡張 | Lua プラグイン (パッケージ管理、宣言的パラメータ、権限制御)。`effect-packages/` に 2 件の例 |
| 音声 | Carla 経由で VST3 / LV2 |

### 5.2 Release (部分確認)

| tag | 区分 | asset 数 |
|---|---|---|
| `v0.7.0-rc.1` | `Latest` 表示 | 5 |
| `v0.6.2` | 不明 | 5 |
| `v0.6.1` | 不明 | 5 |

- asset 名、サイズ、digest、`prerelease` の値は未確認 (ページ表示に出現せず、API はシェルから未到達)。
- README は改名の時期を「0.8 まで」と記載する。最新 Release は `v0.7.0-rc.1`。両者の整合は未確認。

### 5.3 帰結

- Kind `freya` を追加する。本体の取得元は GitHub Releases `GT-610/freya-nle`。
- Release に `prerelease = false` が存在しない場合があるため、`prerelease` を含めて最新を選び、UI に区分を表示する。
- プラグイン管理は実装しない (`catalog` は空配列)。Lua プラグインのパッケージ管理は対象外。
- 対象 OS: Windows / Linux。macOS は asset 未確認のため対象外。
- asset の選択規則は Phase 2 で `GET /repos/GT-610/freya-nle/releases` の実応答から確定する。

## 6. ホスト別の対応 Kind

| Kind | Windows | Linux |
|---|---|---|
| `aviutl` | 対応 | 非対応 |
| `aviutl2` | 対応 | 非対応 (Wine 不使用) |
| `ymm4` (normal / lite) | 対応 | 非対応 (Wine 不使用) |
| `neoutl` | 対応 | 対応 |
| `freya` | 対応 | 対応 |

macOS: ランチャーの対象外 (README: Windows 11 と Linux)。
非対応の Kind は、New Instance 画面で選択不可とし、理由を 1 行表示する。

## 7. PLAN.md への差分

| 節 | 変更 |
|---|---|
| §1 §3 | Kind に `freya` を追加。対象 OS を Windows / Linux に変更 |
| §2 | 入力表に `freya` と YMM4 の asset 規則、APM の導入条件を追加 |
| §4.1 | `prefix/` を削除。`instance.toml` に `edition` を追加 |
| §5.2 | Kind 表に `freya` を追加 |
| §5.3 | Wine を削除。ホスト別の対応表に置換 |
| §5.5 | `.ymme` の配置規則を追加 |
| §6 | AviUtl2 はポータブル ZIP を展開。`http` の core URL の sha384 検証を追加 |
| §7 §8 §11 | `wine_path` と Phase 6 (Wine) を削除。macOS を削除。UI を Prism Launcher の構成へ変更。Phase 1 を完了に変更 |

## 8. 要確認 (Phase 0 後に残る項目)

| # | 内容 | 閉じる方法 | 影響する Phase |
|---|---|---|---|
| 1 | `.ymme` を Release asset に持つリポジトリの割合 (GitHub 296 件) | Phase 2 で全件走査 | 2 |
| 2 | Freya NLE の Release asset 名 5 件、OS 区分、`prerelease` の値 | Phase 2 で Releases API の実応答を確認 | 2 |
| 3 | NeoUtl の Forgejo 実応答 (`assets[]` のフィールド、チェックサムの有無) | Phase 2 で `GET /repos/TeamNeoUtl/NeoUtl/releases` を確認 | 2 |
| 4 | AviUtl2 ポータブル ZIP の配布 URL、asset 名、展開後の構造、`{dataDir}` をインスタンス内に置く方法 | Phase 5 で ZIP の実物を確認 | 5 |
| 5 | YMM4 Lite 版の展開後の構造 | Phase 2 で ZIP のエントリ一覧を確認 | 2 |
| 6 | APM `archivePath` の解釈 | Phase 3 で実アーカイブ 2 件以上と照合 | 3 |
| 7 | APM 生成物 (`v3/*.json`) の実体 | Phase 2 で `cdn.jsdelivr.net` から取得 | 5 |
| 8 | APM の GitHub パッケージ 78 件のうち `archive-name-pattern.json` に一致しない件数 | Phase 5 で全件走査 | 5 |
