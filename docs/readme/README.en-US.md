# P-ai (PAI)

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/badge/Release-0.9.69-6366f1)](https://github.com/kawayiYokami/P-ai/releases)

**Languages / 语言**
[简体中文](../../README.md) | [繁體中文](README.zh-TW.md) | [English](README.en-US.md) | [日本語](README.ja-JP.md)

---

> **A self-growing desktop AI work system — ready-to-use, with agent delegation, long-term memory, tool review, MCP, and high-concurrency workspace automation.**

---

PAI is an actively evolving desktop AI work system. It is not a chat client — it is a complete desktop system organized around conversations, tasks, memory, departments, tools, review, and remote messaging. The backend uses Rust async concurrency and streaming architecture to guarantee response speed; the frontend uses Vue 3 + DaisyUI for a clean interface. All data is stored locally, with no intermediate servers.

### Entry & Efficiency

Global hotkey summon, voice wakeup, background voice input, quick screenshot — PAI brings desktop AI access to "summon anytime, handle anything, continue anywhere." Supports local sessions, remote sessions, and multiple parallel sessions; quick commands can trigger common operations in one keystroke.

### Organization & Personas

Multiple departments and personas can be independently configured, each with its own avatar and private memory. Tasks and sessions are separated by department, identity, and responsibility. Local sessions support multi-agent group chat; remote sessions support WeChat, Feishu, DingTalk, OneBot, and other protocols.

### Interface & Interaction

UI, chat style, colors, and fonts are all customizable, with multiple windows running in parallel. Fast response, clean but not bare-bones.

### Capabilities & Tools

A complete capability set is pre-built: LLM can execute operation scripts to control the computer and send reactions proactively; common Skills are built-in; full image-to-text, native PDF and Office reading are supported; tool modifications are reversible; tool execution and code changes can undergo multi-angle AI review. API provider onboarding is streamlined and ready to use.

### Memory & Context

Long conversations are dynamically compressed and archived; a single session can persist indefinitely, with context staying effective through continuous compression and organization. The memory system is low-cost and comprehensive — the more you use it, the better the AI understands you.

### Engineering & Reliability

High performance, concurrent, fast to respond. Local sessions support message delivery, session branching, and manual delegation; remote sessions support sending and receiving files and images. Built-in proactive planning mode, delegation system, and persona system; LLM can autonomously manage MCP, skills, personas, and departments. Tool execution has a review chain; code changes can be validated from multiple angles.

---

### Real Usage Scenarios

The following are not hypotheticals — they actually happened:

- Starting from v0.8, PAI has been used to develop PAI itself for over 1 month, producing 407 commits and 496 file changes
- Users have been using PAI continuously for financial analysis and news monitoring for over 3 months
- Users have been using PAI via WeChat remote contacts to produce Xiaohongshu content for over 3 months, with over a thousand published posts
- Users have been using PAI to analyze research papers for over 2 months and published multiple papers based on it
- Users have been using PAI for scheduled web scraping, accumulating over 500M of data
- A user ran PAI continuously for 20 hours on a programming task — it reviewed, resolved, researched online, and passed on its own
- Users have been using PAI long-term to create game guides
- Users have been using PAI long-term to operate games and complete daily tasks
- Users run dozens of sessions simultaneously, using PAI to monitor multiple online channels at once
- After extended use, users consistently report it gets smoother over time — the AI understands them better

---

### Project Stats

- 872 commits, 116 releases
- 79 plan documents
- Full-stack evolution across Vue, Rust, and Tauri 2
- Local sessions, remote sessions, memory, review, delegation, multi-window, and workspace capabilities all shipped

---

## Tech Stack

- Desktop shell: Tauri 2
- Backend: Rust (async, tokio)
- Frontend: Vue 3 + TypeScript + Vite
- UI: DaisyUI + Tailwind CSS
- Package manager: pnpm

## Platform & Updates

Current release strategy:

- Windows: NSIS installer + zip portable (`PORTABLE` marker), in-app auto-update
- Linux: `.deb` / `AppImage`, release pipeline maintained

## Data & Privacy

- API keys are stored locally, never passing through any intermediate server
- Conversations, tasks, archives, memory, and media are all stored locally
- Portable version data lives in `data/` next to the executable — plug-and-play from a USB drive
- You can manage, export, and clean up all your data yourself

## Who It Is For

- Developers who want AI truly embedded in their desktop workflow
- People not satisfied with AI tools that "can only chat"
- People who need long-running task execution, not one-shot Q&A
- People who want AI with review capability, not blind delegation
- People with imagination for AI organizational collaboration

## Quick Start

Download the installer or portable version from [Releases](https://github.com/kawayiYokami/P-ai/releases).

Main file locations after installation:

- Executable: `/usr/bin/p-ai`
- Desktop entry: `/usr/share/applications/p-ai.desktop`
- Icon: `/usr/share/pixmaps/p-ai.png`
- Default data directory: `~/.config/p-ai/`

## Acknowledgments

This project relies on these excellent upstream projects and communities: [Tauri](https://tauri.app/) · [Vue 3](https://vuejs.org/) · [DaisyUI](https://daisyui.com/) · [Tailwind CSS](https://tailwindcss.com/) · [rust-genai](https://github.com/jeremychone/rust-genai) · [rmcp](https://github.com/modelcontextprotocol/rust-sdk) · [Shiki](https://shiki.style/) · [Mermaid](https://mermaid.js.org/) · [KaTeX](https://katex.org/) · [markstream-vue](https://www.npmjs.com/package/markstream-vue) · [tokio](https://tokio.rs/) · [reqwest](https://github.com/seanmonstar/reqwest) · [rusqlite](https://github.com/rusqlite/rusqlite) · [tantivy](https://github.com/quickwit-oss/tantivy) · [Linux.do](https://linux.do/) · [AstrBot](https://github.com/AstrBotDevs/AstrBot)

The project author has also developed three plugins for the AstrBot ecosystem: [AngelHeart](https://github.com/kawayiYokami/astrbot_plugin_angel_heart) (intelligent group chat) · [AngelMemory](https://github.com/kawayiYokami/astrbot_plugin_angel_memory) (hierarchical memory retrieval) · [AngelSmile](https://github.com/kawayiYokami/astrbot_plugin_angel_smile) (sticker management)

Thanks to everyone who has contributed ideas, testing, feedback, and code to this project.

## License

This project is licensed under [GNU General Public License v3.0](../../LICENSE).
