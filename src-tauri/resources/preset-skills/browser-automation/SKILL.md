---
name: browser-automation
description: 用 agent-browser 做浏览器代理自动化。适用于打开网页、快照页面、按元素引用点击和填表、等待页面变化、截图、导出 PDF、会话隔离等场景。核心流程是 open -> snapshot -i -> 用 ref 操作 -> 页面变化后重新 snapshot。
---

# Agent Browser

- 参考来源：`https://clawhub.ai/TheSethRose/agent-browser`
- 上游 CLI：`https://github.com/vercel-labs/agent-browser`
- 当前 skill 采用 agent-browser 工作流，不再优先使用 playwright-cli。

## 规则
- 浏览器任务优先走固定流程：`open -> snapshot -i -> interact -> re-snapshot`。
- 只有拿到快照里的元素引用后，才进行点击、填写、选择、上传等操作。
- 页面跳转、弹窗、异步加载、明显 DOM 变化后，必须重新 `snapshot -i`。
- 输入框默认优先用 `fill`，不要先 `type` 再赌已有内容会被清空。
- 不确定页面是否稳定时，显式加 `wait`，不要盲点。
- CLI 不可用就直接说明，不要假装成功。

## 安装
- 推荐：
```bash
npm install -g agent-browser
agent-browser install
agent-browser install --with-deps
```

- 从源码：
```bash
git clone https://github.com/vercel-labs/agent-browser
cd agent-browser
pnpm install
pnpm build
agent-browser install
```

## 核心流程
1. `agent-browser open <url>`
2. `agent-browser snapshot -i`
3. 从输出里拿元素引用，例如 `@e1`、`@e2`
4. 用引用执行点击、填写、选择、上传、滚动
5. 页面变化后重新 `agent-browser snapshot -i`

## 常用命令

### 导航
```bash
agent-browser open https://example.com
agent-browser back
agent-browser forward
agent-browser reload
agent-browser close
```

### 快照
```bash
agent-browser snapshot
agent-browser snapshot -i
agent-browser snapshot -c
agent-browser snapshot -d 3
agent-browser snapshot -s "#main"
```

### 交互
```bash
agent-browser click @e1
agent-browser dblclick @e1
agent-browser hover @e1
agent-browser focus @e1
agent-browser fill @e2 "hello"
agent-browser type @e2 "world"
agent-browser press Enter
agent-browser select @e3 "value"
agent-browser check @e4
agent-browser uncheck @e4
agent-browser scroll down 500
agent-browser scrollintoview @e1
agent-browser upload @e5 file.pdf
```

### 等待
```bash
agent-browser wait @e1
agent-browser wait 2000
agent-browser wait --text "Success"
agent-browser wait --url "/dashboard"
agent-browser wait --load networkidle
```

### 信息获取
```bash
agent-browser get text @e1
agent-browser get html @e1
agent-browser get value @e1
agent-browser get attr @e1 href
agent-browser get title
agent-browser get url
```

### 截图与导出
```bash
agent-browser screenshot
agent-browser screenshot path.png
agent-browser screenshot --full
agent-browser pdf output.pdf
```

### 会话
```bash
agent-browser --session test1 open https://example.com
agent-browser --session test2 open https://example.org
agent-browser session list
```

### 调试
```bash
agent-browser open https://example.com --headed
agent-browser console
agent-browser errors
agent-browser highlight @e1
agent-browser trace start
agent-browser trace stop trace.zip
```

## 什么时候重新 snapshot
- 点击后发生跳转
- 弹出新内容、折叠面板、模态框
- 表单提交后页面刷新
- 过滤器、分页、排序后列表重绘
- 你怀疑旧 ref 已经过期

## 提示词
- `使用 agent-browser 完成浏览器任务，严格遵守 open -> snapshot -i -> 用 ref 操作 -> 页面变化后重新 snapshot 的流程。`

## 安全提醒
- `https://clawhub.ai/TheSethRose/agent-browser` 当前带有安全提示，安装前先审查来源。
- 全局安装前先确认 npm 包和 GitHub 仓库可信。
- 不要在敏感站点上随意执行上传、截图、录屏、导出等操作。

## 输出
- 说明用了哪些命令。
- 说明关键页面状态、元素引用和操作结果。
- 如果失败，说明失败发生在哪一步。
- 需要时附上截图、PDF、trace 或快照摘要。
