# P-ai（PAI）

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/badge/Release-0.9.50-6366f1)](https://github.com/kawayiYokami/P-ai/releases)

**Languages / 语言**
[简体中文](README.md) | [繁體中文](docs/readme/README.zh-TW.md) | [English](docs/readme/README.en-US.md) | [日本語](docs/readme/README.ja-JP.md)

---

> **A self-growing desktop AI work system — ready-to-use, long-running, with agent delegation, memory, tool review, MCP, and high-concurrency workspace automation.**
>
> **开箱即用的自我成长型桌面 AI 工作系统 — 部门委派、长期记忆、工具审查、MCP、高并发工作区自动化。**

---

## 它是什么

P-ai 不是另一个 AI 聊天客户端。

它默认驻留在你的桌面（Windows/Linux），有全局热键、系统托盘、独立多窗口。但它真正的不同在于：**它内部有一套完整的组织系统**。

主助理、副手部门、远程客服部门、私有人格、私有 Skill —— 这些不是抽象概念，而是实际运行在系统中的协作单元。AI 不只是"回答者"，而是可以委派任务、协同工作、接受审查的组织。

---

## 如果你很久没看这个项目

最容易低估的是：它已经不是一个"聊天框 + 几个工具"了。它已经拥有极其强大而且精细的审查功能，支持单应用多窗口模式、并发稳定运行和独立工作目录。

从 0.8 往后，连续做出了几条重型能力跃迁：

### 工具治理系统 — 领先多数大公司工程团队数月

它不是"能调工具"，而是有完整的工具审查链：终端命令审批、`apply_patch` 补丁审查、可配置的审查专用模型、单工具评估、批次评估、最终审查报告。工具调用结果可追溯、可回看、可审核。AI 不仅执行，还要接受评审。

这种"让 AI 审查 AI 的工具使用"的机制，已经从简单权限弹窗推进到可追溯、可复核、可生成报告的工程化审查链路。

### 多部门委派架构 — 让 AI 像组织一样工作

主助理可以将任务委派给副手部门，远程客服部门自动承接 IM 消息并按办事指南决策。每个部门有独立人格、独立工具权限、独立 Skill 和 MCP。私有人格和私有部门可以按需创建，支持运行时热刷新。

这不是"一个模型换换人设"，而是有真实边界、真实权限、真实委派链的组织化协作。

### 长期记忆与上下文治理 — 不死记硬背，精准检索

长对话自动归档，归档时生成总结并提取长期记忆。记忆通过 RAG 延迟注入提示词，用户画像快照持续维护。上下文压缩走统一 `SummaryContext` 管线，不是粗暴截断，而是结构化整理。

效果是：AI 能记住几个月前的对话，但不会让上下文无限膨胀。

### 工作区感知 — AI 真正理解你在哪个项目里

每个会话可以绑定不同的主工作目录。`AGENTS.md` 自动注入让 AI 立即理解当前项目的编码规范、架构约定、发布流程。工具执行（终端、补丁）在对应工作区上下文中运行，路径自动压缩显示。

这比任何"上传文件到聊天"的体验都更接近真正的开发协作。

### 远程 IM 接入 — AI 进入真实社交网络

个人微信、OneBot/NapCat、钉钉 Stream 已接入。支持联系人级收发权限、激活策略、后台入队、会话回流。远程消息和本地对话走同一套会话、任务、归档链路。

不是"AI 帮你回消息"，而是"你的 AI 组织里有一个部门专门负责远程联络"。

### 高并发架构 — 从第一天就为多任务设计

后端 Rust 异步运行时，供应商级串行请求门、零拷贝热路径、只读快路径缓存、消息 JSONL 分片存储。前端虚拟滚动、首屏轻量快照、流式通道重建。

不是"能跑就行"，而是系统性地压低了每条热路径的延迟和内存开销。

---

## 现在它能做什么

### 桌面常驻

- 全局热键呼出 / 隐藏，系统托盘常驻
- 独立配置窗、聊天窗、归档窗，无边框原生窗口
- Windows 安装版 + 便携版 + Linux .deb/AppImage，应用内自动更新

### 组织化 AI

- 主助理 + 副手部门 + 远程客服部门 + 自建私有部门
- 私有人格 / 私有 Skill / 私有 MCP，运行时热刷新
- 部门级工具权限与办事指南
- 会话绑定部门，部门决定人格、模型、工具范围

### 任务与督工

- 会话级 Todo，督促任务，计划模式
- 长期任务分阶段推进，跨会话追踪
- 暂停 / 恢复 / 完成状态流转

### 工具审查系统

- `shell_exec` / `apply_patch` 审批链路
- 可配置审查专用模型（与对话模型分离）
- 单工具评估 → 批次评估 → 最终审查报告
- 终端/补丁分组展示，路径压缩，原始变更预览
- 审查意见写回工具消息，完整追溯链

### 长期记忆

- 自动归档 + 结构化总结 + 记忆提取
- RAG 延迟注入，用户画像持续维护
- 上下文统一压缩管线，成本可控

### 工作区感知

- 会话主工作目录绑定
- `AGENTS.md` 自动注入提示词
- 路径压缩显示（`easy_call_ai/src/...`）
- 终端/补丁在工作区上下文中执行

### 远程 IM

- 个人微信 / OneBot (NapCat) / 钉钉 Stream
- 联系人级权限与激活策略
- 后台入队，会话回流，统一任务链路

### 模型体系

- OpenAI / Anthropic / Gemini / DeepSeek / Kimi / Codex
- 供应商级并发控制，流式输出
- 思维链推理保留与回传（DeepSeek/Kimi）
- 多模态图片 + 视觉描述降级

### 桌面体验

- 流式 Markdown 渲染（Shiki 代码高亮、Mermaid 图表、KaTeX 数学）
- 图片预览缩放/拖拽，本地文件链接定位打开
- 虚拟滚动、消息多选、草稿保留、队列引导
- 自定义深色/浅色主题生成

---

## 一个典型工作流

### 日常使用

1. 热键呼出，问一个问题，热键隐藏。和 Spotlight 一样快，但回答你的是一个有记忆、有上下文的 AI。

### 开发协作

1. AI 调用终端或 `apply_patch` 修改代码
2. 工具结果进入审查链路
3. 审查模型生成评估意见、批次报告
4. 你审核后决定批准、驳回或继续修改

### 长期推进

1. 创建任务，设定目标
2. AI 分阶段推进，自动回顾历史上下文
3. 长时间未处理的事项自动归档，但记忆保留
4. 任何时候回来都能继续，不需要重述背景

### 远程联动

1. 微信/钉钉收到消息
2. 远程客服部门接管，按办事指南决策
3. 重要事项回流到任务系统
4. 你在桌面看到完整的处理记录和待办

---

## 为什么它和所有 AI 客户端都不一样

大多数 AI 产品的问题不是模型不够强，而是**系统层太薄**：

| 常见缺失 | P-ai 的做法 |
|---|---|
| 只有聊天，没有任务系统 | 会话级 Todo + 督工任务 + 计划模式，AI 能持续推进 |
| 所有事塞给一个角色 | 主助理 + 副手 + 远程客服 + 私有人格，有边界有权限 |
| 每次打开像失忆重开 | 长期记忆 RAG + 自动归档 + 用户画像持续维护 |
| 工具能调但不能审 | 审查模型 + 终端审批 + 补丁审查 + 批次评估 + 最终报告 |
| 上下文无限膨胀 | 统一 SummaryContext 压缩管线，成本可控 |
| 不会真的进工作目录 | 工作区绑定 + AGENTS.md 注入 + 路径感知执行 |
| 和社交网络割裂 | 微信 / OneBot / 钉钉 + 联系人权限 + 激活策略 |
| 一个模型吃所有场景 | 多供应商 + 部门级模型分工 + 审查专用模型分离 |

P-ai 的方向是：**给 AI 身份、部门、委派链、工作区、工具边界、审查机制、长期记忆、远程渠道。**

## 技术栈

- 桌面壳：Tauri 2
- 后端：Rust（异步，tokio）
- 前端：Vue 3 + TypeScript + Vite
- UI：DaisyUI + Tailwind CSS
- 包管理：pnpm

## 构建与开发

```bash
# 开发模式（前端热重载 + Rust 自动编译）
pnpm tauri dev

# 仅前端 dev server
pnpm dev

# 前端类型检查
pnpm typecheck

# Rust 编译检查
cd src-tauri && cargo check

# 前端测试
pnpm test

# Rust 测试
cd src-tauri && cargo test

# Windows 冒烟测试
pnpm smoke

# 生产构建
pnpm build
pnpm tauri build
```

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

### Windows

从 [Releases](https://github.com/kawayiYokami/P-ai/releases) 下载安装版或便携版。

### Arch Linux

```bash
git clone https://github.com/kawayiYokami/P-ai.git
cd P-ai/packaging/arch
chmod +x install-with-yay.sh
./install-with-yay.sh
```

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