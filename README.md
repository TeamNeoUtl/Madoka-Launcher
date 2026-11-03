<p align="center"><img src="assets/splash.svg"></p>

# Madoka Launcher とは

ゆっくりMovieMaker 4（YMM4） / AviUtl（ExEdit0 / ExEdit2） 専用の、バージョン・プラグイン・インスタンス管理アプリです。

動画編集ソフト本体及び拡張プラグインのダウンロードと整理を簡単に行えます。

Madoka Launcher は完全無料・自由・オープンソースです。広告やトラッキング等は一切ありません。

類似の他アプリとの違いは以下の通りです。
- 複数の動画編集ソフトの管理
- 複数のインスタンスの管理
- Windows 以外に対応

# 対応ソフト

| | YMM4 | YMM4 Lite | ExEdit0 | ExEdit2 | NeoUtl | Freya NLE |
| --- | --- | --- | --- | --- | --- | --- |
| **バージョン管理** | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ |
| **インスタンス管理** | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ |
| **プラグイン管理** | ✅️（[饅頭遣いのおもちゃ箱](https://manjubox.net/ymm4/)参照） | ✅️（[饅頭遣いのおもちゃ箱](https://manjubox.net/ymm4/)参照） | ✅️（[apm-data](https://github.com/team-apm/apm-data) 参照） | ✅️（[aviutl2-catalog-data](https://github.com/Neosku/aviutl2-catalog-data) 参照） | ❌️ | ❌️ |

# ダウンロード・インストール

[launcher.taisho-guy.org](https://launcher.taisho-guy.org) から Madoka Launcher を入手できます。

Windows 及び Linux に対応しています。

# ビルド

| OS | 開発環境 |
| --- | --- |
| Linux | Arch Linux 系推奨（Ubuntu も多分通る） |
| Windows | MSYS2 推奨（MSVC も多分通る） |

`cargo build --release` だけ。

`git`, `rust` 等は各自インストールしてください。

# よくある質問

<details><summary>バージョン管理とは</summary>

動画編集ソフトウェア本体のインストール、更新、削除等を簡単に行えます。

</details>

<details><summary>インスタンス管理とは</summary>

複数の実行環境を一箇所で作成、実行、削除等できます。

</details>

<details><summary>プラグイン管理とは</summary>

プラグインや拡張等のインストール、更新、削除等を簡単に行えます。

</details>

<details><summary>Madoka Launcher は ゆっくりMovieMaker や AviUtl の公式ですか</summary>

いいえ。Madoka チームは ゆっくりMovieMaker や AviUtl とは直接関係のない、有志による非営利チームです。

</details>

<details><summary>Windows 以外でも AviUtl や YMM4 を動作させることができますか</summary>

Madoka Launcher は現在、Wine 等の互換レイヤーや QEMU 等のエミュレーターによる Windows 以外の OS における Windows 専用のソフトウェアを動作させる機能は搭載しておりません。

NeoUtl や Freya NLE は Linux や macOS に対応していますから、これらをご活用下さい。

</details>

<details><summary>プラグインや本体の更新をどの様に検出しているのですか</summary>

| ソフト名 | 参照元 |
| --- | --- |
| ExEdit0 | AviUtlのお部屋、AviUtl Package Manager 提供のリポジトリ |
| ExEdit2 | AviUtlのお部屋、AviUtl2 カタログ提供のリポジトリ |
| YMM4 及び YMM4 Lite | 饅頭遣いのおもちゃ箱、GitHub 上のリポジトリ |
| NeoUtl | Codeberg 上のリポジトリ |
| Freya NLE | GitHub 上のリポジトリ |

AviUtl については、既存のプラグインマネージャーと同じリポジトリを参照します。

全ての対応ソフトウェアについて Madoka は独自のデータベースを持ちませんから、最新への追従が容易になっています。

</details>