<template>
  <div class="ecall-sandbox-card" :class="{ 'ecall-sandbox-card-dark': isDark }">
    <div class="ecall-sandbox-toolbar">
      <div class="ecall-sandbox-title-slot">
        <input
          v-if="editingUrl"
          ref="urlInputRef"
          v-model="urlDraft"
          class="ecall-sandbox-url-input"
          placeholder="http://127.0.0.1:1420"
          spellcheck="false"
          @keydown.enter.prevent="commitUrl"
          @keydown.esc.prevent="cancelUrlEdit"
          @blur="cancelUrlEdit"
        />
        <button
          v-else
          type="button"
          class="ecall-sandbox-title"
          :title="titleHint"
          @click="startUrlEdit"
        >
          <AppWindow class="ecall-sandbox-title-icon" />
          <span class="ecall-sandbox-title-text">{{ titleLabel }}</span>
        </button>
      </div>
      <div class="ecall-sandbox-actions">
        <button
          type="button"
          class="ecall-sandbox-action"
          :title="reloadTitle"
          @click="reload"
        >
          <RotateCw class="ecall-sandbox-action-icon" />
        </button>
        <template v-if="mode === 'inline'">
          <button
            type="button"
            class="ecall-sandbox-action"
            :class="{ 'ecall-sandbox-action-active': viewMode === 'code' }"
            :title="viewMode === 'code' ? previewTitle : codeTitle"
            @click="toggleViewMode"
          >
            <Code2 v-if="viewMode === 'preview'" class="ecall-sandbox-action-icon" />
            <Eye v-else class="ecall-sandbox-action-icon" />
          </button>
          <button
            type="button"
            class="ecall-sandbox-action"
            :title="copied ? copiedText : copyTitle"
            @click="copyCode"
          >
            <Check v-if="copied" class="ecall-sandbox-action-icon" />
            <Copy v-else class="ecall-sandbox-action-icon" />
          </button>
          <button
            type="button"
            class="ecall-sandbox-action"
            :title="exportTitle"
            @click="exportHtml"
          >
            <Download class="ecall-sandbox-action-icon" />
          </button>
          <button
            v-if="openInReaderTitle"
            type="button"
            class="ecall-sandbox-action ecall-sandbox-action-labeled"
            :title="openInReaderTitle"
            :aria-label="openInReaderTitle"
            @click="emit('open-in-reader')"
          >
            <PanelRightOpen class="ecall-sandbox-action-icon" />
            <span class="ecall-sandbox-action-text">{{ openInReaderTitle }}</span>
          </button>
          <button
            v-if="exitTitle"
            type="button"
            class="ecall-sandbox-action"
            :title="exitTitle"
            @click="emit('exit')"
          >
            <X class="ecall-sandbox-action-icon" />
          </button>
        </template>
      </div>
    </div>

    <div v-if="urlNotice" class="ecall-sandbox-error">
      <CircleAlert class="ecall-sandbox-error-icon" />
      <span class="ecall-sandbox-error-text">{{ urlNotice }}</span>
    </div>

    <!-- 内联 html 模式：srcdoc + 注入 CSP。这一份 iframe 的沙箱属性不许动。 -->
    <div v-if="viewMode === 'preview' && mode === 'inline'" class="ecall-sandbox-stage" :class="{ 'ecall-sandbox-stage-overflow': heightOverflow }">
      <iframe
        ref="frameRef"
        :key="reloadSeq"
        class="ecall-sandbox-frame"
        :class="{ 'ecall-sandbox-frame-capped': heightOverflow }"
        sandbox="allow-scripts allow-forms allow-modals"
        :srcdoc="srcdoc"
        :style="frameStyle"
        title="interactive-html-sandbox"
      />
      <div v-if="!frameReady && !frameError" class="ecall-sandbox-loading">
        <span class="ecall-sandbox-loading-dot" />
        {{ loadingText }}
      </div>
    </div>
    <!--
      网址模式：单独一个 iframe，不注入 CSP，让目标页面用自己的策略。
      这里同时给了 allow-scripts 与 allow-same-origin，等于对这个框架关掉了沙箱——
      只要被嵌入页与宿主同源，它就能读写父文档。分两种情况：
      · 生产环境：PAI 的 origin 是 http://tauri.localhost，嵌 127.0.0.1 是跨源，读不到父文档；
      · dev 环境：PAI 自己就跑在 127.0.0.1:1420，这时是同源，隔离形同虚设。
        这个风险我们接受（开发者自己嵌自己的 dev server），但硬边界不能松：
        只允许 http + loopback，外网、file://、javascript: 一律拒绝，见 normalizeSandboxUrl。
    -->
    <div v-else-if="viewMode === 'preview'" class="ecall-sandbox-stage">
      <iframe
        ref="frameRef"
        :key="`url:${reloadSeq}`"
        class="ecall-sandbox-frame"
        sandbox="allow-scripts allow-forms allow-modals allow-same-origin"
        :src="activeUrl"
        :style="urlFrameStyle"
        title="interactive-url-sandbox"
      />
    </div>
    <pre v-else class="ecall-sandbox-source"><code>{{ code }}</code></pre>

    <div v-if="frameError" class="ecall-sandbox-error">
      <CircleAlert class="ecall-sandbox-error-icon" />
      <span class="ecall-sandbox-error-text">{{ frameError }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { AppWindow, Check, CircleAlert, Code2, Copy, Download, Eye, PanelRightOpen, RotateCw, X } from "@lucide/vue";
import {
  SANDBOX_MAX_HEIGHT,
  SANDBOX_MESSAGE_TYPE_ERROR,
  SANDBOX_MESSAGE_TYPE_READY,
  SANDBOX_MESSAGE_TYPE_RESIZE,
  SANDBOX_MIN_HEIGHT,
  buildSandboxSrcdoc,
  collectThemeVariables,
  normalizeSandboxUrl,
} from "../utils/sandbox-html-builder";
import { saveTransportTextFileAs } from "../../../services/tauri-api";

const props = withDefaults(defineProps<{
  code: string;
  isDark?: boolean;
  titleText?: string;
  loadingText?: string;
  reloadTitle?: string;
  codeTitle?: string;
  previewTitle?: string;
  copyTitle?: string;
  copiedText?: string;
  exportTitle?: string;
  /** 非空时显示「退出预览、回到代码」按钮 */
  exitTitle?: string;
  /**
   * 非空时在工具栏右侧显示「在右栏打开」。
   * 只由聊天侧显式给值——右栏自己渲染（FileReaderHtmlPreview）不传，因此那边不会出现这个按钮。
   */
  openInReaderTitle?: string;
}>(), {
  isDark: false,
  titleText: "交互组件",
  loadingText: "正在运行交互组件…",
  reloadTitle: "重新加载",
  codeTitle: "查看源码",
  previewTitle: "返回预览",
  copyTitle: "复制代码",
  copiedText: "已复制",
  exportTitle: "导出为 HTML",
  openInReaderTitle: "",
});

const emit = defineEmits<{
  (e: "exit"): void;
  (e: "open-in-reader"): void;
}>();

const frameRef = ref<HTMLIFrameElement | null>(null);
const viewMode = ref<"preview" | "code">("preview");
const frameHeight = ref(0);
const frameReady = ref(false);
const frameError = ref("");
const heightOverflow = ref(false);
const copied = ref(false);
const reloadSeq = ref(0);
// ==================== 网址模式 ====================
// inline：渲染 props.code（srcdoc + 注入 CSP）；url：加载本机地址（src，目标页自己的策略）。
// 两套来源不混用，各自一个 iframe，沙箱属性也各自写死在模板里。
const mode = ref<"inline" | "url">("inline");
const activeUrl = ref("");
const editingUrl = ref(false);
const urlDraft = ref("");
const urlNotice = ref("");
const urlInputRef = ref<HTMLInputElement | null>(null);
/** 网址模式的目标页不会回报高度（没有我们的 Bridge），给固定视口高度 */
const SANDBOX_URL_HEIGHT = 520;
let copyTimer = 0;
let themeObserver: MutationObserver | null = null;
let loadingFallbackTimer = 0;

const themeTokens = computed(() => collectThemeVariables());

const srcdoc = computed(() =>
  buildSandboxSrcdoc(props.code, themeTokens.value, props.isDark),
);

const frameStyle = computed(() => {
  if (heightOverflow.value) return { height: `${SANDBOX_MAX_HEIGHT}px` };
  const height = Math.max(frameHeight.value, SANDBOX_MIN_HEIGHT);
  return { height: `${height}px` };
});

const urlFrameStyle = computed(() => ({ height: `${SANDBOX_URL_HEIGHT}px` }));

const titleLabel = computed(() => (mode.value === "url" ? activeUrl.value : props.titleText));
const titleHint = computed(() => (mode.value === "url"
  ? "点击修改地址；清空后回车回到内联 html"
  : "点击输入本机地址，预览你起的前端"));

function clampHeight(height: number): number {
  if (!Number.isFinite(height) || height <= 0) return 0;
  return Math.min(Math.max(Math.ceil(height), SANDBOX_MIN_HEIGHT), SANDBOX_MAX_HEIGHT * 4);
}

let shrinkTimer = 0;

function applyFrameHeight(height: number) {
  const next = height > SANDBOX_MAX_HEIGHT ? SANDBOX_MAX_HEIGHT : height;
  heightOverflow.value = height > SANDBOX_MAX_HEIGHT;
  // 变小防抖、变大立即应用、±2px 抖动忽略，避免 ResizeObserver 回环振荡
  if (next === frameHeight.value) return;
  if (next > frameHeight.value) {
    if (shrinkTimer) {
      window.clearTimeout(shrinkTimer);
      shrinkTimer = 0;
    }
    frameHeight.value = next;
    return;
  }
  if (Math.abs(next - frameHeight.value) <= 2) return;
  if (shrinkTimer) window.clearTimeout(shrinkTimer);
  shrinkTimer = window.setTimeout(() => {
    shrinkTimer = 0;
    frameHeight.value = next;
  }, 160);
}

function onMessage(event: MessageEvent) {
  const frame = frameRef.value;
  if (!frame || event.source !== frame.contentWindow) return;
  const data = event.data;
  if (!data || typeof data !== "object") return;
  if (data.type === SANDBOX_MESSAGE_TYPE_RESIZE) {
    const height = clampHeight(Number(data.height));
    if (!height) return;
    applyFrameHeight(height);
    frameReady.value = true;
    return;
  }
  if (data.type === SANDBOX_MESSAGE_TYPE_READY) {
    pushThemeToFrame();
    return;
  }
  if (data.type === SANDBOX_MESSAGE_TYPE_ERROR) {
    frameError.value = String(data.message || "沙箱脚本运行错误");
  }
}

function pushThemeToFrame() {
  const frame = frameRef.value;
  if (!frame || !frame.contentWindow) return;
  // 网址模式嵌的是别人的页面，它不认我们的主题消息，不往那边发
  if (mode.value === "url") return;
  try {
    frame.contentWindow.postMessage({
      type: "pai-sandbox:theme",
      tokens: themeTokens.value,
      isDark: props.isDark,
    }, "*");
  } catch {
    // iframe 尚未就绪时忽略
  }
}

// ==================== 网址模式 ====================

function startUrlEdit() {
  editingUrl.value = true;
  urlDraft.value = mode.value === "url" ? activeUrl.value : "";
  urlNotice.value = "";
  void nextTick(() => {
    urlInputRef.value?.focus();
    urlInputRef.value?.select();
  });
}

function cancelUrlEdit() {
  // 回车提交后 input 会被移除，浏览器随后可能补一次 blur，这里必须只认「还在编辑」
  if (!editingUrl.value) return;
  editingUrl.value = false;
  urlDraft.value = "";
}

function commitUrl() {
  const raw = urlDraft.value.trim();
  editingUrl.value = false;
  urlDraft.value = "";
  // 清空后回车 = 回到内联 html
  if (!raw) {
    exitUrlMode();
    return;
  }
  const resolved = normalizeSandboxUrl(raw);
  if (!resolved) {
    // 不合法就保持现状，只提示；不要白屏，也不要悄悄把当前页面换掉
    urlNotice.value = "只能打开本机地址（http://127.0.0.1 或 http://localhost），其他地址不加载";
    return;
  }
  urlNotice.value = "";
  activeUrl.value = resolved;
  mode.value = "url";
  viewMode.value = "preview";
  frameError.value = "";
  frameReady.value = false;
  reloadSeq.value += 1;
}

function exitUrlMode() {
  mode.value = "inline";
  activeUrl.value = "";
  urlNotice.value = "";
  viewMode.value = "preview";
  frameError.value = "";
  frameReady.value = false;
  frameHeight.value = 0;
  heightOverflow.value = false;
  reloadSeq.value += 1;
}

function reload() {
  frameError.value = "";
  frameReady.value = false;
  frameHeight.value = 0;
  heightOverflow.value = false;
  reloadSeq.value += 1;
  armLoadingFallback();
}

function toggleViewMode() {
  viewMode.value = viewMode.value === "preview" ? "code" : "preview";
}

async function copyCode() {
  try {
    await navigator.clipboard.writeText(props.code || "");
    copied.value = true;
    if (copyTimer) window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => {
      copied.value = false;
      copyTimer = 0;
    }, 1500);
  } catch {
    copied.value = false;
  }
}

