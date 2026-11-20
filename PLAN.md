| 優先度 | 機能 | 上流の実装 | Madoka の現状 |
|---|---|---|---|
| 1 | 前提以外のパッケージ関係の表示 | `relations` の 6 種(requires, recommends, conflicts, similar, replaces, forkOf)を詳細画面に表示 | `Package.dependencies` のみ。競合・代替・派生は表示も保持もしない |
| 2 | 導入前の注意事項 | マニフェストの `notice`(Markdown)を導入前モーダルで表示し、確認後に導入 | `notice` を読まない。非推奨と未検証の確認のみ |
| 3 | アンインストール手順と `run`/`delete` ステップ | `uninstallSteps`(delete、run)と、導入手順の `run`(args、管理者昇格) | `Step` は Extract、ExtractSfx、RunAuoSetup、Copy、Place の 5 種。削除は記録済みファイルの消去のみ |
| 4 | 導入済み非推奨パッケージの一覧ダイアログ | 起動時に一覧表示し、個別削除と「再表示しない」を提供 | バッジ、非表示切替、導入時の警告のみ。導入済み一覧の通知はない |
| 5 | 既存 AviUtl2 フォルダの取り込み | 初期セットアップの「インストール済み」分岐。exe を検出して版を取得 | 実装済み。新規作成画面でフォルダを選択し、コピーしてインスタンス化。exe のハッシュで版を判定(不一致は `-`)し、既知プラグインを登録 |
| 6 | Markdown の画像・HTML・コード装飾 | 画像、`<details>`、コードのシンタックスハイライト、HTML のサニタイズ | 画像は代替テキスト、HTML は破棄(PLAN 6-20 で既知) |
| 7 | カタログ本文のロケール別配信 | `list`/`detail` を 5 言語別に配信し、不足時は fallback ロケール | UI のみ 5 言語。カタログ本文は単一言語 |
| 8 | パッケージ登録フォーム | バージョン、画像、手順、ライセンス、ドラフト保存、ソース束インポート、テスト、送信 | JSON 貼付と手順表示のみ。保存・送信・画像・ドラフトなし |
| 9 | ローカルモード | ローカル `manifest.json` のパスを設定に保存し、カタログとして常用 | 貼付登録は終了時に破棄(PLAN §6 の判断) |
| 10 | 構成インポートでの削除同期 | インポート内容に合わせて導入と削除を同時実行し、導入・削除・未登録 ID・失敗を集計 | 不足分の導入のみ。構成に無いパッケージの削除はなし |
| 11 | リンク集ページ | 公式、コミュニティ、サポート、開発者向け、リポジトリの 5 区分 | ページなし |
| 12 | フィードバック送信と診断添付 | 不具合報告と問い合わせ、`app.log` とデバイス情報の添付 | ページなし。ログ閲覧の UI も未検出 |

優先度 10 の「削除同期」は、確認範囲では未検出という扱いです。

## 採用しない方がよいもの
- **匿名統計と外部指標(人気・トレンド)**: 上流は `package_state` で ID を送信し、`metrics` で人気とトレンドを配信します。Madoka は README で「トラッキング等は一切なし」とし、PLAN 5-14 でも外部指標を使わないと決めています。
- **`UpdateChecker.aui2`**: PLAN 1-1 で対象外です。
- **フィードバック送信(優先度 12)**: 外部エンドポイントが必要で、トラッキング不使用の方針と衝突しうるため、採否の判断が必要です。

## macOS 対応
- **対象**: Freya NLE(arm64 / x86_64)と MacUtl(arm64 限定、macOS 27.0 以降)。NeoUtl は MacUtl が担う。
- **配置**: データは `~/Library/Application Support/Madoka-Launcher`、キャッシュは `~/Library/Caches/Madoka-Launcher`、ログは `~/Library/Logs/Madoka-Launcher`。標準ディレクトリ以外の根(テスト等)は根の下に置く。
- **バンドル**: `.app` を丸ごと保持する。実行ファイルは `Info.plist` の `CFBundleExecutable`、無ければ `Contents/MacOS` の既知の名前で解決する。更新は旧 `.app` を退避して新 `.app` を置き、失敗時は戻す。展開後に `codesign --verify` を行い、失敗なら ad-hoc 署名を付ける。
- **アーカイブ**: 相対かつ展開先内に収まる symlink を許可する。`__MACOSX/`、`._*`、`.DS_Store` は除外する。複製・取り込み・差し替えは symlink を保持する。
- **起動前検査**: `LSMinimumSystemVersion` が実行中の macOS より新しい場合は起動せず、必要な版を表示する。起動失敗時は Gatekeeper の案内を添える。
- **ポータブルモード**: 無効。設定画面のスイッチも出さない。
- **自己更新**: 無効。更新の通知とリリースページの表示のみ行う。
- **ディープリンク**: `Info.plist` の `CFBundleURLTypes` が `madoka` と `aviutl2-catalog` を登録する。受信は Apple Event で、`launcher-macos` が既存のローカルソケットへ転送する。`unsafe` は同クレートのみ許可し、各ブロックに安全性の根拠を記す。
- **配布**: `scripts/package-macos.sh` が `Madoka Launcher.app` を組み立て、ad-hoc 署名して zip 化する。`release.yml` は `macos-arm64` と `macos-x86_64` を作る。CI は `macos-latest` で fmt、clippy、test を実行する。
- **未検証**: 実機での起動、Gatekeeper の挙動、Apple Event の受信、`with_download_started_handler`。

