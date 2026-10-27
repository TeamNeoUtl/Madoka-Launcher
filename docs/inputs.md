# Phase 0 入力調査 (2026-10-27、添付 3 ファイル反映済)

凡例: 確認済 = 取得した一次資料に記載あり。推定 = 資料から導出。未確認 = 取得不能。

## 0. 決定事項と取得状況

| 項目 | 内容 |
|---|---|
| プロジェクト名 | `MLAYN-Launcher` |
| YMM4 本体の取得元 | GitHub Releases (`manju-summoner/YukkuriMovieMaker4`)。`files.manjubox.net` は使用しない |
| 索引・データリポジトリ | 同梱しない。実行時に取得 |
| 添付で確認した実体 | `ymm4plugins.yml` (426 件)、`index.json` (445 件)、`YMM4LiquidGlass.ymme` |
| シェルの到達先 | `manjubox.net` は 403。GitHub / Codeberg は未試行 |
| 未取得 | YMM4 Release の asset 名、APM `v3/*.json` 実体、Forgejo API の応答 |

凡例: 確認済 = 実体または一次資料で確認。推定 = 資料から導出。未確認 = 取得不能。

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

### 1.3 `.ymme` (確認済、実体 1 件)

`YMM4LiquidGlass.ymme` (114,848 bytes) の内容:

| エントリ | サイズ | 内容 |
|---|---|---|
| `YMM4LiquidGlass.dll` | 74,752 | プラグイン本体 |
| `ComputeSharp.D2D1.dll` | 188,416 | 依存ライブラリ |
| `readme.txt` | 1,687 | 説明。インストール手順は「`.ymme` をダブルクリック」 |

- ZIP。フォルダ階層なし。マニフェストなし。
- 手動配置先は `<YMM4>/user/plugin/` (サブフォルダ推奨)。設定は `<YMM4>/user/setting/`。
- ダブルクリック時のインストーラーが ZIP 展開以外の処理を行うかは未確認。
- 標本は 1 件。他の `.ymme` が階層を持つかは未確認。

方針: `<instance>/app/user/plugin/<ymme の stem>/` へ全エントリを展開する。書き込んだ相対パスを `instance.toml` の `files` に記録する。

### 1.4 YMM4 本体 (部分確認)

| 項目 | 内容 | 状態 |
|---|---|---|
| 取得元 | GitHub Releases `manju-summoner/YukkuriMovieMaker4` | 決定 |
| 最新 tag | v4.56.1.0 | 確認済 |
| Release 数 | 約 190 | 確認済 |
| Asset 数 | 4 / Release | 確認済 |
| Asset 名と通常版 / Lite 版の区別 | | 未確認 |
| 展開後 | フォルダ内の `YukkuriMovieMaker.exe` を実行。インストーラーなし | 確認済 |
| 対応 OS | Windows のみ | 確認済 |

Asset 選択規則は Phase 2 の最初のタスクで確定する (`GET /repos/manju-summoner/YukkuriMovieMaker4/releases` の実応答から)。

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

- すべて Windows 実行ファイル。Linux / macOS では Wine 経由が必要。
- 索引は任意の実行ファイルを起動できる。実行前に確認ダイアログで `path` と `args` を表示する (PLAN §6)。

### 2.5 本体エントリ (確認済、実体)

| 項目 | 値 |
|---|---|
| `id` | `Kenkun.AviUtlExEdit2` |
| `latest-version` | `v2.1.15` |
| `source` | `direct: https://spring-fragrance.mints.ne.jp/aviutl/AviUtl2_v2.1.15_setup.exe` (配布元は作者の公式サイト) |
| `install` | `download` → `run {download} -F -S -I "{appDir}"` (`elevate: true`) |
| `uninstall` | `run {appDir}/aviutl2_setup.exe -uninstall` (`elevate: true`) |
| 検証 | `version[].file[]` の `{appDir}/aviutl2.exe` の XXH3-128 |

