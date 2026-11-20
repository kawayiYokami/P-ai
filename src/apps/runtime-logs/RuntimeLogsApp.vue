<template>
  <div class="flex h-screen min-h-0 flex-col bg-base-100 text-base-content">
    <header class="flex h-10 shrink-0 items-center gap-2 bg-base-200 px-3" data-tauri-drag-region>
      <span class="pointer-events-none text-sm font-semibold opacity-80">运行日志</span>
      <span class="pointer-events-none text-xs opacity-50">内存 · 进程退出即清空</span>
      <div class="flex-1" data-tauri-drag-region />
      <div class="flex items-center gap-1 text-xs">
        <label class="flex items-center gap-1">
          <span class="opacity-60">级别</span>
          <select v-model="selectedLevel" class="select select-bordered select-xs w-24">
            <option value="all">全部</option>
            <option v-for="level in levelOptions" :key="level" :value="level">{{ level.toUpperCase() }}</option>
          </select>
        </label>
        <label class="flex items-center gap-1">
          <span class="opacity-60">模块</span>
          <select v-model="selectedModule" class="select select-bordered select-xs w-36">
            <option value="all">全部</option>
            <option v-for="m in moduleOptions" :key="m" :value="m">{{ m }}</option>
          </select>
        </label>
        <button class="btn btn-ghost btn-xs" title="复制" :disabled="filteredLogs.length === 0" @click="copyLogs">复制</button>
        <button class="btn btn-ghost btn-xs" title="清空" :disabled="logs.length === 0" @click="clearLogs">清空</button>
      </div>
      <button class="btn btn-ghost btn-xs" title="最小化" @click="minimizeWindow">
        <span class="text-sm">─</span>
      </button>
      <button class="btn btn-ghost btn-xs hover:bg-error" title="关闭" @click="closeWindow">
        <span class="text-sm">✕</span>
      </button>
    </header>

    <div v-if="filteredLogs.length === 0" class="flex-1 border-t border-base-300 bg-base-100 p-3 text-xs opacity-50 [font-family:var(--app-code-font-family)]">{{ loading ? "正在加载..." : "暂无日志" }}</div>
    <div
      v-else
      class="relative flex-1 min-h-0 border-t border-base-300"
      @mouseenter="logScrollbarRef?.reveal()"
      @mouseleave="logScrollbarRef?.hide()"
    >
      <div
        ref="logScroller"
        class="ecall-chat-scroll-container h-full overflow-y-auto bg-base-100 text-xs leading-5 [font-family:var(--app-code-font-family)]"
        @scroll.passive="handleNativeScroll"
      >
        <Virtualizer
          ref="vlistRef"
          :data="filteredLogs"
          :item-size="ROW_HEIGHT"
          :scroll-ref="(logScroller as unknown as HTMLElement)"
          v-slot="{ item, index }"
        >
          <div
            class="overflow-hidden pr-3 pl-3 text-ellipsis whitespace-pre"
            :class="levelClass(item.level)"
            :title="item.message"
            :style="{ height: `${ROW_HEIGHT}px`, lineHeight: `${ROW_HEIGHT}px` }"
          >{{ formatLine(item) }}</div>
        </Virtualizer>
      </div>
      <FloatingScrollbar ref="logScrollbarRef" :target="logScroller" persistent />
    </div>

    <footer class="flex h-6 shrink-0 items-center gap-2 border-t border-base-300 bg-base-200 px-3 text-xs opacity-60">
      <span>显示 {{ filteredLogs.length }} / {{ logs.length }}</span>
      <span v-if="errorText" class="text-error">{{ errorText }}</span>
      <span class="ml-auto">{{ stickToBottom ? "滚动到底部自动跟随" : "已暂停跟随，滚回底部恢复" }}</span>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Virtualizer } from "virtua/vue";
import {
  hideCurrentTransportWindow,
  invokeTauri,
  minimizeCurrentTransportWindow,
  onTransportNotification,
  writeTransportClipboardText,
} from "../../services/tauri-api";
import { useAppTheme } from "../../features/shell/composables/use-app-theme";
import FloatingScrollbar from "../../features/shell/components/FloatingScrollbar.vue";
import type { PersistedThemePreferences } from "../../features/shell/theme/theme-types";

type RuntimeLogEntry = {
  id: string;
  createdAt: string;
  level: string;
  message: string;
  repeat: number;
};

const POLL_INTERVAL_MS = 100;
const ROW_HEIGHT = 20;
const BOTTOM_FOLLOW_THRESHOLD_PX = 40;
const levelOptions = ["info", "warn", "error", "debug", "trace"] as const;

const logs = ref<RuntimeLogEntry[]>([]);
const loading = ref(false);
const errorText = ref("");
const selectedLevel = ref<"all" | string>("info");
const selectedModule = ref("all");
const vlistRef = ref<InstanceType<typeof Virtualizer> | null>(null);
// 滚动容器由本组件持有并显式交给 Virtualizer：VList 只能把父元素当滚动容器，
// 在本页布局下会认错容器，导致 onScroll 永不触发、跟随判定失效。
const logScroller = ref<HTMLElement | null>(null);
const logScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastCreatedAt = "";
let unlistenTheme: (() => void) | null = null;
// 程序化贴底会派发 scroll 事件，不能让它改写跟随状态，否则会覆盖用户刚做的上滚。
let programmaticScroll = false;
const stickToBottom = ref(true);