async function exportHtml() {
  const doc = srcdoc.value;
  if (!doc) return;
  try {
    await saveTransportTextFileAs(
      `pai-sandbox-${Date.now()}.html`,
      doc,
      [{ name: "HTML", extensions: ["html"] }],
    );
  } catch (error) {
    frameError.value = `导出失败: ${error instanceof Error ? error.message : String(error)}`;
  }
}

function armLoadingFallback() {
  if (loadingFallbackTimer) window.clearTimeout(loadingFallbackTimer);
  loadingFallbackTimer = window.setTimeout(() => {
    // 沙箱内无 Bridge 或脚本异常时保底展示，避免永久 loading
    if (!frameReady.value && !frameError.value) {
      frameReady.value = true;
      frameHeight.value = Math.max(frameHeight.value, 240);
    }
  }, 4000);
}

watch(
  () => [props.isDark, themeTokens.value],
  () => pushThemeToFrame(),
  { deep: true },
);

watch(frameRef, () => {
  frameError.value = "";
  armLoadingFallback();
});

onMounted(() => {
  window.addEventListener("message", onMessage);
  if (typeof MutationObserver === "function" && typeof document !== "undefined") {
    themeObserver = new MutationObserver(() => pushThemeToFrame());
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "class", "style"],
    });
  }
  armLoadingFallback();
});

