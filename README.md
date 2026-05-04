# P-ai（PAI）

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/badge/Release-0.9.69-6366f1)](https://github.com/kawayiYokami/P-ai/releases)

**Languages / 语言**
[简体中文](README.md) | [繁體中文](docs/readme/README.zh-TW.md) | [English](docs/readme/README.en-US.md) | [日本語](docs/readme/README.ja-JP.md)

---

> **A self-growing desktop AI work system — ready-to-use, long-running, with agent delegation, memory, tool review, MCP, and high-concurrency workspace automation.**
>
> **开箱即用的自我成长型桌面 AI 工作系统 — 部门委派、长期记忆、工具审查、MCP、高并发工作区自动化。**

---

PAI 是一个持续演进中的桌面 AI 工作系统。它不是一个聊天客户端，而是一套围绕会话、任务、记忆、部门、工具、审查、远程消息组织起来的完整桌面系统。底层用 Rust 异步并发和流式架构保证响应速度，前端用 Vue 3 + DaisyUI 保持简洁界面。所有数据本地存储，不经过中间服务器。

### 入口与效率

快捷键呼出、语音唤醒、后台语音输入、快速截图——PAI 把桌面 AI 的入口做到了「随时呼出、随时处理、随时继续」。支持本地会话、远程会话、多会话并行，快捷指令可以一键触发常用操作。

### 组织与人格

多种部门和人格可以独立配置，每套人格带头像、带私有记忆。任务和会话按部门、身份、职责分开，本地会话支持多 Agent 同时群聊，远程会话支持微信、飞书、钉钉、OneBot 等协议。

### 界面与交互

UI、对话样式、配色、字体都可以自定义，多窗口并行展开。响应速度快，界面干净但不简陋。

### 能力与工具

预设了完整的能力集：LLM 可以执行操作脚本控制电脑、主动发表情；常见 Skill 已经内置；支持全面图转文、PDF 和 Office 原生阅读；工具修改可回退；工具执行和代码修改可以多角度 AI 审查。API 供应商接入做了简化，开箱即用。

### 记忆与上下文

长对话会动态精简归档，单会话可以长期延续，上下文通过持续压缩和整理保持有效。记忆系统成本低、覆盖面全，AI 会越用越懂你。

### 工程与可靠性

高性能、支持并发、响应快。本地会话支持消息投送、会话分支、人工发起委托；远程会话支持收发文件和图片。内置主动计划模式、委托系统、人物系统，LLM 可以自主管理 MCP、技能、人格和部门。工具执行有审查链，代码修改可以多角度校验。

---

### 真实使用场景

以下不是假设，是实际发生的事：

- 从 v0.8 开始，PAI 被用来开发 PAI 自身超过 1 个月，期间产生了 407 次提交、496 个文件变更
- 有用户持续用 PAI 处理财经问题和新闻舆论监督，超过 3 个月
- 有用户通过微信远程联系人，用 PAI 生产小红书文案，超过 3 个月，累计更新上千条发布
- 有用户用 PAI 分析研究论文超过 2 个月，并在此基础上发表了多篇论文
- 有用户用 PAI 定时爬取网络资料，累计超过 500M
- 有用户让 PAI 连续工作 20 小时执行一个编程任务，自行审查、自行解决、自行查阅网络资料，最终通过
- 有用户长期用 PAI 制作游戏攻略
- 有用户长期用 PAI 操作游戏完成日常任务
- 有用户同时开启数十个会话，用 PAI 同时监控多个网络频道
- 长期使用后，用户普遍反馈越用越顺，AI 越来越懂自己

---

### 项目数据

- 872 次提交，116 个版本发布
- 79 份计划文档
- 前后端跨 Vue、Rust、Tauri 2 持续演进
- 本地会话、远程会话、记忆、审查、委派、多窗口、工作区能力均已落地

---

## 技术栈

- 桌面壳：Tauri 2
- 后端：Rust（异步，tokio）
- 前端：Vue 3 + TypeScript + Vite
- UI：DaisyUI + Tailwind CSS
- 包管理：pnpm

## 平台与更新

当前发布策略：

- Windows：NSIS 安装版 + zip 便携版（`PORTABLE` 标记），应用内自动更新
- Linux：`.deb` / `AppImage`，保留发布链路

## 数据与隐私

- API Key 保存在本地，不经过任何中间服务器
- 对话、任务、归档、记忆、媒体全部本地存储
- 便携版数据在可执行文件同级 `data/`，U 盘即插即用
- 你可以自行管理、导出、清理所有数据

## 适合谁

- 想把 AI 真正放进桌面工作流的开发者
- 不满足于"只会聊天"的 AI 工具的人
- 需要长期任务推进、而不是一次一清的人
- 希望 AI 有审查能力、不是盲目放权的人
- 对 AI 组织化协作有想象力的人

## 快速开始

从 [Releases](https://github.com/kawayiYokami/P-ai/releases) 下载安装版或便携版。

安装后主要文件位置：

- 可执行文件：`/usr/bin/p-ai`
- 桌面启动项：`/usr/share/applications/p-ai.desktop`
- 图标：`/usr/share/pixmaps/p-ai.png`
- 默认数据目录：`~/.config/p-ai/`

## 致谢

这个项目能走到今天，依赖这些优秀的上游项目与社区：[Tauri](https://tauri.app/) · [Vue 3](https://vuejs.org/) · [DaisyUI](https://daisyui.com/) · [Tailwind CSS](https://tailwindcss.com/) · [rust-genai](https://github.com/jeremychone/rust-genai) · [rmcp](https://github.com/modelcontextprotocol/rust-sdk) · [Shiki](https://shiki.style/) · [Mermaid](https://mermaid.js.org/) · [KaTeX](https://katex.org/) · [markstream-vue](https://www.npmjs.com/package/markstream-vue) · [tokio](https://tokio.rs/) · [reqwest](https://github.com/seanmonstar/reqwest) · [rusqlite](https://github.com/rusqlite/rusqlite) · [tantivy](https://github.com/quickwit-oss/tantivy) · [Linux.do](https://linux.do/) · [AstrBot](https://github.com/AstrBotDevs/AstrBot)

项目作者还为 AstrBot 生态开发了三款插件：[AngelHeart](https://github.com/kawayiYokami/astrbot_plugin_angel_heart)（智能群聊交互） · [AngelMemory](https://github.com/kawayiYokami/astrbot_plugin_angel_memory)（层级记忆检索） · [AngelSmile](https://github.com/kawayiYokami/astrbot_plugin_angel_smile)（表情包管理）

也感谢所有为本项目贡献想法、测试、反馈和代码的人。

## 许可证

本项目采用 [GNU General Public License v3.0](LICENSE)。