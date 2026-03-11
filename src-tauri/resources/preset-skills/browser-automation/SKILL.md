---
name: browser-automation
description: 用 Microsoft Playwright CLI 官方仓库和最新 README 做浏览器自动化。适用于打开网页、快照、点击、填表、截图、会话隔离等场景。优先按官方文档安装和使用：https://github.com/microsoft/playwright-cli
---

# Playwright CLI

- 官方仓库：`https://github.com/microsoft/playwright-cli`
- 以仓库最新 README 为准。
- 不要自创浏览器工作流，直接用 `playwright-cli`。

## 安装
- `npm install -g @playwright/cli@latest`
- `playwright-cli --help`
- `playwright-cli install --skills`
- 没有全局命令时：`npx playwright-cli --help`
- 如果环境不支持，自行安装 Node.js。

## 常用
- 打开：`playwright-cli open https://example.com`
- 可视：`playwright-cli open https://example.com --headed`
- 跳转：`playwright-cli goto https://example.com`
- 快照：`playwright-cli snapshot`
- 点击：`playwright-cli click e12`
- 填写：`playwright-cli fill e8 "hello"`
- 按键：`playwright-cli press Enter`
- 截图：`playwright-cli screenshot`
- 面板：`playwright-cli show`

## 会话
- 命名会话：`playwright-cli -s=example open https://example.com --persistent`
- 列出：`playwright-cli list`
- 全关：`playwright-cli close-all`
- 强杀：`playwright-cli kill-all`
- 绑定会话：`PLAYWRIGHT_CLI_SESSION=todo-app <agent-command>`

## 规则
- 需要元素引用时，先 `snapshot`。
- 需要隔离时，用 `-s=`。
- 需要持久状态时，用 `--persistent`。
- 需要可见浏览器时，用 `--headed`。
- CLI 不可用就直接说明，不要假装执行成功。

## 提示词
- `使用 https://github.com/microsoft/playwright-cli ，按最新 README 安装并用 playwright-cli 完成任务。`

## 输出
- 说明用了哪些命令。
- 说明当前页面和结果。
- 需要时附上截图或快照。
