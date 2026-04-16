# π师傅

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-Desktop-000000?logo=rust)](https://www.rust-lang.org)

**语言 / Languages**  
[简体中文](README.md) | [繁體中文](docs/readme/README.zh-TW.md) | [English](docs/readme/README.en-US.md) | [日本語](docs/readme/README.ja-JP.md)

---

> 把 LLM 从“聊天网页”推进成“长期工作的桌面 AI 系统”。

π师傅是一个 Windows 优先、同时支持 Linux 发布的桌面 AI 助手。  
它不是只会问答的聊天框，而是一个带有：

- 全局热键呼出
- 托盘常驻
- 多人格 / 多部门 / 多模型协作
- 任务与督工
- Skill 与 MCP 工作区
- 自动归档与长期记忆
- 工具执行、审批与审查

的桌面 AI 工作中枢。

如果你想要的是“问一句答一句”的界面，这个项目可能不是重点。  
如果你想要的是“能长期推进任务、管理上下文、管理工具边界、管理组织结构”的桌面 AI，这就是它要做的事。

## 0.8 之后，它进化成了什么

如果你很久没看这个项目，最容易低估的就是：  
它已经不是“桌面聊天 + 几个工具”了。

从 `0.8` 往后，这个项目连续做出了几条非常重的能力跃迁：

- **远程 IM 接入正式成型**
  - 个人微信
  - OneBot / NapCat
  - 钉钉 Stream
  - 联系人级收发权限、激活策略、后台入队与会话回流

- **聊天从单轮问答进化成长期协作系统**
  - 多会话并行
  - 会话级 Todo
  - 计划模式
  - 督工任务
  - 长对话自动归档与压缩

- **工具链从“能调”进化到“可审、可追溯、可批量评估”**
  - 终端审批
  - `apply_patch` 审批
  - 工具审查模型
  - 批次评估
  - 最终审查报告

- **工作区从“目录概念”进化到“会话级运行环境”**
  - 会话主工作目录
  - `AGENTS.md` 自动注入
  - 私有部门 / 私有人格 / 私有 Skill 刷新

- **模型接入层完成重构**
  - 全量迁移到 `rust-genai`
  - 接入 Codex 协议与账号登录
  - 多供应商、多角色模型分工更稳定

- **桌面体验已经非常不像“套壳网页”**
  - 自定义 Markdown 渲染链
  - 图片预览支持缩放与拖拽
  - Windows 安装版 / 便携版自动更新
  - 大量围绕流式、任务、审批、归档的细节打磨

一句话说：

> 它已经从“桌面 AI 客户端”进化成“带任务系统、组织系统、工作区系统、审查系统、远程渠道系统的桌面 AI 平台”。  

## 现在它已经能做什么

### 1. 真正驻留在桌面的 AI

- 全局热键唤起 / 隐藏聊天窗
- 系统托盘常驻
- 独立配置窗、聊天窗、归档窗
- 不需要把工作切走到浏览器里

### 2. 不只是一个助手，而是一套组织

- 主助理 + 部门 + 人格
- 支持把任务委派给下属角色
- 支持后台协作与回流汇报
- 支持私有部门、私有人格、私有工作区

这意味着它不是“一个模型硬扛所有事”，而是开始具备组织化协同能力。

### 3. 不是一次性问答，而是长期任务推进

- 任务创建、追踪、提醒、完成
- 督工任务与计划模式
- 会话级 Todo 与任务状态胶囊
- 长期事项可以分阶段推进
- 会话丢失、归档、恢复都有配套链路

### 4. 不只是会调工具，还会审工具

- `shell_exec` / `apply_patch` 支持审批
- 可配置“工具审查模型”
- 支持单工具评估、批量评估、最终审查报告
- 支持原始变更预览、补丁预览、审查意见回看
- 支持终端与补丁结果写回工具消息，便于后续追踪

项目的目标不是盲目放工具，而是让工具调用可解释、可追溯、可控。

### 5. 不只是本地聊天，还能接远程渠道

- 支持远程 IM 渠道接入
- 支持联系人级收发控制
- 支持激活策略、冷却与自动发送决策
- 远程消息可以进入统一会话与任务链路

### 6. Skill + MCP 工作区

- 支持预设 Skill
- 支持工作区内安装 / 编写 / 刷新 Skill
- 支持 MCP 工具接入
- 能按人格 / 部门边界控制工具能力
- 支持会话主工作目录 `AGENTS.md` 自动注入提示词

这让 AI 的能力扩展不是“堆插件”，而是有运行时边界、有组织归属的系统化扩展。

### 7. 长期记忆、自动归档、上下文治理

- 长对话自动归档
- 上下文压缩与整理
- 低成本记忆回灌
- 会话与归档并行存在
- 后端统一计算上下文占用

项目不追求“无限塞上下文”，而是追求“长期活着且成本可控”。

### 8. 多模型、多供应商统一运行

当前后端统一适配多种 API 形态，包括：

- `openai`
- `anthropic`
- `gemini`
- `openai_tts`

同时支持：

- 多 API 配置
- 不同角色使用不同模型
- 流式输出
- 工具调用与上下文拼装
- Codex 协议与账号登录
- 多模态输入与图片回退治理

### 9. 桌面交互体验已经重做过很多轮

- 聊天窗口流式体验与动画收口
- 自定义 Markdown 渲染链
- 图片预览支持缩放 / 拖拽
- 本地文件链接定位打开
- 输入面板指令系统
- 聊天工具栏、Todo 浮层、模型切换等交互持续打磨

## 一个典型工作流

### 临时协作

1. 热键呼出聊天窗
2. 粘贴文本、图片、截图或附件
3. 让 AI 直接回答、分析或调用工具
4. 完成后收起窗口，继续当前工作

### 长期事项推进

1. 让 AI 创建任务或计划
2. 设定持续推进目标
3. 由主助理或部门持续跟进
4. 需要时自动回顾上下文与历史改动

### 开发 / 运维辅助

1. AI 调用终端或补丁工具
2. 工具结果进入审查链路
3. 用户查看评估意见、补丁内容、最终审查报告
4. 再决定是否继续提交、继续修改或中断

## 为什么它和普通 AI 客户端不一样

很多 AI 产品的问题，不是模型不够强，而是系统层太薄：

| 常见问题 | 结果 |
|---|---|
| 只有聊天，没有任务 | AI 只能回答，不能持续推进 |
| 没有稳定身份和组织 | 所有事都塞给一个角色 |
| 没有长期工作区 | 每轮都像失忆重开 |
| 工具没有治理 | 容易失控，也难追责 |
| 记忆全靠堆上下文 | 成本高、稳定性差 |

π师傅的方向正相反：

- 给 AI 身份
- 给 AI 部门
- 给 AI 委派链
- 给 AI 工作区
- 给 AI 工具边界
- 给 AI 审查与归档
- 给 AI 长期记忆

## 技术栈

- 桌面壳：Tauri 2
- 后端：Rust
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

- Windows 安装版：NSIS
- Windows 便携版：zip + `PORTABLE` 标记文件
- Linux：`.deb` / `AppImage`

当前应用内自动更新覆盖：

- Windows 安装版
- Windows 便携版

Linux 目前保留发布构建链路，但不走应用内自动更新。

## 数据与隐私

- API Key 默认保存在本地
- 对话、任务、归档、工作区、媒体等数据默认保存在本地
- 便携版可切换到可执行文件同级 `data/` 目录
- 你可以自行管理、导出、清理这些数据

## 适合谁

- 想把 LLM 真正放进桌面工作流的人
- 不满足于“只会聊天”的 AI 工具的人
- 需要长期任务推进与上下文治理的人
- 希望 AI 有组织能力、委派能力、审查能力的人
- 喜欢自己塑造 Skill / MCP / 人格 / 部门体系的人

## Arch Linux 安装（yay）

如果你在 Arch Linux / Manjaro 上，希望从本项目仓库直接安装：

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

这个项目能走到今天，离不开这些重要依赖和上游项目：

- [Tauri](https://tauri.app/) 与相关插件：桌面壳、托盘、全局快捷键、更新能力
- [Vue 3](https://vuejs.org/)：前端响应式基础
- [DaisyUI](https://daisyui.com/) 与 [Tailwind CSS](https://tailwindcss.com/)：界面系统
- [markstream-vue](https://www.npmjs.com/package/markstream-vue)、[stream-markdown](https://www.npmjs.com/package/stream-markdown)、[Shiki](https://shiki.style/)、[Mermaid](https://mermaid.js.org/)、[KaTeX](https://katex.org/)：流式 Markdown 与代码 / 图表渲染
- [genai](https://github.com/jeremychone/rust-genai)：当前多模型接入层的重要基础，帮助项目统一连接不同模型供应商
- [rmcp](https://github.com/modelcontextprotocol/rust-sdk) 与 MCP 生态：工具协议接入
- [rusqlite](https://github.com/rusqlite/rusqlite)、[tantivy](https://github.com/quickwit-oss/tantivy)、[hayro](https://crates.io/crates/hayro)：本地数据、全文检索与搜索能力基础
- [reqwest](https://github.com/seanmonstar/reqwest)、[tokio](https://tokio.rs/)：网络与异步运行时
- [async-openai](https://github.com/64bit/async-openai) 与相关模型生态库：模型连接层的重要组成

也感谢所有为本项目贡献想法、测试、反馈和代码的人。

## 许可证

本项目采用 [GNU General Public License v3.0](LICENSE)。
