---
name: user-browser
description: 当需要使用用户自己的 Chrome 浏览器（带上他已登录的账号、Cookie、标签页）读取真实网页、点开链接、填表或提取登录后内容时，必须立刻阅读我。它驱动用户本机正在运行的 Chrome，操作发生在用户看得见的窗口里。
---

# 使用用户的浏览器

用 `open-browser-use` 驱动**用户自己的 Chrome**。工作发生在用户眼前那个窗口，登录态就是他自己的，不需要另建 profile，也不需要他重新登录。

## 前置检查

```bash
obu version            # 命令在，看一眼版本
obu ping               # 返回 pong 才算真连通
```

`obu` 与 `open-browser-use` 是同一个程序，短名更好敲。没有这个命令时才走安装流程，见文末「首次接入」。

## 连接模型

- `obu` 是短命客户端，连上 Chrome 扩展后执行、打印、退出；**扩展和 Chrome 常驻**，所以页面、标签、登录态在你退出后都还在。
- 不需要挂在后台 shell 里常驻，也不需要 MCP。每条命令独立可用。
- 多次调用默认共用同一会话 `obu-cli`，`open-tab` / `claim-tab` 定下的默认标签会被后续命令沿用。

先探已连通的实例再动手，不要重复装或重复 setup：

```bash
obu ping && obu info      # info 里能看到扩展 id 与版本
```

同一台机器上可能有别的会话（PAI 的其它会话、Claude、Codex）已经在用同一个扩展。**不要重复 setup、不要重装**：setup 会重写原生消息宿主注册，正常情况一次就够；重复跑只会打扰用户。用不同 `--session-id` 分标签组即可，彼此不干扰。

## 命令

只读与定位：

```bash
obu user-tabs                                   # 用户已经开着的标签（含 id、标题、URL）
obu tabs                                        # 本会话的标签
obu open-tab --url <url>                        # 新开标签，返回 tab.id
obu claim-tab --tab-id <id>                     # 接管用户已有的某个标签
obu navigate --tab-id <id> --url <url>
obu history --query "关键词" --limit 20          # 查用户浏览历史
```

页面内容与脚本：

```bash
obu cdp --tab-id <id> --method Runtime.evaluate \
  --params '{"expression":"document.title","returnByValue":true}'
```

`cdp` 能发任何 Chrome DevTools Protocol 命令，点击、填表、滚动、截图都靠它，见下节。

一次做完多个动作用动作计划，省得反复起进程：

```bash
obu run -c '
open-tab https://example.com
wait-load domcontentloaded
page-info
finalize-tabs []
'
```

`run` 支持的动作只有：`ping` `info` `tabs` `user-tabs` `turn-ended` `open-tab` `claim-tab` `navigate` `wait-load` `page-info` `history` `cdp` `call` `move-mouse` `wait-file-chooser` `set-file-chooser-files` `finalize-tabs`。

**没有 click、fill、snapshot**。想要这些就直接用 `cdp`，不要在 `run` 里试。

收尾：

```bash
obu finalize-tabs --keep '[]'                   # 关掉本会话开的、没交接的标签
```

## 用 cdp 做交互

在页面里执行一段脚本，返回值必须可序列化：

```bash
# 读正文
obu cdp --tab-id <id> --method Runtime.evaluate \
  --params '{"expression":"document.body.innerText.slice(0,3000)","returnByValue":true}'

# 点击
obu cdp --tab-id <id> --method Runtime.evaluate \
  --params '{"expression":"document.querySelector(\"#submit\").click();1","returnByValue":true}'

# 填输入框并触发事件
obu cdp --tab-id <id> --method Runtime.evaluate \
  --params '{"expression":"(()=>{const i=document.querySelector(\"input[name=q]\");i.value=\"关键词\";i.dispatchEvent(new Event(\"input\",{bubbles:true}));return 1})()","returnByValue":true}'

# 截图，落盘后再看
obu cdp --tab-id <id> --method Page.captureScreenshot --params '{"format":"png"}'
```

要点：

- 先定位再动手。用 `querySelector` 取不到就先返回页面上有哪些可交互元素，别盲点：

