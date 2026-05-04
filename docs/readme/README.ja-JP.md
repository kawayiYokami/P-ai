# P-ai（PAI）

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/badge/Release-0.9.69-6366f1)](https://github.com/kawayiYokami/P-ai/releases)

**Languages / 言語**
[简体中文](../../README.md) | [繁體中文](README.zh-TW.md) | [English](README.en-US.md) | [日本語](README.ja-JP.md)

---

> **開箱直結の自己成長型デスクトップ AI ワークシステム — 部門委任、長期記憶、ツール審査、MCP、高並行ワークスペース自動化。**

---

PAI は継続的に進化するデスクトップ AI ワークシステムです。チャットクライアントではなく、会話、タスク、記憶、部門、ツール、審査、リモートメッセージを中心に構成された完全なデスクトップシステムです。バックエンドは Rust の非同期並行とストリーミングアーキテクチャでレスポンス速度を保証し、フロントエンドは Vue 3 + DaisyUI でシンプルなインターフェースを維持します。すべてのデータはローカルに保存され、中間サーバーを経由しません。

### エントリーと効率

ホットキー召喚、音声ウェイクアップ、バックグラウンド音声入力、クイックスクリーンショット — PAI はデスクトップ AI のアクセスポイントを「いつでも召喚、いつでも処理、いつでも続行」に仕上げました。ローカルセッション、リモートセッション、複数セッション並行をサポートし、クイックコマンドでよく使う操作をワンキーで起動できます。

### 組織と人格

複数の部門と人格を独立設定でき、各人格にアバターとプライベートメモリを持たせられます。タスクとセッションは部門・身分・職責ごとに分かれ、ローカルセッションはマルチエージェント同時グループチャットを、リモートセッションは WeChat、飛書、釘釘、OneBot などのプロトコルをサポートします。

### インターフェースとインタラクション

UI、チャットスタイル、配色、フォントはすべてカスタマイズ可能で、複数ウィンドウを並行表示できます。レスポンスが速く、インターフェースはクリーンだが簡素すぎません。

### 能力とツール

完全な能力セットがプリインストールされています：LLM は操作スクリプトでパソコンを制御でき、リアクションを能動的に送信できます。よく使う Skill は内蔵済み。フル画像テキスト変換、PDF や Office のネイティブ読み取りに対応。ツールの変更は元に戻せ、ツール実行とコード変更はマルチ角度 AI 審査を受けられます。API プロバイダーの接続は簡素化されており、開箱直結です。

### 記憶とコンテキスト

長い会話は動的に圧縮・アーカイブされ、単一セッションは無期限に持続できます。コンテキストは継続的な圧縮と整理によって有効性を保ちます。記憶システムは低コストで網羅的 — 使うほどに AI があなたを理解するようになります。

### エンジニアリングと信頼性

高性能、並行対応、高速レスポンス。ローカルセッションはメッセージ配信、セッション分岐、手動委任をサポート。リモートセッションはファイルと画像の送受信に対応。内蔵の能動計画モード、委任システム、人物システムにより、LLM は MCP、スキル、人格、部門を自律管理できます。ツール実行には審査チェーンがあり、コード変更はマルチ角度で検証できます。

---

### 実際の使用シナリオ

以下は仮定ではなく、実際に起きたことです：

- v0.8 から、PAI は PAI 自身の開発に 1 ヶ月以上使われ、407 コミット、496 ファイル変更を生成
- ユーザーが PAI を継続的に財務分析とニュース監視に使用し、3 ヶ月以上経過
- ユーザーが WeChat リモート連絡先を通じて PAI で小紅書コンテンツを 3 ヶ月以上셍산し、累計千件以上の投稿を更新
- ユーザーが PAI で研究論文を 2 ヶ月以上分析し、複数の論文を発表
- ユーザーが PAI で定期的なウェブスクレイピングを実行し、累計 500M 以上のデータを蓄積
- ユーザーが PAI を 20 時間連続稼働させ、プログラミングタスクを実行 — 自己審査、自己解決、オンライン調査を経て最終的に合格
- ユーザーが PAI を長期的にゲーム攻略作成に使用
- ユーザーが PAI を長期的にゲーム操作とデイリータスク完了に使用
- ユーザーが数十のセッションを同時に起動し、PAI で複数のオンラインチャンネルを同時監視
- 長期使用後、ユーザーは一貫して「使うほどにスムーズになる」「AI が自分をより理解するようになる」と報告

---

### プロジェクトデータ

- 872 コミット、116 リリース
- 79 件の計画ドキュメント
- Vue、Rust、Tauri 2 にまたがるフルスタック継続進化
- ローカルセッション、リモートセッション、記憶、審査、委任、マルチウィンドウ、ワークスペース機能がすべて実装済み

---

## 技術スタック

- デスクトップシェル：Tauri 2
- バックエンド：Rust（非同期、tokio）
- フロントエンド：Vue 3 + TypeScript + Vite
- UI：DaisyUI + Tailwind CSS
- パッケージマネージャー：pnpm

## プラットフォームとアップデート

現在のリリース戦略：

- Windows：NSIS インストーラー + zip ポータブル版（`PORTABLE` マーカー）、アプリ内自動更新
- Linux：`.deb` / `AppImage`、リリースパイプライン維持

## データとプライバシー

- API キーはローカルに保存され、中間サーバーを経由しません
- 会話、タスク、アーカイブ、記憶、メディアはすべてローカル保存
- ポータブル版のデータは実行ファイルと同じ階層の `data/` に保存 — USB ドライブから即利用可能
- すべてのデータを自分で管理、エクスポート、削除できます

## 誰向けか

- AI をデスクトップワークフローに本気で埋め込みたい開発者
- 「チャットしかできない」AI ツールに満足できない人
- ワンショット Q&A ではなく、長期タスク実行が必要な人
- 盲目的な委任ではなく、AI に審査能力を持ってほしい人
- AI の組織的協働に想像力を持つ人

## クイックスタート

[Releases](https://github.com/kawayiYokami/P-ai/releases) からインストーラーまたはポータブル版をダウンロード。

インストール後の主要ファイル配置：

- 実行ファイル：`/usr/bin/p-ai`
- デスクトップエントリー：`/usr/share/applications/p-ai.desktop`
- アイコン：`/usr/share/pixmaps/p-ai.png`
- デフォルトデータディレクトリ：`~/.config/p-ai/`

## 謝辞
n本プロジェクトは、これらの優れたアップストリームプロジェクトとコミュニティに支えられています：[Tauri](https://tauri.app/) · [Vue 3](https://vuejs.org/) · [DaisyUI](https://daisyui.com/) · [Tailwind CSS](https://tailwindcss.com/) · [rust-genai](https://github.com/jeremychone/rust-genai) · [rmcp](https://github.com/modelcontextprotocol/rust-sdk) · [Shiki](https://shiki.style/) · [Mermaid](https://mermaid.js.org/) · [KaTeX](https://katex.org/) · [markstream-vue](https://www.npmjs.com/package/markstream-vue) · [tokio](https://tokio.rs/) · [reqwest](https://github.com/seanmonstar/reqwest) · [rusqlite](https://github.com/rusqlite/rusqlite) · [tantivy](https://github.com/quickwit-oss/tantivy) · [Linux.do](https://linux.do/) · [AstrBot](https://github.com/AstrBotDevs/AstrBot)

プロジェクト作者は AstrBot エコシステム向けに 3 つのプラグインも開発しています：[AngelHeart](https://github.com/kawayiYokami/astrbot_plugin_angel_heart)（インテリジェントグループチャット） · [AngelMemory](https://github.com/kawayiYokami/astrbot_plugin_angel_memory)（階層記憶検索） · [AngelSmile](https://github.com/kawayiYokami/astrbot_plugin_angel_smile)（ステッカー管理）

本プロジェクトにアイデア、テスト、フィードバック、コードを貢献してくださったすべての方に感謝します。

## ライセンス

本プロジェクトは [GNU General Public License v3.0](../../LICENSE) を採用しています。
