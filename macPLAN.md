macOS対応の調査を開始する。Freya NLE の macOS 配布形式と MacUtl のリリース構成を確認する。

# macOS 対応 実装計画（Freya NLE / MacUtl 同時対応）

## 1. 調査結果

### Freya NLE
- v0.8.1 に macOS 用アセットが 2 本ある。`Freya-NLE-0.8.1-macos-arm64.zip`（24.8MB）と `Freya-NLE-0.8.1-macos-x86_64.zip`（27.2MB）。
- v0.8.0 と v0.7.0-rc.2 の名称は `Freya-NLE-macOS-Xcode-arm64.zip`、v0.7.0-rc.1 以前は `AviQtl-macOS-Xcode-arm64.zip`。いずれも arm64 のみ。
- 現行の正規表現は 0.8.1 の macOS アセットに一致しない。Intel 版と arm64 版を区別する軸が現状のコードにない。
- **未確認: zip の中身が `.app` かどうか。** 配布は zip で、リリースノートに「a bundle that admits what it needs」とあるため `.app` の可能性が高い。ただし、アセットの中身を列挙できていない。
- 確認手順: 1 本ダウンロードして `unzip -l` する。または実装の最初に `zip_entry_names` で列挙する。

### MacUtl（Ren9618/MacUtl）
- NeoUtl v0.8.11 の非公式 macOS フォーク。AGPL-3.0-or-later。
- Apple Silicon（arm64）専用。macOS 27.0 以降が必須。同梱 FFmpeg の dylib が `minos 27.0` のため。
- 配布は GitHub Releases。タグは `v0.8.11-mac.3` 形式。アセットは `MacUtl-0.8.11-macos.dmg` と `MacUtl-0.8.11-macos.zip`。
- GitHub が `sha256` の digest を報告するため、ハッシュ検証つきで導入できる。
- ad-hoc 署名のみ。自動更新は無効。
- `target/bundle/MacUtl.app` から zip を作る構成なので、zip の中身は `MacUtl.app` と推定する。これも未確認で、同じ手順で確認する。

### 現行コードの macOS 障害点
| 箇所 | 問題 |
|---|---|
| `Host` | Windows / Linux のみ。`current()` は `cfg!(windows)` で分岐する |
| `find_executable` | 深さ 2 までの検索。`X.app/Contents/MacOS/x` は深さ 3 なので到達しない |
| `Archive` | symlink エントリを拒否する。`.app` の Frameworks は symlink を含む |
| `copy_dir` / `copy_tree` / `Swap` | symlink を黙って捨てる。複製・取り込み・更新で `.app` が壊れる |
| `sniff` | Mach-O の magic を知らず、素の実行ファイルを zip と誤判定する |
| `portable_root` | `exe.parent()/data` を使う。`.app` 内の `Contents/MacOS/data` になり不適切 |
| 自己更新 | `.app` の自己置換でコード署名が壊れる |
| URL スキーム | `register_url_scheme` に macOS 実装がない。Info.plist と Apple Event が必要 |
| `open_with_shell` | macOS 用の `open` がない |
| `unsafe_code = "forbid"` | Apple Event 処理に `objc2` の unsafe が必要で、ワークスペース方針と衝突する |

## 2. 方針

### 対応 Kind
| Kind | Windows | Linux | macOS |
|---|---|---|---|
| Neoutl | ✅ | ✅ | ❌（MacUtl が担う） |
| Freya | ✅ | ✅ | ✅ |
| **Macutl（新規）** | ❌ | ❌ | ✅（arm64 のみ） |
| Aviutl / Aviutl2 / Ymm4 | ✅ | ❌ | ❌ |

### 展開先ディレクトリ（macOS）
| 用途 | パス |
|---|---|
| データ根 | `~/Library/Application Support/Madoka-Launcher`（`ProjectDirs` の既定。変更なし） |
| インスタンス | `<根>/instances/<id>/app/<名前>.app`（`.app` を丸ごと保持し、バンドル内を展開しない） |
| キャッシュ・ダウンロード | `~/Library/Caches/Madoka-Launcher` |
| ログ | `~/Library/Logs/Madoka-Launcher` |
| ランチャー本体 | `Madoka Launcher.app`（`/Applications` または `~/Applications`） |
| ポータブルモード | macOS では無効化（`.app` 配置先が読み取り専用・translocation の可能性があるため） |

## 3. 実装フェーズ

### Phase 0: 検証（実装前）
1. Freya 0.8.1 の macOS zip 2 本と MacUtl の zip を列挙する。
2. 確認項目は `.app` 名、`Contents/MacOS` 内の実行ファイル名、symlink の有無、`__MACOSX/` と `._*` の有無、署名状態、unix mode。
3. 結果で Phase 2 の細部を確定する。