```bash
obu cdp --tab-id <id> --method Runtime.evaluate --params '{"expression":"[...document.querySelectorAll(\"a,button,input\")].slice(0,40).map(e=>e.tagName+\" \"+(e.innerText||e.placeholder||\"\").slice(0,30)).join(\"\\n\")","returnByValue":true}'
```

- 返回值带 `returnByValue:true` 才会给出内容；不带只回对象句柄，读不到东西。
- 只取需要的片段，`slice` 截断。整页正文直接灌进上下文既慢又没用。
- 脚本返回 `undefined` 时看不到结果，最后补个 `;1`。
- 点击后用 `location.href` 或 `document.title` 核对确实跳转了，不要假定成功。

## 参数转义的坑

在 Git Bash / PowerShell 里手拼那串 JSON，引号会被 shell 先吃掉一层。踩过两次：一次整段被当命令、一次转义不合法。稳妥做法是把参数写进文件再喂进去：

```bash
obu cdp --tab-id <id> --method Runtime.evaluate --params "$(cat params.json)"
```

`params.json` 内容：

```json
{"expression":"document.title","returnByValue":true}
```

临时文件放进项目的 `.pai/temp/`，用完删掉。

## 权限与边界

这个扩展在用户浏览器里申请了很宽的权限：读取所有网站、调试器、下载、历史记录、标签分组。也就是说——

- **用户点得到、你也点得到**。扩展驱动的是他自己那个窗口，没有锁定机制。任何「用户不可操作」的说法在这里都不成立。
- 只做用户明确要求的事。不要顺手读与他请求无关的标签、历史或登录信息。
- 不要在页面里替用户提交支付、删除、发帖、修改账号设置这类不可逆操作，除非他明确说了要这么做。
- 涉及用户真实账号的敏感页面，先把要做什么说清楚，再动手。
- 不读取、不导出 cookie 内容。需要身份就用他现成的登录态，别把票据搬走。

## 收尾

- 自己开的标签用 `finalize-tabs` 清掉，别留在用户浏览器里。
- 接管（`claim-tab`）来的标签，`finalize-tabs --keep '[{"tabId":<id>,"status":"handoff"}]'` 交还，**不要关掉用户正在看的页面**。
- 关标签前按 `user-tabs` 返回的实际 id 和 URL 核对，不要凭记忆猜 id。
- 临时脚本文件清掉。

## 首次接入：引导用户启用扩展

只有当 `obu` 不存在或 `obu ping` 连不上时才走这里。**安装和注册可以由你做，启用扩展必须用户自己点。**

1. 装命令行与原生宿主（会写注册表，需要用户知情）：

```bash
npm i -g open-browser-use
obu setup --no-open
```

2. 告诉用户下一步要他做什么，把话说全，一次说清：

> 扩展已经装进你的 Chrome 了，但 Chrome 默认把它标成禁用。请打开 `chrome://extensions`，找到 Open Browser Use，把右下角的开关打开。
> 如果列表里没有它，点「加载已解压的扩展程序」，选 `open-browser-use setup beta` 生成的目录。
> 它申请的权限包括读取所有网站、调试器、下载、历史记录，启用前请先看清楚。

3. 用户说启用好了之后，再验证：

```bash
obu ping      # 期望 pong
obu info      # 期望看到 extensionId 与版本
```

`obu ping` 仍然失败时，按这个顺序查，**不要靠猜**：

```bash
obu                          # 无参数会打印扩展状态与下一步命令
obu profiles                 # 能看到装了扩展的 Chrome profile 才说明宿主注册对了
```

- `obu` 报「no connectable socket」：扩展没启用，或 Chrome 需要重启。
- `obu profiles` 能看到 profile 但 `ping` 失败：Chrome 开着但没让扩展启动，重启 Chrome。
- 注册表项在 `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.ifuryst.open_browser_use.extension`，值指向宿主清单 json。
- 扩展被 Chrome 标记 `disable_reasons: [8192]` 表示「未在设置中启用」，只能是用户手动打开，没有命令行绕过。

不要在用户没同意的情况下用窗口自动化去替他点那个开关。

## 不做

- 不做自动化测试、批量爬取、可重复流水线。
- 不主动重装、不重复 `setup`，也不清用户的扩展。
- 不动用户没提到的标签页，不导出 cookie，不读取无关的历史记录。
