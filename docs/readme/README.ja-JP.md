# Easy Call AI

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-24C8D8?logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.0-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-1.0-000000?logo=rust)](https://www.rust-lang.org)

**言語**
[中文](../../README.md) | [English](README.en-US.md) | [日本語](README.ja-JP.md) | [한국어](README.ko-KR.md)

---

Easy Call AI は、日常PC作業に溶込むデスクトップAI助手です。
「又一本聊天站点」ではなく、热键一つで呼出、直ちに問題解決することを目的とします。

## 開発背景

> 多くAI工具は高機能であるが、実運用では以下課題がある：

| 痛点 | 説明 |
|------|------|
| 切替頻繁 | 頻繁ブラウザ・タブ・アプリ間切替必要 |
| 設定分散 | 供給者・模型設定分散し管理困難 |
| 対話喪失 | 長対話履歴追跡困難 |
| 体験分断 | デスクトップ環境とAI体験分離 |

**Easy Call AI** は、摩擦軽減し、AIをデスクトップ標準機能の如く使用できる体験を目指す。

## 対象者

- 作業中AI頻繁質問者
- 複数供給者・複数模型使分者
- 必要時直速呼出使用者
- 対話履歴・記憶継続活用希望者

## 主機能

- **全局热键** — 対話窓口呼出・非表示
- **托盘常駐** — 設定／対話／書庫／終了
- **複数LLM設定** — 複数供給者・模型保存
- **図文分離** — 対話AI・視覚AI独立設定
- **工具呼出** — 検索／取得／記憶
- **複数人格** — 異AI人格切替
- **流式出力** — 実時応答・思考表示
- **画像貼付** — 多様態消息保存
- **自動書庫** — 対話履歴追跡可能
- **多言語UI** — 中／英／日／韓

## 典型用途

- 文書・誤謬・画面写直質問
- 任務応模型直切替
- 長対話自動書庫整理
- 記憶機能連続協作品質向上

## 速用開始

<details>
<summary>使用手順展開</summary>

> 起動後、画面右下系統托盘確認。応用图标常駐于此、右clickで設定窓口開放。

1. 応用起動後、先ず設定窓口開放
2. `LLM` 标签内API設定追加保存
3. `対話` 标签内「対話AI／視覚AI／AI人格」選択
4. 呼出热键で対話窓口開放、質問開始使用
5. 過去内容は書庫窓口確認

</details>

## 隐私・データ

- API Key・設定本地端末保存
- 対話／書庫／記憶データ既定本地保存
- 缓存清除・書庫削除・データ輸出可能

## 許諾

本計画採用 [GNU General Public License v3.0](../../LICENSE)。