- 本体は ZIP ではなくセットアップ実行ファイル。`-I` で導入先を指定する。
- `-F` `-S` の意味は未確認。
- `elevate: true` は管理者権限の要求。導入先を `{appDir}` = `<instance>/app` にした場合に必要かは未確認。
- `{dataDir}` が `{appDir}/data` になるかは未確認 (aviutl2-cli は `install_dir/data` を使用)。
- Linux / macOS では、セットアップ実行ファイルを Wine で実行する (動作は未確認)。

### 2.6 単一インスタンス前提の箇所

| 項目 | 内容 | 状態 |
|---|---|---|
| `{dataDir}` / `{appDir}` | 索引はパスを変数で参照。実装側が 1 組の値を保持する | 確認済 |
| 既定の `{dataDir}` | `C:\ProgramData\aviutl2` (別ツールの記述) | 推定 |
| マルチインスタンス化 | インスタンスごとに `{appDir}` `{dataDir}` `{tmp}` を解決する | 設計 |

## 3. APM (AviUtl 無印)

### 3.1 リポジトリ (確認済)

| 対象 | 場所 | ライセンス |
|---|---|---|
| アプリ | `github.com/team-apm/apm` (TypeScript) | MIT |
| データ | `github.com/team-apm/apm-data` | **CC BY-NC-SA 4.0** |
| スキーマ | `github.com/team-apm/apm-schema` (v3.3.0) | CC BY 4.0 |

配信: `https://cdn.jsdelivr.net/gh/team-apm/apm-data@main/v3/` (jsDelivr)。
更新状況: apm-data リポジトリの更新日表示は 2024-11-02。現在の更新頻度は未確認。

### 3.2 ファイル構成 (確認済)

| パス | 内容 |
|---|---|
| `v3/list.json` | 全データファイルのマニフェスト (更新時刻を保持) |
| `v3/core.json` | AviUtl 本体と拡張編集 |
| `v3/packages/<developerID>.json` | 開発者単位のパッケージ集合 (`{ version: 3, packages: [...] }`) |
| `v3/scripts.json` | スクリプト配布先の探索情報 |
| `v3/convert.json` | v1/v2 の ID から v3 の ID への対応 |
| `*.min.json` | 同内容の圧縮版 |

パッケージ ID: `<developerID>/<packageName>` (例 `oov/PSDToolKit`)。

### 3.3 パッケージ項目 (確認済。二次資料の記述。JSON 実体は未取得)

| キー | 型 | 必須 |
|---|---|---|
| `id` | string | 必須 |
| `displayName` | `{ja, en}` | 必須 |
| `description` | `{ja, en}` | 必須 |
| `latestVersion` | string | 必須 |
| `downloadURLs` | `[{ url, directURL? }]` | 必須 |
| `releases` | `[{ version, integrity: {target, algorithm, hash} }]` | 必須 (新しい順) |
| `files` | `[{ source, destination }]` | 必須 |
| `dependencies` | `[id]` | 任意 |
| `conflicts` | `[id]` | 任意 |
| `pageURL` | string | 任意 |
| `note` | `{ja, en}` | 任意 |

- `integrity.algorithm` は `sha384` / `sha512` / `sha256`。`hash` は Base64。形式 `sha384-<base64>`。
- `files.destination` は `aviutl.exe` のあるディレクトリからの相対パス。
- `files.source` はアーカイブ展開後のルートからの相対パス。最上位が単一ディレクトリの場合はその内側が基準。
- `downloadURLs.url` は GitHub の場合 `/releases/latest`。`directURL` は個別 asset の URL。

### 3.4 帰結

- ライセンス: データは非営利限定。ランチャー本体 (GPL-3.0) とデータは別配布物。ランチャーはデータを同梱せず実行時に取得する。
- 導入先はインスタンスの `app/` (= `aviutl.exe` の位置)。`plugins/` 独立ディレクトリは AviUtl 無印では使用しない。§4.1 を変更する。

## 4. NeoUtl (Codeberg)

### 4.1 リポジトリ (確認済)

