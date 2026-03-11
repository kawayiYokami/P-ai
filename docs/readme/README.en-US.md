# Easy Call AI

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-24C8D8?logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.0-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-1.0-000000?logo=rust)](https://www.rust-lang.org)

**Languages**  
[简体中文](../../README.md) | [繁體中文](README.zh-TW.md) | [English](README.en-US.md) | [日本語](README.ja-JP.md)

---

> Turn LLMs into a desktop AI command center that actually gets work done.  
> This is not just a chat overlay. It is a desktop AI operating system that can manage itself, manage subordinates, manage skills, manage workspace state, and keep executing over time.

Easy Call AI is not trying to be another AI chat website.

It is trying to do something far more ambitious:

- summon AI to the edge of your desktop with one hotkey
- give AI stable personas, stable departments, stable skills, stable MCP tools, and a stable workspace
- let AI do more than answer questions: accept tasks, track tasks, delegate tasks, and collaborate over time
- make memory sustainable, traceable, archivable, and much cheaper than brute-force context stuffing

If you want a talking webpage, this is not the point.  
If you want a desktop AI system that can keep working with you for the long run, this is it.

## What It Is Now

Easy Call AI is closer to a combination of:

- a desktop AI overlay
- a long-running task system
- a department / worker delegation system
- a Skill + MCP workspace
- a low-cost persistent memory system
- a conversation system with auto-archive and context organization

In one sentence:

> It is a summonable, delegatable, executable, accumulative desktop AI work hub.

## Why It Is Different

Most AI products are limited not because the model is weak, but because the system design is primitive:

| Common problem | What it causes |
|------|------|
| Chat only, no task model | AI can answer, but cannot keep work moving |
| One identity, no organization | Everything is forced into one assistant |
| No stable workspace | Every round feels like a partial reset |
| Expensive and messy memory | Context grows heavier and more fragile |
| Blurry tool boundaries | Hard to control, hard to reuse |

Easy Call AI goes in the opposite direction:

- give LLMs identity
- give LLMs departments
- give LLMs subordinates
- give LLMs skills
- give LLMs tool boundaries
- give LLMs long-term workspace state
- give LLMs cost-aware persistent memory

The goal is not to make AI feel more like a chatbot.  
The goal is to make AI feel more like a living work system.

## Core Capabilities

### 1. One-hotkey desktop AI
- summon / hide chat instantly with a global hotkey
- live in the system tray
- work at the edge of your screen without breaking your current flow

### 2. Unified multi-model runtime
- manage multiple providers and model setups
- assign different models to different responsibilities
- unify streaming, tool calling, context handling, and archiving in one backend pipeline

### 3. A real task system
- AI can create, maintain, and complete tasks
- support run-at, recurring, and persistent tracking flows
- task board, task detail, and current tracked task
- long-running work can move in stages instead of being forgotten after one reply

### 4. Department and worker delegation
- the main assistant can delegate work to subordinate departments
- departments can keep delegating downward
- background execution does not block foreground chat
- results flow back through the chain

That means AI is no longer one mind trying to do everything.  
It starts behaving like an organization.

### 5. Skill workspace
- built-in preset skills
- install, write, refresh, and inspect skills inside the workspace
- the Skill page can display both `SKILL.md` summary and full body
- migrate public skill ideas into local project-native skills

### 6. MCP tool ecosystem
- connect MCP tools
- govern tool access by persona and department boundaries
- treat tools as runtime capabilities instead of a random pile of integrations

### 7. Private departments and personas
- AI can have its own private organization space beyond the main config
- private departments, private personas, and private skills can be refreshed into runtime
- keep the main assistant and private workspace evolving together without cramming everything into one global file

### 8. Efficient, low-cost memory
- active recall
- memory categories
- private memory
- auto-archive for long conversations
- context organization and compression
- backend-driven context usage calculation

The point is not infinite context.  
The point is keeping AI alive for the long run without burning money.

## Example Workflows

### Quick question
1. Summon the chat window with a hotkey
2. Ask, paste, or screenshot
3. Let AI answer or call tools
4. Hide it and keep working

### Long-running task
1. AI creates a task
2. Set target time, recurring interval, or persistent goal
3. Let the system remind and track progress
4. Update status stage by stage instead of “done and gone”

### Department collaboration
1. Main assistant receives work
2. Delegates to a department / worker
3. Subordinate executes independently
4. Result flows back upward
5. Main assistant delivers the final response

### Workspace evolution
1. AI maintains skills, private personas, and private departments in its workspace
2. Refresh brings them into runtime
3. The organization evolves with real work over time

## Who It Is For

- people who want LLMs deeply embedded into desktop workflows
- people who are not satisfied with one-turn chat
- people who need long-running task execution
- people who want AI with organization, delegation, and memory
- people who want to shape their own AI system instead of living inside someone else’s product rules

## Quick Start

1. Launch the app
2. Open Config from the system tray
3. Add and save your LLM API config
4. Choose the main assistant department model and assignee persona
5. Set the summon hotkey
6. Start working from the chat window
7. Expand the system further through Tasks, Departments, Skills, and MCP

## What Has Already Been Built

- desktop AI overlay
- auto-archive
- unified tool-calling pipeline
- task system and task board
- department delegation / worker workflow
- private organization workspace refresh
- preset and custom skill workspace
- MCP integration and capability governance
- context organization
- backend-driven context usage calculation

## Privacy & Data

- API keys stay on your machine by default
- conversations, archives, tasks, memory, and workspace data are stored locally by default
- you can manage, export, and clean your data yourself

## License

This project is licensed under [GNU General Public License v3.0](../../LICENSE).
