<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { AppMarkdownRenderer, initKatex } from "./index";

// 数学渲染依赖 window.__ecall_katex，使用方必须在 setup 顶层初始化一次（与 ChatMessageItem 一致）
initKatex();

/**
 * Markdown 渲染全量演示。
 * 一份文本覆盖解析器全部块类型与行内类型：标题 1-4、段落、引用、有序/无序/嵌套列表、
 * 任务列表、表格、代码块（多语言 + 无语言）、行内与块级数学、mermaid、details、
 * 脚注、分割线、文件嵌入，以及粗体/斜体/粗斜体/删除线/行内码/链接/自动链接/图片/
 * 图片链接/上下标/kbd/mark/换行/工具引用/脚注引用。
 */

const demoWidth = ref(760);
const fixedHeight = ref(false);
const layout = ref<"compact" | "comfortable" | "relaxed">("comfortable");
const variant = ref<"chat" | "document">("chat");
const streaming = ref(false);
const isDark = ref(false);

const LAYOUT_CLASSES = ["ecall-md-layout-compact", "ecall-md-layout-comfortable", "ecall-md-layout-relaxed"];

function applyLayoutClass(next: "compact" | "comfortable" | "relaxed") {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  LAYOUT_CLASSES.forEach((name) => root.classList.remove(name));
  root.classList.add(`ecall-md-layout-${next}`);
}

onMounted(() => applyLayoutClass(layout.value));
onBeforeUnmount(() => {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  LAYOUT_CLASSES.forEach((name) => root.classList.remove(name));
});

// 图片走 base64 data URI：URL 里不能有空格与括号，否则 markdown 图片语法会在空格处截断
const IMAGE_SRC =
  "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHdpZHRoPScyNDAnIGhlaWdodD0nMTIwJz48ZGVmcz48bGluZWFyR3JhZGllbnQgaWQ9J2cnIHgxPScwJyB5MT0nMCcgeDI9JzEnIHkyPScxJz48c3RvcCBvZmZzZXQ9JzAnIHN0b3AtY29sb3I9JyM0ZjQ2ZTUnLz48c3RvcCBvZmZzZXQ9JzEnIHN0b3AtY29sb3I9JyMwNmI2ZDQnLz48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48cmVjdCB3aWR0aD0nMjQwJyBoZWlnaHQ9JzEyMCcgZmlsbD0ndXJsKCNnKScvPjx0ZXh0IHg9JzEyMCcgeT0nNjgnIGZvbnQtc2l6ZT0nMjAnIGZpbGw9J3doaXRlJyB0ZXh0LWFuY2hvcj0nbWlkZGxlJyBmb250LWZhbWlseT0nc2Fucy1zZXJpZic+ZGVtbyBpbWFnZTwvdGV4dD48L3N2Zz4=";

const markdownText = computed(() =>
  [
    "# 一级标题：Markdown 全量演示",
    "",
    "这一段覆盖行内样式：**粗体**、*斜体*、***粗斜体***、~~删除线~~、`inline code`、",
    "行内数学 $a^2 + b^2 = c^2$、[普通链接](https://example.com)、自动链接 https://example.com/a/b ，",
    "还有 <mark>高亮 mark</mark>、<kbd>Ctrl</kbd> + <kbd>K</kbd>、下标 H<sub>2</sub>O、上标 E = mc<sup>2</sup>、",
    "以及行内换行（本行末尾有 br）<br>换行之后还在同一段。",
    "",
    "工具引用标记：[toolcall:demo-call-1] 和连续三个 [toolcall:demo-call-2][toolcall:demo-call-3]。脚注引用在这里[^note1]，第二个[^note2]。",
    "",
    "## 二级标题",
    "",
    "### 三级标题",
    "",
    "#### 四级标题（解析器到四级为止）",
    "",
    "---",
    "",
    "> 这是一段引用。",
    "> 引用可以有第二行。",
    ">",
    "> > 还可以嵌套一层引用。",
    "",
    "无序列表：",
    "",
    "- 第一项",
    "- 第二项，带 **粗体** 与 `code`",
    "- 第三项",
    "  - 嵌套子项 A",
    "  - 嵌套子项 B",
    "  - 再深一层（解析器无层级，仍平铺）",
    "",
    "有序列表（从 3 开始，验证起始序号）：",
    "",
    "3. 第三步",
    "4. 第四步",
    "5. 第五步",
    "",
    "任务列表：",
    "",
    "- [x] 已经完成的事项",
    "- [ ] 还没做的事项",
    "",
    "表格（左对齐 / 居中 / 右对齐）：",
    "",
    "| 左对齐 | 居中 | 右对齐 |",
    "| :--- | :---: | ---: |",
    "| `alpha` | 中间 | 1024 |",
    "| **beta** | 中间 | 2048 |",
    "| gamma | 中间 | 4096 |",
    "",
    "代码块，带语言（走 shiki 高亮）：",
    "",
    "```ts",
    "type Demo = { id: string; size: number };",
    "",
    "export function scale(item: Demo, factor: number): Demo {",
    "  return { ...item, size: Math.round(item.size * factor) };",
    "}",
    "```",
    "",
    "```python",
    "def scale(item: dict, factor: float) -> dict:",
    '    return {**item, "size": round(item["size"] * factor)}',
    "```",
    "",
    "```bash",
    "pnpm typecheck && cargo check --manifest-path src-tauri/Cargo.toml",
    "```",
    "",
    "```json",
    '{ "ok": true, "count": 3, "items": ["a", "b", "c"] }',
    "```",
    "",
    "无语言代码块（不走向高亮）：",
    "",
    "```",
    "plain text block",
    "  保留前导空格",
    "```",
    "",
    "块级数学：",
    "",
    "$$",
    "\\int_{0}^{\\infty} e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
    "$$",
    "",
    "mermaid 流程图：",
    "",
    "```mermaid",
    "graph LR",
    "  A[收到消息] --> B{需要工具？}",
    "  B -- 是 --> C[执行工具]",
    "  B -- 否 --> D[直接回答]",
    "  C --> D",
    "```",
    "",
    "mermaid 时序图：",
    "",
    "```mermaid",
    "sequenceDiagram",
    "  participant U as 用户",
    "  participant P as PAI",
    "  U->>P: 发起请求",
    "  P->>U: 流式返回",
    "```",
    "",
    "<details>",
    "<summary>折叠块标题（点开看内容）</summary>",
    "",
    "折叠块内部也按 markdown 渲染：",
    "",
    "- 列表项",
    "- **粗体**",
    "",
    "</details>",
    "",
    "<details open>",
    "<summary>默认展开的折叠块</summary>",
    "",
    "这一块一开始就是展开状态。",
    "",
    "</details>",
    "",
    "图片：",
    "",
    `![演示图片](${IMAGE_SRC})`,
    "",
    "图片链接（点图跳转）：",
    "",
    `[![可点击的图片](${IMAGE_SRC})](https://example.com)`,
    "",
    "文件嵌入标签（demo 里指向一个不存在的沙箱文件，用来展示兜底形态）：",
    "",
    '<agent-embed src="demo/not-found.html"></agent-embed>',
    "",
    "最后一段普通正文，用来确认结尾间距正常。",
    "",
    "[^note1]: 第一条脚注的内容。",
    "[^note2]: 第二条脚注，稍微长一点，用来验证脚注区的换行与缩进。",
  ].join("\n"),
);

