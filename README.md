# Easy Call AI

一个基于 `Tauri 2 + Rust + Vue 3 + DaisyUI` 的 Windows 桌面 AI 助手，支持托盘、快捷键呼出、可配置多供应商 API、流式对话与工具调用。

## 功能概览
- 托盘运行与菜单操作（配置 / 对话 / 退出）
- 全局热键唤起与隐藏对话窗口
- API 配置管理（多配置保存、模型刷新、能力开关）
- 人格管理（系统提示词、可切换）
- 对话记录持久化与归档查看
- 流式回复与思维链展示（按窗口 UI 渲染）
- 图片粘贴与多模态消息存储
- 内置工具链（搜索、抓取、记忆）

## 技术栈
- 前端：`Vue 3`、`TypeScript`、`Vite`、`TailwindCSS`、`DaisyUI`
- 桌面端：`Tauri 2`
- 后端：`Rust`
- 包管理：`pnpm`

## 项目结构
- `src/`: 前端代码（配置窗、对话窗、归档窗）
- `src-tauri/`: Rust 后端与 Tauri 命令
- `docs/`: 设计文档与运行说明
- `plan/`: 需求与迭代计划
- `.debug/`: 本地调试配置（已在 `.gitignore` 中忽略）

## 本地运行（Windows）
1. 安装依赖
```bash
pnpm install
```
2. 启动开发模式
```bash
pnpm tauri dev
```

## 开发约定
- 提交信息格式：`<type>: <中文描述>`
- 支持前缀：`feat` `fix` `refactor` `chore` `docs` `style` `test`
- 详细规范见：`CONTRIBUTING.md`

## 许可证

本项目采用 **GNU GPL v3.0 or later（GPL-3.0-or-later）**。

- 允许：使用、修改、分发（包括商业场景）
- 要求：分发衍生作品时需开源对应源码，并继续使用 GPL 许可证

详情请查看：`LICENSE`