### Phase 1: `launcher-core` の型
- `Host::MacOs` を追加し、`current()` を `cfg!(target_os)` による 3 値分岐にする。
- `Arch { X86_64, Aarch64 }` と `Arch::current()` を新設する。
- `AppSelection` に `arch` を追加する。
- `Kind::Macutl`（serde `"macutl"`）を追加し、`executable_stems` は `["MacUtl"]`、`supported_on` は上表のとおりにする。
- `Bundle` は変更しない。Freya の macOS は zip 一択。
- 影響する網羅 `match` をすべて更新する（`runner.rs`、`installer.rs` の `layout_of` / `executable_host`、`strings.rs`、`style.rs`、`new_instance.rs`）。

### Phase 2: アーカイブとバンドル処理（最大の変更）
1. **`.app` 解決**
   - `find_executable` は `Host::MacOs` で `*.app` ディレクトリ（深さ 2 まで）を探す。
   - `Contents/Info.plist` の `CFBundleExecutable`（`plist` クレート）から `Contents/MacOS/<exe>` を解決する。
   - 失敗時は `executable_stems` で `Contents/MacOS` を探す。
   - 素の実行ファイル（Mach-O）も受理する。
2. **インストール根の抽象化**
   - `exe_dir` を `install_root(kind, dir)` に置換する。バンドルなら `.app` ディレクトリ、それ以外は実行ファイルの親を返す。
   - `update_app_checked` の置換先がバンドル全体になる。
3. **symlink 対応（unix）**
   - `Archive::open` は symlink を許可する。ただし相対ターゲットのみ、正規化後に展開先内へ収まるもののみ。それ以外は `SymlinkEntry` で拒否する。
   - `extract` で `std::os::unix::fs::symlink` を作成し、`Created` に記録する。
   - `store::copy_dir`、`stage::copy_tree`、`Tree`（swap）を symlink 保持に変更する。
   - 既存の `install_app_rejects_symlink` 系テストは「範囲外ターゲットは拒否」に更新する。
4. **バンドル更新**
   - `.app` の更新はファイル単位の上書きにしない。旧 `.app` を backup へ rename し、新 `.app` を配置する。失敗時は rename で戻す。
   - 上書きでは旧版の残骸が Frameworks に残るため。
5. **zip 前処理**
   - `__MACOSX/`、`._*`、`.DS_Store` を展開対象から除外する。
6. **署名検証**
   - 展開後に `codesign --verify --deep --strict` を実行する。
   - 失敗時は `codesign --force --deep -s -`（ad-hoc）で再署名する。Apple Silicon は未署名バイナリを kill するため。
   - 隔離属性は付与しない（自プロセス展開では付かない）。
7. `sniff` に Mach-O の magic（`CFFAEDFE`、`CAFEBABE`）を追加する。
8. `open_with_shell` に macOS の `open` を追加する。
9. `is_executable_name` は Linux と同様に拡張子なしで一致させる。

### Phase 3: `launcher-providers`
- **FreyaProvider**
  - macOS の正規表現を追加する（新: `Freya-NLE-.+-macos-(arm64|x86_64)\.zip`、旧: `(Freya-NLE|AviQtl)-macOS-Xcode-arm64\.zip`）。
  - `is_asset_of` に `(Host::MacOs, arch)` を追加する。arch 不一致は `NoMatchingAsset`。
  - `is_any_asset` に macOS を含める。
- **MacutlProvider（新規 `macutl.rs`）**
  - 対象は `Ren9618/MacUtl` の Releases API。
  - 許可アセットは `^MacUtl-.+-macos\.zip$`（dmg は除外）。
  - `plan_install_app` は `Host::MacOs && Arch::Aarch64` のみ許可する。それ以外は `UnsupportedHost`。
  - `version` は tag（`v0.8.11-mac.3`）とし、digest は既存の `Release` 経路で SHA-256 検証する。
  - `catalog` は空、`plan_install_package` は `OpenInBrowser`（Neoutl と同形）。
- `MadokaProvider`、`has_provider`、`has_catalog`（`Macutl` は false）を更新する。
- `ALLOWED_HOSTS` は変更不要（`github.com`、`release-assets.githubusercontent.com` が既登録）。

### Phase 4: パスとランタイム（`launcher-core::paths`、`launcher-app`）
- `cache` / `logs` を `ProjectDirs` の `cache_dir` / `data_local_dir` 由来に分離する（macOS のみ。他 OS は現状維持）。
- `portable_root` と `is_portable` は macOS で `None` を返す。設定画面のポータブルスイッチは非表示にする。
- `logging::init` は新ログディレクトリへ出力する。
- `launcher-browser`: 変更は最小。`wry` は macOS で `WKWebView` を使い、`build(&window)` 経路は既存。`with_download_started_handler` が macOS で動くことを実機で確認する。

