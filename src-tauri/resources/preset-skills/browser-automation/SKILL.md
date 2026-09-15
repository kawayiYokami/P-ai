---
name: browser-automation
description: 当需要操作浏览器、自动化网页交互、测试 Web UI 或爬取动态网页时，必须立刻阅读我。
---

# Browser Automation

用 `playwright-cli` 驱动浏览器。浏览器由常驻守护进程持有，每条命令只是一次性客户端——不需要挂后台 shell，也不需要装 Playwright MCP。

## 前置检查

```bash
playwright-cli --help     # 有输出即可用
playwright-cli list       # 当前有哪些浏览器会话
```

装的是全局 `@playwright/cli` 时命令名就是 `playwright-cli`；没有时可退到 `npx playwright cli`。缺命令时先 `npm install -g @playwright/cli@latest`，需要 Node 18+。

## 运行模型

先记住这四条，后面的命令才不会用错：

- `open` 会起一个 daemon，浏览器在 daemon 里；命令本身秒级返回。之后每条命令都是短命客户端，连回同一个 daemon，所以页面、cookie、标签页都还在。
- **在 PAI 里，前台 exec 起的会话撑不过一条命令**：每条前台命令都跑在一个"命令结束就清场"的作业对象里，daemon 会被一起收回。要让会话跨命令常驻，就把 `open` 或 `attach` 放进后台 shell 并让那条命令保持不退出——`playwright-cli -s=x open <url> && sleep 86400`，接管现成浏览器则是 `playwright-cli -s=chrome attach --cdp=chrome && sleep 86400`。daemon 挂在后台任务的作业对象里，之后每条前台命令都能连回它（`list` 会显示该会话 open，直接 `eval` / `click` 即可，不必重新 open/attach，也不会再弹权限确认）。
- 会话按名字隔离：默认 `default`，用 `-s=<名字>` 或环境变量 `PLAYWRIGHT_CLI_SESSION` 切换。
- 会话按工作目录分组，在项目根目录下执行，命令才会落到同一组会话里。

profile 默认只存在内存里，浏览器一关就丢；要跨重启保留登录态，用 `--persistent` 或 `--profile=<目录>`。

## 用哪个浏览器

优先用系统已经装好的浏览器，避免额外下载内核：

| 写法 | 结果 |
|---|---|
| `--browser=chrome` / `--browser=msedge` | **首选**。直接用本机已装的 Chrome / Edge，不需要下载；复用程序，profile 独立 |
| 什么都不加 | Playwright 自带的 chromium，本机缓存里没有对应内核时需要额外下载 |
| `--headed` | 显示窗口，默认是 headless |
| `attach --cdp=chrome` | 接管一个已经开着的浏览器，能带上你现有的登录态。**仅在用户主动要求时使用**：要用户本人在目标浏览器打开 `chrome://inspect/#remote-debugging` 授权，且接管时浏览器会向用户弹权限确认 |
| `detach` | 只断开接管，不关那个外部浏览器 |

本机有 Chrome 或 Edge 就用 `--browser=chrome` / `--browser=msedge`；只有确实需要自带内核、且缓存里缺对应版本时，才 `playwright-cli install-browser <browser>` 下载，不要主动下载。

## 常用命令

观察与定位，交互前必做：

```bash
playwright-cli snapshot                 # 取元素 ref，后续操作用 ref
playwright-cli find "登录"              # 页面大时先搜，只把命中的几条读进来
playwright-cli screenshot --filename=page.png
playwright-cli console [级别]           # 控制台消息
playwright-cli requests                 # 网络请求列表，配 request / response-body 看详情
playwright-cli eval "document.title"
```

交互：

```bash
playwright-cli click e15
playwright-cli fill e5 "user@example.com" --submit
playwright-cli type "搜索词"
playwright-cli press Enter
playwright-cli select e9 "选项值"
playwright-cli check e12   # 或 uncheck e12
playwright-cli hover e4    # 或 drag e2 e8 / upload ./a.pdf / drop e4 --path=./a.png
playwright-cli resize 1280 900
```

目标除了 ref，也可以用 CSS 选择器，或 `getByRole('button', { name: 'Submit' })` 这类 locator。

标签页与导航：