onBeforeUnmount(() => {
  window.removeEventListener("message", onMessage);
  if (themeObserver) {
    themeObserver.disconnect();
    themeObserver = null;
  }
  if (copyTimer) window.clearTimeout(copyTimer);
  if (loadingFallbackTimer) window.clearTimeout(loadingFallbackTimer);
  if (shrinkTimer) window.clearTimeout(shrinkTimer);
});
</script>

<style scoped>
.ecall-sandbox-card {
  position: relative;
  margin: 0.25rem 0;
  max-width: 100%;
}

.ecall-sandbox-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0 0.1rem 0.2rem;
}

.ecall-sandbox-title-slot {
  display: flex;
  align-items: center;
  min-width: 0;
  flex: 1 1 auto;
}

.ecall-sandbox-title {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  min-width: 0;
  max-width: 100%;
  padding: 0;
  border: 0;
  background: none;
  font-size: var(--app-text-caption-size, 0.72rem);
  color: color-mix(in srgb, currentColor 70%, transparent);
  cursor: text;
}

.ecall-sandbox-title:hover {
  color: currentColor;
}

.ecall-sandbox-title-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ecall-sandbox-url-input {
  width: 100%;
  min-width: 0;
  padding: 0.05rem 0.35rem;
  border: 1px solid color-mix(in srgb, currentColor 22%, transparent);
  border-radius: 0.3rem;
  background: color-mix(in srgb, currentColor 4%, transparent);
  font-size: var(--app-text-caption-size, 0.72rem);
  line-height: 1.4;
  color: currentColor;
  outline: none;
}

