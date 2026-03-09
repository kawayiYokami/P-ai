# 20260309 预设 Skill 执行归档

## 1. 原始目标
- 在现有 Skill 工作区机制之上，引入“预设 Skill”概念。
- 让应用自带一批可直接同步到工作区的 Skill 模板。
- 初始方向围绕新闻检索与浏览器自动化展开，后续按产品需要继续扩展。

## 2. 最终落地结果
- 预设 Skill 已落到 src-tauri/resources/preset-skills/ 。
- Skill 工作区初始化不再依赖 README 模板，而是直接同步预设 Skill 本体。
- 预设 Skill 会在工作区初始化或重置时自动恢复。
- 当前已落地的预设 Skill 包括：
  - news-analyst
  - browser-automation
  - skill-setup
  - mcp-setup
  - workspace-guide

## 3. 关键实现变化
### 3.1 资源目录
- 预设 Skill 统一放在：src-tauri/resources/preset-skills/
- 每个预设 Skill 单独目录存放，内部至少包含 SKILL.md

### 3.2 工作区同步
- src-tauri/src/features/skill/workspace.rs 已改为直接同步预设 Skill。
- 不再向 llm-workspace/skills/ 写入 README.md 。
- 若历史遗留 README.md 存在，会在初始化时清理。

### 3.3 MCP 与 Skill 说明收口
- 原先分散的 README 说明已收口为预设 Skill：
  - skill-setup 负责说明如何自行安装与编写 Skill。
  - mcp-setup 负责说明如何自行安装与配置 MCP。
- mcp-setup 已补齐目录结构、servers 裸定义、policies 格式、开关规则与推荐 MCP。

### 3.4 Browser 方向
- browser-automation 已改为直接指向 Playwright CLI 官方仓库与最新 README。
- 预设中强调优先遵循官方安装和使用方式，而不是自创浏览器工作流。

### 3.5 新闻方向
- news-analyst 采用独立预设 Skill 形式落地。
- 内容强调新闻时效性、多源对齐、信息脱水与结构化输出。
- 已修正 UTF-8 BOM 问题，并补了解析器容错，避免前端读取失败。

### 3.6 工作空间守则
- 新增 workspace-guide，不再把 LLM 只当成编程工具人。
- 内容覆盖：
  - 编程
  - 教学
  - 查资料
  - 分析
  - 写计划
  - 归档结果
- 同时约定工作空间目录、Git 使用方式、工具链偏好、Plan 与 Archive 习惯。

## 4. 偏离原计划但被采纳的扩展
- 初始计划只写两个预设 Skill，最终扩展为五个，这是主动扩大范围后的结果。
- README 模板路线被放弃，改为“直接把说明包装成可用 Skill”。
- 预设 Skill 不再只是展示入口，而是默认同步到工作区，减少用户手工安装成本。

## 5. 当前产品口径
- 预设 Skill 是随应用分发的默认 Skill 模板。
- 它们通过现有 Skill 工作区机制生效，不引入新的运行时体系。
- 工作区重置会恢复默认预设内容。
- MCP 与 Skill 的使用说明优先体现在预设 Skill 本体，而不是额外 README。

## 6. 已完成事项清单
- 已建立 preset-skills 资源目录。
- 已落地新闻、浏览器、Skill 安装、MCP 安装、工作空间守则五个预设 Skill。
- 已完成 Skill 工作区初始化逻辑改造。
- 已清理 Skill README 模板依赖。
- 已清理 MCP README 模板依赖。
- 已补 Shell 工作空间重置按钮与后端重置命令。
- 已修复 news-analyst 因 BOM 导致的读取失败问题。

## 7. 后续可继续迭代的方向
- 继续优化各预设 Skill 的文案和触发描述。
- 如有需要，再为特定领域新增新的预设 Skill。
- 后续若要引入“模板市场 / 在线下载 / 版本升级”，可在当前资源目录模式之上继续扩展。

## 8. 归档结论
- “预设 Skill”这一轮已完成从概念规划到默认资源落地。
- 原计划文件可以归档，不再作为进行中的执行计划保留。