```bash
playwright-cli tab-list
playwright-cli tab-new <url>   # 或 tab-select 0 / tab-close 1
playwright-cli goto <url>      # 或 go-back / go-forward / reload
```

存储与登录态：

```bash
playwright-cli cookie-list --domain=example.com
playwright-cli cookie-get <name>   # 或 cookie-set <name> <value> / cookie-delete <name>
playwright-cli localstorage-list   # 或 localstorage-get <key> / sessionstorage-list
playwright-cli state-save auth.json  # 或 state-load auth.json
```

自定义逻辑，命令覆盖不到的场景用：

```bash
playwright-cli run-code "async page => { return await page.title(); }"
playwright-cli run-code --filename=./script.js
```

必须是单个函数表达式，不支持 `import` / `require`。

辅助工具：

```bash
playwright-cli generate-locator e5 --raw           # 把 ref 变成稳定 locator，写进代码或测试
playwright-cli highlight e5                        # 在页面上标出元素，方便给人指位置
playwright-cli show --annotate                     # 让用户在页面上画框批注，回传批注截图 + 快照 + 说明
playwright-cli recording-start / recording-stop    # 录操作，生成 Playwright 代码
playwright-cli tracing-start / tracing-stop        # 排障轨迹
playwright-cli --raw ...                           # 只输出结果值，便于接管道或做 diff
playwright-cli --json ...                          # 结构化输出
```

## 典型流程

- 只读：`open <url>` → `snapshot`（页面大就 `find`）
- 交互：`snapshot` → `click eN` → `snapshot` 确认结果
- 表单：`snapshot` → `find 输入` → `fill eN "文本"` → `press Enter`
- 对比改动：`--raw snapshot > before.yml` → 操作 → `--raw snapshot > after.yml` → diff
- 收尾：`close`；确认有无残留用 `list`，僵尸用 `kill-all`

## 经验与坑

- 每次导航或重要 DOM 变化后重新 `snapshot`，旧 ref 立即作废。
- 页面越大越要先 `find`，别把几百条 ref 灌进上下文。
- 输入不生效时先 `eval "document.visibilityState"`——窗口被完全遮挡时页面会被判成后台页，注入的键鼠事件不生效；此时不要反复重试。
- 点击卡住点不动（元素可见、位置稳定，却一直在 actionability 检查上等到超时），多半是有浮层遮挡。先 `screenshot` 看一眼；急着推进就绕开点击：读出目标的 `href`，在 `run-code` 里 `page.goto(href)` 直接导航。
- 默认是 headless，遇到站点风控或异常提示时，改用 `--headed` 或 `--browser=chrome` 重试。
- Windows 下 URL 里的 `&` 会被 shell 截断：cmd 用 `^&`，PowerShell 用 `--%`。
- 登录态只有一条正路：独立 profile（`--persistent` / `--profile`）+ 用户亲手登录一次，之后长期复用。不要读取或解密用户的 cookie 内容。
- 前台 exec 里的会话活不过一条命令，要跨命令复用就在后台 shell 里关住它（见上文运行模型）；否则每次都得重新 `open` / `attach`，同一批动作放在一条命令内做完。
- 接管来的浏览器里，关标签前要按 `tab-list` 返回的 URL 核对，不要凭上一次的索引去关——标签顺序会变，猜错就会关掉用户正在看的页面。
- 收尾：自己的会话用 `close`；接管来的外部浏览器用 `detach`，只断开、不关它。
- 客户端比 daemon 旧时会报版本不匹配，按提示重新 `open` 即可。
- 出问题先看 daemon 目录下 `<会话名>.err`：Windows 在 `%LOCALAPPDATA%\ms-playwright\daemon\<工作区哈希>\`，其它平台在系统缓存目录下的 `ms-playwright/daemon/`。
- 分工：桌面坐标级操作走 `operate`，网页交互走这里。

## 不做

- **除非用户主动要求，绝不接管用户正在使用的浏览器**（`attach`）。接管会在用户真实的浏览器里开标签、点页面，还会要求用户逐步确认权限；用户没提就绝不碰。
- 不主动下载浏览器内核，缓存里缺了才装。
- 不用 headless 操作用户账号。
