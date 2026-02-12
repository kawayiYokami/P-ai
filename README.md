# Easy Call AI

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-24C8D8?logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.0-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-1.0-000000?logo=rust)](https://www.rust-lang.org)

**语言 / Languages**
[中文](README.md) | [English](docs/readme/README.en-US.md) | [日本語](docs/readme/README.ja-JP.md) | [한국어](docs/readme/README.ko-KR.md)

---

Easy Call AI 是一个面向日常电脑场景的桌面 AI 助手。
它的目标不是"做一个聊天网站"，而是让你在任何时候都能用一个热键，把 AI 叫到屏幕边上，快速解决当下问题。

## 为什么要做这个

> 很多 AI 工具都很强，但在真实使用中常见几个痛点：

| 痛点 | 说明 |
|------|------|
| 切来切去 | 需要频繁打开网页、切标签、复制粘贴上下文 |
| 配置分散 | 不同供应商、不同模型难以统一管理 |
| 对话易丢 | 聊到一半上下文混乱，历史不好回看 |
| 体验割裂 | 和系统环境脱节，不能像"随叫随到的助手" |

**Easy Call AI** 让 AI 更像你的桌面能力，而不是另一个网页应用。

## 适合谁

- 经常边工作边提问 AI 的用户
- 需要管理多个模型/多个 API 配置的用户
- 喜欢"随手呼出、快速问完就收起"工作流的用户
- 希望长期保留对话与记忆，而不是一次性聊天的用户

## 主要功能

- **全局热键** —— 呼出/隐藏对话窗口
- **托盘驻留** —— 配置 / 对话 / 归档 / 退出
- **多 LLM 配置** —— 保存多套供应商与模型
- **图文分离** —— 对话 AI、图转文 AI 独立配置
- **工具调用** —— 搜索 / 抓取 / 记忆
- **多人格** —— 可切换不同 AI 设定
- **流式输出** —— 实时显示思考内容
- **图片粘贴** —— 多模态消息存储
- **自动归档** —— 对话历史可追溯
- **多语言** —— 中 / 英 / 日 / 韩界面

## 典型用途

- 看文档、报错、截图时，直接呼出窗口提问
- 不同任务切不同模型，不用反复改参数
- 长对话自动归档，保留可追溯记录
- 让 AI 记住你的长期偏好，提高连续协作效率

## 快速开始

<details>
<summary>点击展开使用步骤</summary>

> 启动后请检查屏幕右下角系统托盘，应用图标常驻于此，右键可打开配置窗口。

1. 打开应用后先进入配置窗口
2. 在 `LLM` 标签中添加并保存你的 API 配置
3. 在 `对话` 标签中选择"对话AI / 图转文AI / AI人格"
4. 用呼唤热键打开对话窗，输入问题开始使用
5. 需要回看时，到归档窗口查看历史会话

</details>

## 隐私与数据

- 你的 API Key 和本地配置保存在本机
- 对话、归档、记忆数据默认保存在本地配置目录
- 你可以随时清理缓存、删除归档、导出数据

## 许可证

本项目采用 [GNU General Public License v3.0](LICENSE)。
