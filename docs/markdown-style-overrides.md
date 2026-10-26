# Markdown 渲染架构与样式覆盖说明

PAI 的 Markdown 渲染采用「自研渲染器 + 语义样式全局化 + 场景排版局部化」的架构。渲染器已替代早期 markstream-vue 方案，支持流式增量渲染、shiki 高亮、KaTeX 数学、Mermaid、toolcall 引用组等复杂节点。

## 渲染器架构

### 统一渲染器

核心组件：[AppMarkdownRenderer.vue](../src/features/chat/markdown/AppMarkdownRenderer.vue)

- 统一导出入口：[index.ts](../src/features/chat/markdown/index.ts)
- Props：`text` / `blocks`（二选一输入）、`isDark`、`streaming`、`variant`（`chat` / `document`）、`localImageBasePath`、`toolcallPreviewMap`
- 渲染根节点挂 `ecall-markdown-content` 类，即可复用共享语义样式

### 块解析与流式增量

- 块解析与行内分段：[parse-markdown.ts](../src/features/chat/markdown/parse-markdown.ts)（`MarkdownBlock` / `InlineSegment`）
- 增量解析器：[incremental-markdown.ts](../src/features/chat/markdown/incremental-markdown.ts)（`IncrementalMarkdownBlockParser`，流式输出时逐块暴露）

支持的块类型：`paragraph` / `heading`(1-4) / `quote` / `list`(ordered) / `table` / `code` / `math` / `details` / `footnotes` / `hr`

支持的行内段：`text` / `html_br` / `toolcall_ref` / `footnote_ref` / `code` / `math`(display) / `link` / `image` / `imageLink` / `html_sub` / `html_sup` / `html_kbd` / `html_mark` / `strong` / `em`

### 特殊节点组件

| 节点 | 组件 | 说明 |
|---|---|---|
| 代码块 | [CodeBlock.ts](../src/features/chat/markdown/CodeBlock.ts) | 异步 import shiki 高亮；支持 `streaming` 增量高亮；mermaid 语言走 MermaidBlock |
| Mermaid | [MermaidBlock.ts](../src/features/chat/markdown/MermaidBlock.ts) | 独立 mermaid 渲染 |
| 数学 | [init-katex.ts](../src/features/chat/markdown/init-katex.ts) + [streaming-math.ts](../src/features/chat/markdown/streaming-math.ts) | KaTeX 渲染，流式数学支持 |
| 图片 | [MarkdownImage.ts](../src/features/chat/markdown/MarkdownImage.ts) + [LazyMarkdownImage.ts](../src/features/chat/markdown/LazyMarkdownImage.ts) | 懒加载、本地图片 base path、图片预览弹窗 |
| toolcall 引用 | [toolcall-ref-group.ts](../src/features/chat/markdown/toolcall-ref-group.ts) | 跨段落 toolcall 引用聚合、预览（经 `toolcallPreviewMap`） |
| 自动链接 | [markdown-auto-link.ts](../src/features/chat/markdown/markdown-auto-link.ts) | 链接识别 |
| 代码块预览 | [CodeBlockPreviewDialog.vue](../src/features/chat/components/dialogs/CodeBlockPreviewDialog.vue) | 大代码块弹窗 |

## 共享语义样式

通用 Markdown 语义节点统一维护在：

- [markdown-content.css](../src/features/chat/markdown/markdown-content.css)（`.ecall-markdown-content`）

包括：

- 文字颜色继承
- 链接
- strong / em
- blockquote
- inline code
- table 基础边框与单元格
- hr
- Mermaid / code block 的基础可见性修正
- 各语义节点（heading / list / paragraph / text 等）的基础排版

不要在每个页面重复写这些语义节点的主题覆盖。

## 场景局部样式

各窗口只保留和具体场景有关的排版差异，例如：

- 字号
- 行高
- 段落间距
- 标题尺寸
- 列表缩进
- 表格 hover / stripe 等局部增强
- 文件阅读器、归档页等特殊阅读密度

这类样式应留在对应组件内，不要塞回全局共享层。

## 代码块渲染器

代码块组件按场景拆分：

- 聊天窗口：`AppMarkdownRenderer` 内置 `CodeBlock`（shiki 异步高亮 + streaming）
- 文件阅读器：`src/apps/file-reader/` 使用自己的 `file-reader-markstream` 注册与代码块组件

不要把聊天窗口的渲染器直接复用到文档阅读场景。聊天代码块依赖聊天气泡外壳，文件阅读器应使用自己的注册与组件。

## 覆盖原则

1. 优先改共享语义样式，避免重复覆盖。
2. 只在场景确有差异时写局部规则。
3. 子组件内部 DOM 需要穿透时才使用 `:deep()`。
4. 避免无依据使用 `!important`；若必须覆盖第三方库 containment 或可见性规则，应先确认来源与影响范围。
5. 透明度只保留一层来源，避免 `color-mix(... transparent)` 与 `opacity` 叠加。
6. 新增块类型或行内段类型时，必须在 parse-markdown 的类型联合、解析器与渲染器三处同步，并补 spec 测试（各模块均有对应 `.spec.ts` / `.test.ts`）。