const toolcallPreviewMap = {
  "demo-call-1": { title: "exec", target: "pnpm typecheck", body: "命令执行完成，退出码 0" },
  "demo-call-2": { title: "read", target: "src/App.vue", body: "读取了 120 行" },
  "demo-call-3": { title: "rg", target: "markdown-content.css", body: "命中 3 处" },
};
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <select v-model="layout" class="select select-bordered select-xs" @change="applyLayoutClass(layout)">
        <option value="compact">紧凑</option>
        <option value="comfortable">适中</option>
        <option value="relaxed">宽松</option>
      </select>
      <select v-model="variant" class="select select-bordered select-xs">
        <option value="chat">chat 变体</option>
        <option value="document">document 变体</option>
      </select>
      <label class="flex items-center gap-1 text-xs">
        <input v-model="streaming" type="checkbox" class="checkbox checkbox-xs" />
        streaming
      </label>
      <label class="flex items-center gap-1 text-xs">
        <input v-model="isDark" type="checkbox" class="checkbox checkbox-xs" />
        深色配色
      </label>
      <label class="flex items-center gap-1 text-xs">
        <input v-model="fixedHeight" type="checkbox" class="checkbox checkbox-xs" />
        固定 720px 高（内部滚动）
      </label>
      <label class="flex items-center gap-2 text-xs">
        宽度
        <input v-model.number="demoWidth" type="range" min="360" max="980" step="20" class="range range-xs w-40" />
        <span class="tabular-nums text-base-content/60">{{ demoWidth }}px</span>
      </label>
    </div>

    <div
      class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100"
      :class="fixedHeight ? 'h-[720px]' : ''"
      :style="{ width: `min(${demoWidth}px, 100%)` }"
    >
      <div class="min-h-0 flex-1" :class="fixedHeight ? 'overflow-y-auto' : ''">
        <div class="p-4">
          <AppMarkdownRenderer
            :text="markdownText"
            :is-dark="isDark"
            :streaming="streaming"
            :variant="variant"
            :toolcall-preview-map="toolcallPreviewMap"
          />
        </div>
      </div>
    </div>

    <p class="max-w-[760px] text-xs leading-relaxed text-base-content/60">
      喂的是真实 <code>AppMarkdownRenderer</code>，文本覆盖解析器全部块类型与行内类型：标题 1-4、段落、引用（含嵌套）、
      无序/有序/嵌套/任务列表、三向对齐表格、五种代码块、行内与块级数学、两种 mermaid、details（开合两态）、
      脚注、分割线、文件嵌入、粗斜体/删除线/行内码/链接/自动链接/图片/图片链接/上下标/kbd/mark/换行/工具引用/脚注引用。
      顶部切档位走 <code>ecall-md-layout-*</code> 类，与真机同一条链路。图片用 data URI，嵌入标签指向不存在的文件以展示兜底。
      已知边界：解析器不支持列表嵌套层级（<code>MarkdownListItem</code> 只有 text/marker/value），缩进写法的子项会与父项平铺同级；
      任务列表的 <code>[x]</code> 不渲染勾选框，按普通列表项原样显示。
    </p>
  </div>
</template>