| 項目 | 値 |
|---|---|
| Codeberg | `https://codeberg.org/TeamNeoUtl/NeoUtl` |
| GitHub ミラー | `https://github.com/taisho-guy/NeoUtl` |
| 公式サイト | `https://neoutl.taisho-guy.org` |
| ライセンス | AGPL (本文) / Wiki は CC BY-SA 4.0 |
| API | Forgejo。`https://codeberg.org/api/swagger` (エンドポイント実呼び出しは未実施) |

### 4.2 Release (確認済)

| tag | 区分 | 日付 | asset |
|---|---|---|---|
| `0.8.11` | Stable | 2026-09-26 | `neoutl-linux-x86_64.zip` (109 MiB) / `neoutl-windows-x86_64.zip` (103 MiB) |
| `0.0.95-Unstable` | Pre-release | 2026-05-28 | `AviQtl-Linux-x86_64.zip` / `AviQtl-macOS-ARM64.zip` / `AviQtl-Windows-x86_64.zip` |

- asset 名規則 (Stable): `neoutl-<os>-<arch>.zip`。`<os>` = `linux` / `windows`、`<arch>` = `x86_64`。
- macOS の Stable asset: なし。
- Release 数: 2 (tag は 216)。

### 4.3 導入手順 (確認済。Wiki は AviQtl 由来の記述を含むと自己申告)

| OS | 内容 |
|---|---|
| Windows | ZIP 展開。依存同梱。`NeoUtl` を実行 |
| Linux | 依存 (Qt6 全般 / LuaJIT / Vulkan 実装 / FFmpeg / Carla) を別途導入。ZIP 展開後 `NeoUtl` に実行権限を付与して実行 |
| macOS | 現行 Stable の提供なし |

- 対象: Linux x86_64 / Windows x86_64。
- プラグイン: AviUtl のプラグインとは非互換。互換レイヤーの予定なし。拡張方式は WGSL / Rust (比較表の記述)。
- 技術スタック表記: Home は Slint / wgpu、Installation は Qt6。Wiki 内で不一致。

### 4.4 帰結

- Provider は Forgejo Releases から `prerelease = false` の最新を選び、asset 名 `neoutl-{os}-{arch}.zip` に一致するものを取得する。
- macOS ホストでは NeoUtl の作成を無効化する。
- Linux ホストでは起動前に依存の存在確認を行わない (配布元の依存一覧は更新される)。起動失敗時にログを表示する。

## 5. PLAN.md への差分

| 節 | 変更 |
|---|---|
| 全体 | 製品名を `MLAYN-Launcher` に統一 |
| §2 | 入力表を実体に基づく内容へ更新 |
| §5.5 | YAML 426 件 / 有効 412 件、複数カテゴリの分割、`isEnabled` の扱い |
| §5.2 | AviUtl2 本体の取得元 = `direct` の setup 実行ファイル。YMM4 の取得元 = GitHub のみ |
| §6 | `run` アクションの確認ダイアログ、昇格 (`elevate`) の扱い |
| §10 | 変更なし |

## 6. 要確認 (Phase 0 後に残る項目)

| # | 内容 | 閉じる方法 | 影響する Phase |
|---|---|---|---|
| 1 | YMM4 Release の asset 名 4 件と通常版 / Lite 版の区別 | Phase 2 で Releases API の実応答を確認 | 2 |
| 2 | `.ymme` が Release asset にあるリポジトリの割合 (GitHub 296 件) | Phase 2 で全件走査 | 2 |
| 3 | `.ymme` が階層を持つ事例の有無 | 走査時に ZIP の中身を集計 | 3 |
| 4 | AviUtl2 setup の `-F` `-S` の意味、`-I` 指定時の `{dataDir}` | 実機または公式配布元の説明を確認 | 5 |
| 5 | `elevate: true` の要否 | 同上 | 5 |
| 6 | APM `v3/*.json` の実体 | `cdn.jsdelivr.net` から取得して確認 | 5 |
| 7 | Forgejo API (`/api/v1/repos/TeamNeoUtl/NeoUtl/releases`) の応答 | Phase 2 で確認 | 2 |
| 8 | AviUtl2 の Linux / macOS (Wine) での動作 | Phase 6 | 6 |