.ecall-sandbox-url-input:focus {
  border-color: var(--color-primary, currentColor);
}

.ecall-sandbox-title-icon {
  width: 0.85rem;
  height: 0.85rem;
  flex-shrink: 0;
}

.ecall-sandbox-actions {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  flex-shrink: 0;
}

.ecall-sandbox-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.45rem;
  height: 1.45rem;
  border-radius: 0.3rem;
  color: color-mix(in srgb, currentColor 62%, transparent);
  transition: background-color 0.12s ease, color 0.12s ease;
}

.ecall-sandbox-action:hover {
  background: color-mix(in srgb, currentColor 10%, transparent);
  color: currentColor;
}

.ecall-sandbox-action-active {
  color: var(--color-primary, currentColor);
  background: color-mix(in srgb, var(--color-primary, currentColor) 14%, transparent);
}

/* 带文字的入口：与四个纯图标按钮同一行，但要让用户一眼看出它是干什么的 */
.ecall-sandbox-action-labeled {
  width: auto;
  gap: 0.22rem;
  padding: 0 0.4rem;
  white-space: nowrap;
}

.ecall-sandbox-action-text {
  font-size: var(--app-text-caption-size, 0.72rem);
  line-height: 1;
}

.ecall-sandbox-action-icon {
  width: 0.85rem;
  height: 0.85rem;
}

