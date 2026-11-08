<template>
  <div class="flex flex-col gap-2 p-2">
    <div class="flex flex-wrap items-center gap-2 text-xs">
      <label class="flex items-center gap-1">
        <input :checked="wrapEnabled" type="checkbox" class="checkbox checkbox-xs" @change="toggleWrap" />
        软换行
      </label>
      <select v-model.number="demoWidth" class="select select-bordered select-xs">
        <option :value="620">620</option>
        <option :value="900">900</option>
      </select>
      <button type="button" class="btn btn-xs btn-primary" @click="runAll">跑测量</button>
      <span class="text-base-content/60">{{ statusText }}</span>
    </div>

    <div
      class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100"
      :style="{ width: `min(${demoWidth}px, 100%)`, height: '620px' }"
    >
      <FileReaderPanel
        ref="panelRef"
        :initial-open-path="DEMO_PATH"
        :show-tabs="false"
        :show-pick-file-button="false"
        :enable-global-drop="false"
        :markdown-is-dark="false"
        session-key="file-reader-wrap-demo"
      />
    </div>

    <pre class="demo-measure max-h-40 overflow-auto rounded bg-base-200 p-2 text-[11px] leading-4">{{ reportText }}</pre>
    <p class="text-xs text-base-content/60">
      <span class="font-medium text-base-content/80">验的是：</span>换行模式下虚拟块的布局总高是否等于真实总高、末尾内容能否
      一次滚到、按行号跳转的落点是否还偏。真实 FileReaderPanel，只把 Tauri 文件读取命令换成内存里的
      {{ TOTAL_LINES }} 行 × {{ LINE_CHARS }} 字符 mock 代码文件（不发请求，不落盘）。
    </p>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import FileReaderPanel from "./FileReaderPanel.vue";
import { useFileReaderAppearance } from "../../shell/composables/use-file-reader-appearance";

const DEMO_PATH = "E:/demo/workspace/src/long-lines.ts";
const TOTAL_LINES = 2000;
const BLOCK_LINES = 120;
const LINE_CHARS = 260;
const demoWidth = ref(620);
const statusText = ref("待测量");
const reportText = ref("");
const { fileReaderLineWrapEnabled: wrapEnabled, toggleFileReaderLineWrapEnabled } = useFileReaderAppearance();

function makeLine(lineNumber: number): string {
  const head = `const line${lineNumber} = computeSomethingVeryLongName(${lineNumber}, "a-piece-of-text-that-is-long");`;
  return head.padEnd(LINE_CHARS, " // 填充尾部，让这一行在换行模式下必然折成多行");
}

function blockContent(startLine: number, lineCount: number): string {
  const lines: string[] = [];
  for (let i = 0; i < lineCount; i += 1) lines.push(makeLine(startLine + i));
  return lines.join("\n");
}

// 只替换文件阅读器用到的几个命令，其余一律拒绝，效果等同「宿主没提供这个能力」。
const FAKE_COMMANDS: Record<string, (args: Record<string, unknown>) => unknown> = {
  read_file_reader_file: () => ({
    path: DEMO_PATH,
    name: "long-lines.ts",
    extension: "ts",
    kind: "code",
    content: "",
    forcePlain: false,
    virtualized: true,
    totalLines: TOTAL_LINES,
    blockLineCount: BLOCK_LINES,
    len: TOTAL_LINES * LINE_CHARS,
    modifiedMs: 1_700_000_000_000,
  }),
  read_file_reader_file_block: (args) => {
    const startLine = Number(args.startLine || 1);
    const lineCount = Number(args.lineCount || BLOCK_LINES);
    const endLine = Math.min(TOTAL_LINES, startLine + lineCount - 1);
    return { path: DEMO_PATH, startLine, endLine, content: blockContent(startLine, endLine - startLine + 1) };
  },
  stat_file_reader_file: () => ({ path: DEMO_PATH, exists: true, len: TOTAL_LINES * LINE_CHARS, modifiedMs: 1_700_000_000_000 }),
  file_reader_watch_targets: () => ({ targets: [] }),
};

function installFakeInvoke() {
  const target = window as unknown as { __TAURI_INTERNALS__?: Record<string, unknown> };
  target.__TAURI_INTERNALS__ = {
    ...(target.__TAURI_INTERNALS__ || {}),
    invoke: (command: string, args: Record<string, unknown>) => {
      const handler = FAKE_COMMANDS[command];
      if (!handler) return Promise.reject(new Error(`demo 未提供命令: ${command}`));
      return Promise.resolve(handler(args || {}));
    },
    transformCallback: (callback: unknown) => callback,
  };
}
installFakeInvoke();

const nextFrame = () =>
  new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));

function codeScroller(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".file-reader-code-virtual-scroller");
}