### Phase 5: UI・文言
- `KINDS` に `Kind::Macutl` を追加する。`kind_label` は `"MacUtl"`、`kind_icon` / `kind_tone` を設定する。
- `host_label` に `"macOS"` を追加する。`unsupported_host` の表示を更新する。
- 新規キーを `locales/{ja,en,ko,zh-CN,zh-TW}.yml` の 5 ファイルへ同時に追加する。対象は `min_os_unsupported`、`self_update_disabled_macos`、`gatekeeper_hint`。キー集合の一致テストがあるため。
- 既定の `Host` / `Arch` で選択可能な Kind の初期値を `Form::new` が正しく選ぶことを確認する。

### Phase 6: 自己更新・ディープリンク
- **自己更新**: macOS では `self_update` の置換を無効化する。`check` は有効のまま、更新ありを通知して Releases ページを開く。MacUtl と同じ判断。
- **ディープリンク**
  - Info.plist の `CFBundleURLTypes` に `madoka` と `aviutl2-catalog` を登録する。
  - 受信は Apple Event（`kAEGetURL`）で、argv では来ない。ハンドラが既存 IPC チャネルへ push する。
  - `objc2` は unsafe を要するため、新クレート `launcher-macos` に隔離する。`unsafe_code` は同クレートのみ許可し、各 unsafe に安全性の根拠をドキュメントコメントで記す。
  - `register_url_scheme` の macOS 分岐は no-op とする（実行時登録は不可）。
  - IPC ソケットは `GenericNamespaced` 非対応時のファイル fallback が既にあり、変更不要。

### Phase 7: ビルド・配布
- `build.rs` で SVG から `icon.icns` を生成する。
- 新設 `xtask`（または `scripts/package-macos.sh`）で `Madoka Launcher.app` を組み立てる。
  - 構成は `Contents/{Info.plist, MacOS/{Madoka-Launcher,Madoka-Browser}, Resources/icon.icns}`。
  - `codesign -s -` で ad-hoc 署名し、`ditto -c -k --keepParent` で zip 化する。
- `ci.yml` に `macos-latest` を追加する。`release.yml` に `macos-arm64` を追加し、Intel 版は `macos-13` 系で別ジョブにする。
- `.cargo/config.toml` の `crt-static` は Windows のみなので変更不要。

### Phase 8: テスト・文書
- **core**
  - バンドル解決（Info.plist あり・なし・複数 `.app`）。
  - symlink の抽出と、範囲外ターゲットの拒否。
  - symlink を含む複製・取り込み・swap の往復。
  - 更新失敗時のバンドル rollback。
  - `__MACOSX` の除外。
- **providers**
  - Freya の新旧アセット名 × arch 選択。
  - MacUtl のアセット選択と arm64 以外の拒否。
- **app**
  - 5 言語キー一致（既存テスト）。
  - Kind の可用性。
- `README.md` の対応表と OS 表を更新し、ビルド行に macOS を追加する。
- `PLAN.md` に macOS 節を追記する。

## 4. リスク
| リスク | 対処 |
|---|---|
| Freya の zip が `.app` でない可能性 | Phase 0 で確定する。素の実行ファイルでも `find_executable` が受理する設計にしておく |
| MacUtl の macOS 27.0 要件 | 導入時の確認が必須。Info.plist の `LSMinimumSystemVersion` を読み、現在の OS と比較して警告する |
| ad-hoc 署名のバイナリが Gatekeeper に止められる | 自プロセス展開では隔離属性が付かないため実害は小さい。実機で検証する |
| 実機検証手段がない | CI の macOS ランナーで単体テストと `cargo clippy` を実行する。Wine / Rosetta 上の動作は対象外 |

## 5. 確認事項
1. 「ルート直下ではなく適切なディレクトリ」の解釈は、§2 の配置表（Application Support / Caches / Logs）で合っているか。
2. Intel Mac 向けは Freya の x86_64 のみ提供し、MacUtl は arm64 限定でよいか。
3. ランチャー自身の macOS ビルドは、arm64 のみか、Intel も含めるか。

Sources:
- [MacUtl リポジトリ](https://github.com/Ren9618/MacUtl)
- [MacUtl Releases API](https://api.github.com/repos/Ren9618/MacUtl/releases?per_page=5)
- [Freya NLE Releases API](https://api.github.com/repos/GT-610/freya-nle/releases?per_page=5)
