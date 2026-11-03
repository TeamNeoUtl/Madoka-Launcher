<p align="center"><img src="assets/splash.svg"></p>

# Motoyori Launcher とは

ゆっくりMovieMaker 4（YMM4） / AviUtl（ExEdit0 / ExEdit2） 専用の、バージョン・プラグイン・インスタンス管理アプリです。

動画編集ソフト本体及び拡張プラグインのダウンロードと整理を簡単に行えます。

Motoyori Launcher は完全無料・自由・オープンソースです。広告やトラッキング等は一切ありません。

類似の他アプリとの違いは以下の通りです。
- 複数の動画編集ソフトの管理
- 複数のインスタンスの管理
- 他アプリからのインポート
- Windows 以外に対応
- プラグインパックの管理

# 対応ソフト

| | YMM4 | YMM4 Lite | ExEdit0 | ExEdit2 | NeoUtl | Freya NLE |
| --- | --- | --- | --- | --- | --- | --- |
| **バージョン管理** | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ |
| **インスタンス管理** | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ | ✅️ |
| **プラグイン管理** | ✅️（[饅頭遣いのおもちゃ箱](https://manjubox.net/ymm4/)参照） | ✅️（[饅頭遣いのおもちゃ箱](https://manjubox.net/ymm4/)参照） | ✅️（[apm-data](https://github.com/team-apm/apm-data) 参照） | ✅️（[aviutl2-catalog-data](https://github.com/Neosku/aviutl2-catalog-data) 参照） | ❌️ | ❌️ |

# ダウンロード・インストール

[Motoyori.org](https://Motoyori.org) から Motoyori Launcher を入手できます。

Windows 及び Linux に対応しています。

# よくある質問

<details><summary>Motoyori Launcher は ゆっくりMovieMaker や AviUtl の公式ですか</summary>

いいえ。Motoyori チームは ゆっくりMovieMaker や AviUtl とは直接関係のない、有志による非営利チームです。

</details>

<details><summary>「Motoyori」とは</summary>

「**M**od **L**oader for **A**viUtl, **Y**ukkuriMovieMaker, and **N**eoUtl」の略です。

Motoyori の最終目標は、AviUtl 、ゆっくりMovieMaker 、NeoUtl 等で動作する MOD ローダーを開発することです。

これらの動画編集ソフトはお互いに遠い親戚みたいなものですが、それぞれの仕組みが一切異なる為に、プラグイン等のコミュニティ資産を共有できない、という課題があります。

Motoyori は、WASM や WIT 等と言った技術を用いた汎用プラグイン規格・ホストを実装しています。

これにより、プラグイン開発者は、一度プラグインを書き、ビルドするだけで、複数の動画編集ソフト、複数の OS に、同時に対応することが出来ます。又、C++ や C# 等以外の使い慣れた言語で開発することが可能になります。

Motoyori Mod Loader は、現在は非公開で開発されています。

Motoyori Launcher は Motoyori プロジェクトの「始めの一歩」です。

</details>

<details><summary>Windows 以外でも AviUtl や YMM4 を動作させることができますか</summary>

Motoyori Launcher は現在、Wine 等の互換レイヤーや QEMU 等のエミュレーターによる Windows 以外の OS における Windows 専用のソフトウェアを動作させる機能は搭載しておりません。

NeoUtl や Freya NLE は Linux や macOS に対応していますから、これらをご活用下さい。

</details>

<details><summary>バージョン管理とは</summary>

動画編集ソフトウェア本体のインストール、更新、削除等を簡単に行えます。

</details>

<details><summary>インスタンス管理とは</summary>

複数の実行環境を一箇所で作成、実行、削除等できます。

</details>

<details><summary>プラグイン管理とは</summary>

プラグインや拡張等のインストール、更新、削除等を簡単に行えます。

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

全ての対応ソフトウェアについて Motoyori は独自のデータベースを持ちませんから、最新への追従が容易になっています。

</details>