.ecall-sandbox-stage {
  position: relative;
  width: 100%;
}

.ecall-sandbox-stage-overflow {
  overflow-y: auto;
}

.ecall-sandbox-frame {
  display: block;
  width: 100%;
  min-height: 120px;
  border: 0;
  background: transparent;
}

.ecall-sandbox-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  font-size: var(--app-text-xs-size, 0.78rem);
  color: color-mix(in srgb, currentColor 55%, transparent);
  background: color-mix(in srgb, currentColor 3%, transparent);
  pointer-events: none;
}

.ecall-sandbox-loading-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 999px;
  background: var(--color-primary, currentColor);
  opacity: 0.7;
  animation: ecall-sandbox-pulse 1.1s ease-in-out infinite;
}

@keyframes ecall-sandbox-pulse {
  0%, 100% { transform: scale(0.7); opacity: 0.4; }
  50% { transform: scale(1); opacity: 0.9; }
}

.ecall-sandbox-source {
  margin: 0;
  max-height: 420px;
  overflow: auto;
  padding: 0.75rem;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 0.5rem;
  background: color-mix(in srgb, currentColor 5%, transparent);
  font-family: var(--app-code-font-family, monospace);
  font-size: var(--app-text-xs-size, 0.78rem);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  color: color-mix(in srgb, currentColor 85%, transparent);
}

.ecall-sandbox-error {
  display: flex;
  align-items: flex-start;
  gap: 0.4rem;
  padding: 0.45rem 0.6rem;
  margin-top: 0.25rem;
  border: 1px solid color-mix(in srgb, var(--color-error, #f87272) 40%, transparent);
  border-radius: 0.4rem;
  background: color-mix(in srgb, var(--color-error, #f87272) 10%, transparent);
  font-size: var(--app-text-caption-size, 0.72rem);
  color: var(--color-error, #f87272);
}

.ecall-sandbox-error-icon {
  width: 0.85rem;
  height: 0.85rem;
  flex-shrink: 0;
  margin-top: 0.05rem;
}

.ecall-sandbox-error-text {
  min-width: 0;
  word-break: break-all;
}
</style>
