# P-ai（PAI）

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/github/v/release/kawayiYokami/P-ai)](https://github.com/kawayiYokami/P-ai/releases)

**Languages / 語言**
[简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [English](../../README.md) | [日本語](README.ja-JP.md)

---

> **開箱即用的自我成長型桌面 AI 工作系統 — 人格組織與委派、長期記憶、工具審查、MCP、高並發工作區自動化。**

---

PAI 是一個持續演進中的桌面 AI 工作系統。它不是聊天用戶端，而是一套圍繞會話、任務、記憶、人格組織、工具、審查、遠端訊息組織起來的完整桌面系統。底層用 Rust 非同步並發和串流架構保證回應速度，前端用 Vue 3 + DaisyUI 保持簡潔介面。所有資料本機儲存，不經過中間伺服器。

同一套聊天執行時也可在瀏覽器和 VS Code 側邊欄使用。宿主差異只留在傳輸層，產品本身不是桌面端專屬。

### 入口與效率

快捷鍵喚出、Windows 背景語音喚醒（按住錄音鍵）、快速截圖——PAI 把桌面 AI 的入口做到了「隨時喚出、隨時處理、隨時繼續」。支援本機對話、遠端對話、多對話並行，快捷指令可以一鍵觸發常用操作。

### 組織與人格

多種人格可以獨立設定，每套人格帶頭像、帶私有記憶，並以上下級關係組成組織。任務和對話按人格、身份、職責分開，本機對話支援多 Agent 同時群聊，遠端對話支援微信、飛書、釘釘、OneBot、QQ 官方等協定。

### 介面與互動

UI、對話樣式、配色、字型都可以自訂，多視窗並行展開。回應速度快，介面乾淨但不簡陋。Web 端和 VS Code 側邊欄復用同一套聊天介面。

### 能力與工具

預設了完整的能力集：LLM 可以執行操作腳本控制電腦、主動發表情；常見 Skill 已經內建；支援全面圖轉文、PDF 和 Office 原生閱讀、圖像生成；工具修改可回退；工具執行和程式碼修改可以多角度 AI 審查。API 供應商接入做了簡化，開箱即用。LLM 可以自主管理 MCP、技能、人格與組織。

### 記憶與上下文

長對話會動態精簡歸檔，單對話可以長期延續，上下文透過持續壓縮和整理保持有效。記憶系統成本低、覆蓋面全，AI 會越用越懂你。

### 工程與可靠性

高效能、支援並發、回應快。本機對話支援訊息投送、對話分支、目標、定時任務、人工或自主發起委託；遠端對話支援收發檔案和圖片。工具執行有審查鏈，程式碼修改可以多角度校驗。

---

### 真實使用場景

以下不是假設，是實際發生的事：

- 從 v0.8 開始，PAI 就被用來開發 PAI 自身
- 有用戶持續用 PAI 處理財經問題和新聞輿論監督
- 有用戶透過微信遠端聯繫人，用 PAI 生產小紅書文案，累計更新上千條發布
- 有用戶用 PAI 分析研究論文，並在此基礎上發表論文
- 有用戶用 PAI 定時爬取網路資料
- 有用戶讓 PAI 連續工作 20 小時執行一個程式設計任務，自行審查、自行解決、自行查閱網路資料，最終通過
- 有用戶長期用 PAI 製作遊戲攻略
- 有用戶長期用 PAI 操作遊戲完成日常任務
- 有用戶同時開啟數十個對話，用 PAI 同時監控多個網路頻道
- 長期使用後，用戶普遍回饋越用越順，AI 越來越懂自己

---

### 當前形態

- 持續自我迭代，當前發布在 0.97 線
- 桌面應用之外，Web 端和 VS Code 側邊欄已落地
- 本機對話、遠端對話、記憶、審查、委託、任務、多視窗、工作區能力均已落地

## 技術堆疊

- 桌面殼：Tauri 2
- 後端：Rust（非同步，tokio）
- 前端：Vue 3 + TypeScript + Vite
- UI：DaisyUI + Tailwind CSS
- 套件管理：pnpm

## 平台與更新

當前發布策略：

- Windows：NSIS 安裝版 + zip 便攜版（`PORTABLE` 標記），應用內自動更新
- Linux：`.deb` / `AppImage`，保留發布鏈路
- macOS：Intel + Apple Silicon 通用 DMG，保留發布鏈路

應用內自動更新當前僅覆蓋 Windows 安裝版與便攜版。

## 資料與隱私

- API Key 儲存在本機，不經過任何中間伺服器
- 對話、任務、歸檔、記憶、媒體全部本機儲存
- 標準安裝使用系統設定目錄；便攜版資料在可執行檔案同級 `data/`
- 你可以自行管理、匯出、清理所有資料

## 適合誰

- 想把 AI 真正放進桌面工作流的開發者
- 不滿足於「只會聊天」的 AI 工具的人
- 需要長期任務推進，而不是一次一清的人
- 希望 AI 有審查能力，不是盲目放權的人
- 對 AI 組織化協作有想像力的人

## 快速開始

從 [Releases](https://github.com/kawayiYokami/P-ai/releases) 下載安裝版、便攜版、Linux 包或 macOS DMG。

預設資料目錄：

- Windows：`%APPDATA%\easycall\p-ai`
- Linux：`~/.config/p-ai`
- macOS：`~/Library/Application Support/ai.easycall.p-ai`
- 便攜版：可執行檔案同級 `data/`

Linux 上 AUR 包裝為 `easy-call-ai`；官方 `.deb` / `AppImage` 以 Release 產物為準。

## 文件

- [文件索引](../README.md) — 哪些是現狀說明，哪些是歷史文稿
- [AGENTS.md](../../AGENTS.md) — 給編碼協作看的專案約定

## 致謝

這個專案能走到今天，依賴這些優秀的上游專案與社群：[Tauri](https://tauri.app/) · [Vue 3](https://vuejs.org/) · [DaisyUI](https://daisyui.com/) · [Tailwind CSS](https://tailwindcss.com/) · [rust-genai](https://github.com/kawayiYokami/rust-genai) · [rmcp](https://github.com/modelcontextprotocol/rust-sdk) · [Shiki](https://shiki.style/) · [Mermaid](https://mermaid.js.org/) · [KaTeX](https://katex.org/) · [tokio](https://tokio.rs/) · [reqwest](https://github.com/seanmonstar/reqwest) · [rusqlite](https://github.com/rusqlite/rusqlite) · [tantivy](https://github.com/quickwit-oss/tantivy) · [AstrBot](https://github.com/AstrBotDevs/AstrBot)

專案作者還為 AstrBot 生態開發了三款外掛：[AngelHeart](https://github.com/kawayiYokami/astrbot_plugin_angel_heart)（智慧群聊互動） · [AngelMemory](https://github.com/kawayiYokami/astrbot_plugin_angel_memory)（層級記憶檢索） · [AngelSmile](https://github.com/kawayiYokami/astrbot_plugin_angel_smile)（表情包管理）

也感謝所有為本專案貢獻想法、測試、回饋和程式碼的人。

## 授權條款

本專案採用 [GNU General Public License v3.0](../../LICENSE)。