const { applyTheme, restoreThemeFromStorage } = useAppTheme();

const moduleOptions = computed(() => {
  const set = new Set<string>();
  for (const item of logs.value) {
    const m = extractModule(item.message);
    if (m) set.add(m);
  }
  return Array.from(set).sort((a, b) => a.localeCompare(b, "zh-CN"));
});

const filteredLogs = computed(() =>
  logs.value.filter((item) => {
    if (selectedLevel.value !== "all" && item.level !== selectedLevel.value) return false;
    if (selectedModule.value !== "all" && extractModule(item.message) !== selectedModule.value) return false;
    return true;
  }),
);

// 滚动判定直接读自建容器，不再依赖虚拟列表对滚动容器的推断。
function handleNativeScroll() {
  if (programmaticScroll) return;
  const el = logScroller.value;
  if (!el) return;
  const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  stickToBottom.value = distanceToBottom <= BOTTOM_FOLLOW_THRESHOLD_PX;
}

function scrollToBottom() {
  const handle = vlistRef.value as unknown as { scrollToIndex: (index: number, opts?: unknown) => void; scrollTo: (offset: number) => void; scrollSize: number } | null;
  if (!handle || filteredLogs.value.length === 0) return;
  programmaticScroll = true;
  try {
    handle.scrollToIndex(filteredLogs.value.length - 1, { align: "end" });
  } catch {
    try {
      handle.scrollTo(handle.scrollSize);
    } catch {
      // ignore
    }
  }
  requestAnimationFrame(() => {
    programmaticScroll = false;
  });
}

watch(filteredLogs, () => {
  if (stickToBottom.value) {
    nextTick(() => {
      scrollToBottom();
    });
  }
});

onMounted(async () => {
  restoreThemeFromStorage();
  try {
    unlistenTheme = onTransportNotification<PersistedThemePreferences>("theme.changed", (state) => {
      applyTheme(state);
    });
  } catch (err) {
    console.error("[运行日志窗口] 监听主题变化失败", err);
    errorText.value = `监听主题变化失败：${String(err)}`;
  }
  await loadInitial();
  startPolling();
});

onBeforeUnmount(() => {
  stopPolling();
  unlistenTheme?.();
});

async function loadInitial() {
  loading.value = true;
  try {
    await invokeTauri("append_runtime_log_probe", { message: "运行日志窗口已打开" });
  } catch {
    // ignore
  }
  try {
    const items = await invokeTauri<RuntimeLogEntry[]>("list_recent_runtime_logs");
    logs.value = items;
    if (items.length > 0) {
      lastCreatedAt = items[items.length - 1].createdAt;
    }
    await nextTick();
    scrollToBottom();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    loading.value = false;
  }
}

function startPolling() {
  stopPolling();
  pollTimer = setInterval(pollIncremental, POLL_INTERVAL_MS);
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

async function pollIncremental() {
  try {
    const items = await invokeTauri<RuntimeLogEntry[]>("list_runtime_logs_since", {
      sinceCreatedAt: lastCreatedAt,
    });
    if (items.length > 0) {
      logs.value = [...logs.value, ...items];
      lastCreatedAt = items[items.length - 1].createdAt;
      errorText.value = "";
    }
  } catch (err) {
    errorText.value = String(err);
  }
}

async function clearLogs() {
  try {
    await invokeTauri("clear_recent_runtime_logs");
    logs.value = [];
    lastCreatedAt = "";
    errorText.value = "";
  } catch (err) {
    errorText.value = String(err);
  }
}

async function copyLogs() {
  const text = filteredLogs.value.map(formatLine).join("\n");
  if (!text) return;
  try {
    await writeTransportClipboardText(text);
  } catch {
    errorText.value = "复制失败";
  }
}

function minimizeWindow() {
  void minimizeCurrentTransportWindow();
}

function closeWindow() {
  void hideCurrentTransportWindow();
}

function extractModule(message: string): string | null {
  if (!message.startsWith("[")) return null;
  const m = message.match(/^\[([^\]]+)\]/);
  return m?.[1]?.trim() || null;
}

function levelClass(level: string): string {
  switch (String(level || "").trim().toLowerCase()) {
    case "error":
      return "text-error";
    case "warn":
    case "warning":
      return "text-warning";
    case "debug":
      return "text-info";
    case "trace":
      return "opacity-60";
    case "info":
    default:
      return "text-base-content";
  }
}

function formatLine(item: RuntimeLogEntry): string {
  const time = formatTime(item.createdAt);
  const parts = [`[${time}]`, item.level.toUpperCase(), item.message];
  if (item.repeat > 1) parts.push(`x${item.repeat}`);
  return parts.join(" ");
}

function formatTime(value: string): string {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value.replace("T", " ").replace(/(\.\d+)?Z?$/, "");
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}
</script>