/** 反复把 scrollTop 顶到 scrollHeight，直到最后一行底边贴住容器底边：次数就是「要反复几次才到底」。 */
async function scrollToBottomAndVerify() {
  const scroller = codeScroller();
  if (!scroller) return { attempts: null, note: "找不到代码滚动容器" };
  let attempts = 0;
  let gap: number | null = null;
  for (let i = 0; i < 8; i += 1) {
    attempts += 1;
    scroller.scrollTop = scroller.scrollHeight;
    await nextFrame();
    const lastLine = scroller.querySelector<HTMLElement>(`[data-line-number="${TOTAL_LINES}"]`);
    gap = lastLine
      ? Math.round(lastLine.getBoundingClientRect().bottom - scroller.getBoundingClientRect().bottom)
      : null;
    if (gap !== null && gap <= 1) break;
  }
  return {
    attempts,
    lastLineBottomGap: gap,
    scrollHeight: Math.round(scroller.scrollHeight),
    clientHeight: Math.round(scroller.clientHeight),
    scrollTop: Math.round(scroller.scrollTop),
  };
}

/** 按行号跳到末尾 5% 处的一行，量目标行顶边相对视口顶边的误差。 */
async function jumpToLine(lineNumber: number) {
  const scroller = codeScroller();
  if (!scroller) return { lineNumber, errorTop: null, note: "找不到代码滚动容器" };
  const totalHeight = scroller.scrollHeight;
  const progress = Math.min(1, Math.max(0, (lineNumber - 1) / Math.max(1, TOTAL_LINES - 1)));
  scroller.scrollTop = progress * Math.max(0, totalHeight - scroller.clientHeight);
  await nextFrame();
  await nextFrame();
  const line = scroller.querySelector<HTMLElement>(`[data-line-number="${lineNumber}"]`);
  if (!line) return { lineNumber, errorTop: null, note: "目标行未进入渲染窗口" };
  const errorTop = Math.round(line.getBoundingClientRect().top - scroller.getBoundingClientRect().top);
  return { lineNumber, errorTop, scrollHeight: Math.round(scroller.scrollHeight), scrollTop: Math.round(scroller.scrollTop) };
}

function toggleWrap() {
  toggleFileReaderLineWrapEnabled();
  statusText.value = `换行：${wrapEnabled.value ? "开" : "关"}`;
}

/** 走面板自己的「按行号打开」路径，量目标行顶边相对视口顶边的误差。 */
async function jumpToLineViaPanel(lineNumber: number) {
  await panelRef.value?.openPath?.(DEMO_PATH, { reuseActiveTab: true, targetLine: lineNumber });
  await new Promise<void>((resolve) => setTimeout(resolve, 400));
  await nextFrame();
  await nextFrame();
  // 打开会重建虚拟列表实例，scroller 元素要重新取
  const scroller = codeScroller();
  if (!scroller) return { lineNumber, errorTop: null, note: "找不到代码滚动容器" };
  const line = scroller.querySelector<HTMLElement>(`[data-line-number="${lineNumber}"]`);
  const rendered = Array.from(scroller.querySelectorAll<HTMLElement>("[data-line-number]"))
    .map((el) => Number(el.dataset.lineNumber))
    .filter((n) => Number.isFinite(n));
  const renderedRange = rendered.length ? [Math.min(...rendered), Math.max(...rendered)] : null;
  if (!line) return { lineNumber, errorTop: null, renderedRange, note: "目标行未进入渲染窗口" };
  return {
    lineNumber,
    errorTop: Math.round(line.getBoundingClientRect().top - scroller.getBoundingClientRect().top),
    renderedRange,
    scrollHeight: Math.round(scroller.scrollHeight),
    scrollTop: Math.round(scroller.scrollTop),
  };
}

async function runAll() {
  statusText.value = "测量中…";
  await nextFrame();
  const scroller = codeScroller();
  const lines: string[] = [];
  const push = (label: string, value: unknown) => lines.push(`${label}: ${JSON.stringify(value)}`);
  push("换行", wrapEnabled.value);
  push("容器宽/高", scroller ? `${scroller.clientWidth}x${scroller.clientHeight}` : null);
  push("先滚到底", await scrollToBottomAndVerify());
  push("按行号跳到 1950（面板同一套比例滚动）", await jumpToLine(1950));
  reportText.value = lines.join("\n");
  statusText.value = "测量完成";
}

onMounted(() => {
  (window as unknown as { __fileDemo?: unknown }).__fileDemo = {
    runAll,
    toggleWrap,
    setWrap: (value: boolean) => {
      if (value !== wrapEnabled.value) toggleFileReaderLineWrapEnabled();
    },
    setWidth: (value: number) => {
      demoWidth.value = value;
    },
    report: () => reportText.value,
    scroller: codeScroller,
    errors: () => demoErrors,
    open: () => forceOpen(),
  };
  forceOpen();
});

const panelRef = ref<{ openPath?: (path: string, options?: { reuseActiveTab?: boolean; targetLine?: number }) => Promise<void> } | null>(null);
const demoErrors: string[] = [];
window.addEventListener("unhandledrejection", (event) => {
  demoErrors.push(`unhandled: ${String((event as PromiseRejectionEvent).reason)}`);
});

async function forceOpen() {
  try {
    await panelRef.value?.openPath?.(DEMO_PATH);
    statusText.value = "已打开";
  } catch (error) {
    demoErrors.push(`openPath: ${error instanceof Error ? error.message : String(error)}`);
    statusText.value = "打开失败";
  }
}
</script>
