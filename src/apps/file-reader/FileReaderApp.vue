<template>
  <div class="flex h-screen min-h-0 flex-col bg-base-100 text-base-content">
    <header class="flex h-10 shrink-0 items-end gap-2 bg-base-200 px-2" data-tauri-drag-region>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" title="打开文件" @click.stop="pickFile">
        <FilePlus class="h-4 w-4" />
      </button>
      <div class="flex min-w-0 flex-1 items-end gap-1 overflow-x-auto" data-tauri-drag-region>
        <div
          v-for="tab in tabs"
          :key="tab.path"
          class="group flex h-9 max-w-64 shrink-0 items-center gap-2 rounded-t-box border border-b-0 px-3 text-sm"
          :class="tab.path === activePath ? 'border-base-300 bg-base-100 text-base-content' : 'border-transparent bg-base-200 text-base-content/65 hover:bg-base-100/70 hover:text-base-content'"
          :title="tab.path"
          role="button"
          tabindex="0"
          :aria-selected="tab.path === activePath"
          @click="setActiveTab(tab.path)"
          @keydown.enter.prevent="setActiveTab(tab.path)"
          @keydown.space.prevent="setActiveTab(tab.path)"
        >
          <FileText class="h-4 w-4 shrink-0 opacity-70" />
          <span class="min-w-0 truncate font-medium">{{ tab.title }}</span>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-5 min-h-5 w-5 p-0 opacity-60 hover:opacity-100"
            title="关闭"
            @click.stop="closeTab(tab.path)"
          >
            <X class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" title="最小化" @click.stop="minimizeWindow">
        <Minus class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" :title="maximized ? '还原窗口' : '最大化'" @click.stop="toggleMaximizeWindow">
        <Square class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-sm btn-ghost shrink-0 hover:bg-error" type="button" title="关闭" @click.stop="closeWindow">
        <X class="h-3.5 w-3.5" />
      </button>
    </header>

    <div
      v-if="activeTab"
      class="flex h-9 shrink-0 items-center gap-1 overflow-x-auto border-x border-b border-base-300 bg-base-100 px-3 text-sm text-base-content/60"
    >
      <template v-for="segment in activePathSegments" :key="segment.key">
        <span v-if="segment.index > 0" class="shrink-0 text-base-content/35">›</span>
        <button
          type="button"
          class="inline-flex shrink-0 items-center rounded px-1.5 py-1 hover:bg-base-200 hover:text-base-content"
          :title="`打开目录：${segment.path}`"
          @click="showInDirectory(segment.path)"
        >
          {{ segment.label }}
        </button>
      </template>
      <span v-if="activePathSegments.length > 0" class="shrink-0 text-base-content/35">›</span>
      <button
        type="button"
        class="inline-flex shrink-0 items-center rounded px-1.5 py-1 font-medium text-base-content/80 hover:bg-base-200 hover:text-base-content"
        :title="`打开文件：${activeTab.path}`"
        @click="openFileDefault(activeTab.path)"
      >
        {{ activeTab.title }}
      </button>
      <div class="min-w-4 flex-1"></div>
      <button class="btn btn-ghost btn-xs ml-1 shrink-0" type="button" :disabled="!activeTab" title="刷新" @click.stop="refreshActiveTab">
        <RefreshCw class="h-4 w-4" />
      </button>
    </div>

    <main class="min-h-0 flex-1 overflow-auto bg-base-100" :class="activeTab?.kind === 'markdown' ? '' : 'file-reader-code-main'">
      <div v-if="!activeTab" class="flex h-full items-center justify-center text-sm text-base-content/55">
        还没有打开文件。
      </div>
      <div v-else-if="activeTab.loading" class="flex h-full items-center justify-center gap-3 text-sm text-base-content/65">
        <span class="loading loading-spinner loading-sm"></span>
        正在读取文件
      </div>
      <div v-else-if="activeTab.error" class="m-4 rounded-box border border-error/30 bg-error/10 p-4 text-sm text-error">
        {{ activeTab.error }}
      </div>
      <div v-else-if="activeTab.kind === 'markdown'" class="file-reader-content mx-auto w-full max-w-[1200px] px-4 py-4">
        <MarkdownRender
          class="ecall-markdown-content max-w-none"
          custom-id="chat-markstream"
          :nodes="activeMarkdownNodes"
          :is-dark="markdownIsDark"
          :final="true"
          :max-live-nodes="0"
          :batch-rendering="false"
          :initial-render-batch-size="0"
          :render-batch-size="0"
          :render-batch-delay="0"
          :render-batch-budget-ms="0"
          :code-block-props="markdownCodeBlockProps"
          :mermaid-props="markdownMermaidProps"
          :typewriter="false"
        />
      </div>
      <div v-else class="file-reader-code-view" v-html="activeHighlightedCodeHtml"></div>
    </main>

    <div v-if="actionErrorMessage" class="toast toast-end toast-bottom z-50">
      <div class="alert alert-error max-w-xl text-sm shadow-lg">
        <span>{{ actionErrorMessage }}</span>
      </div>
    </div>
    <Win10ResizeHandles :enabled="!maximized" />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FilePlus, FileText, Minus, RefreshCw, Square, X } from "lucide-vue-next";
import MarkdownRender, { enableKatex, enableMermaid, getMarkdown, parseMarkdownToStructure } from "markstream-vue";
import { bundledLanguagesInfo, codeToHtml } from "shiki";
import "markstream-vue/index.css";
import { invokeTauri } from "../../services/tauri-api";
import { registerChatMarkstreamComponents } from "../../features/chat/markdown/register-chat-markstream";
import { useAppTheme } from "../../features/shell/composables/use-app-theme";
import Win10ResizeHandles from "../../features/shell/components/Win10ResizeHandles.vue";

enableMermaid();
enableKatex();
registerChatMarkstreamComponents();

type FileReaderFilePayload = {
  path: string;
  name: string;
  extension: string;
  kind: "markdown" | "code" | string;
  content: string;
};

const SHIKI_LANGUAGE_KEYS = new Set(
  bundledLanguagesInfo.flatMap((item) => [item.id, ...(item.aliases || [])]).map((item) => item.toLowerCase()),
);

type FileTab = {
  path: string;
  title: string;
  extension: string;
  kind: string;
  content: string;
  loading: boolean;
  error: string;
};

const CODE_LANGUAGE_BY_EXTENSION: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  c: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  h: "c",
  hpp: "cpp",
  cs: "csharp",
  java: "java",
  kt: "kotlin",
  kts: "kotlin",
  go: "go",
  js: "javascript",
  jsx: "jsx",
  vue: "vue",
  rs: "rust",
  py: "python",
  rb: "ruby",
  php: "php",
  swift: "swift",
  scala: "scala",
  dart: "dart",
  lua: "lua",
  r: "r",
  m: "objective-c",
  mm: "objective-cpp",
  pl: "perl",
  pm: "perl",
  json: "json",
  jsonc: "jsonc",
  json5: "json5",
  toml: "toml",
  yaml: "yaml",
  yml: "yaml",
  css: "css",
  scss: "scss",
  sass: "sass",
  less: "less",
  html: "html",
  htm: "html",
  xml: "xml",
  svg: "xml",
  sql: "sql",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  fish: "fish",
  ps1: "powershell",
  bat: "bat",
  cmd: "bat",
  dockerfile: "dockerfile",
  ini: "ini",
  env: "dotenv",
  gitignore: "gitignore",
  gitattributes: "gitignore",
  editorconfig: "ini",
  lock: "text",
  csv: "csv",
  tsv: "tsv",
  txt: "text",
  log: "log",
  md: "markdown",
  markdown: "markdown",
  mdx: "mdx",
};

const markdownCodeBlockProps = {
  showHeader: true,
  showCopyButton: true,
  showPreviewButton: false,
  showExpandButton: true,
  showCollapseButton: true,
  showFontSizeButtons: true,
  enableFontSizeControl: true,
  isShowPreview: false,
  showTooltips: false,
};

const markdownMermaidProps = {
  showHeader: true,
  showCopyButton: true,
  showExportButton: false,
  showFullscreenButton: true,
  showCollapseButton: false,
  showZoomControls: true,
  showModeToggle: false,
  enableWheelZoom: true,
  showTooltips: false,
};

const markstreamMarkdown = getMarkdown();
const { currentTheme, restoreThemeFromStorage } = useAppTheme();
const tabs = ref<FileTab[]>([]);
const activePath = ref("");
const actionErrorMessage = ref("");
const highlightedCodeHtmlByPath = ref<Record<string, string>>({});
const markdownIsDark = computed(() => currentTheme.value !== "light");
const appWindow = getCurrentWindow();
const maximized = ref(false);
let unlistenOpenPath: UnlistenFn | null = null;

const activeTab = computed(() => tabs.value.find((tab) => tab.path === activePath.value) || tabs.value[0] || null);

const activeMarkdownSource = computed(() => {
  const tab = activeTab.value;
  if (!tab) return "";
  return tab.kind === "markdown" ? tab.content : "";
});

const activeMarkdownNodes = computed(() =>
  parseMarkdownToStructure(activeMarkdownSource.value, markstreamMarkdown, { final: true })
);

const activeHighlightedCodeHtml = computed(() => {
  const tab = activeTab.value;
  if (!tab) return "";
  return highlightedCodeHtmlByPath.value[tab.path] || escapeHtml(tab.content);
});

const activePathSegments = computed(() => {
  const tab = activeTab.value;
  if (!tab) return [];
  const normalized = normalizePath(tab.path);
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length <= 1) return [];
  const dirs = parts.slice(0, -1);
  return dirs.map((label, index) => ({
    key: `${index}:${label}`,
    index,
    label,
    path: dirs.slice(0, index + 1).join("/"),
  }));
});

function normalizePath(path: string) {
  return String(path || "")
    .trim()
    .replace(/^\\\\\?\\/, "")
    .replace(/^\/\?\//, "")
    .replace(/^\?\//, "")
    .replace(/^\?\\/, "")
    .replace(/\\/g, "/");
}

function extensionFromPath(path: string) {
  const fileName = titleFromPath(path);
  const lowerFileName = fileName.toLowerCase();
  if (CODE_LANGUAGE_BY_EXTENSION[lowerFileName]) return lowerFileName;
  if (SHIKI_LANGUAGE_KEYS.has(lowerFileName)) return lowerFileName;
  const dotIndex = fileName.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return "";
  const extension = fileName.slice(dotIndex + 1).toLowerCase();
  return CODE_LANGUAGE_BY_EXTENSION[extension] || SHIKI_LANGUAGE_KEYS.has(extension) ? extension : "";
}

function escapeHtml(value: string) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function normalizeShikiLineHtml(html: string) {
  return html.replace(/<\/span>\s+<span class="line"/g, '</span><span class="line"');
}

async function updateHighlightedCode(tab: FileTab) {
  if (tab.kind === "markdown") return;
  const language = resolveShikiLanguage(tab.extension);
  try {
    const html = await codeToHtml(tab.content, {
      lang: language,
      theme: "github-dark",
    });
    highlightedCodeHtmlByPath.value = {
      ...highlightedCodeHtmlByPath.value,
      [tab.path]: normalizeShikiLineHtml(html),
    };
  } catch {
    highlightedCodeHtmlByPath.value = {
      ...highlightedCodeHtmlByPath.value,
      [tab.path]: escapeHtml(tab.content),
    };
  }
}

function resolveShikiLanguage(extension: string) {
  const key = String(extension || "").trim().toLowerCase();
  const mapped = CODE_LANGUAGE_BY_EXTENSION[key] || key;
  return SHIKI_LANGUAGE_KEYS.has(mapped) ? mapped : "text";
}

function titleFromPath(path: string) {
  const normalized = normalizePath(path);
  return normalized.split("/").filter(Boolean).pop() || normalized || "未命名文件";
}

function upsertLoadingTab(path: string) {
  const normalizedPath = normalizePath(path);
  const existing = tabs.value.find((tab) => tab.path === normalizedPath);
  if (existing) {
    existing.loading = true;
    existing.error = "";
    activePath.value = existing.path;
    return existing;
  }
  const tab: FileTab = {
    path: normalizedPath,
    title: titleFromPath(normalizedPath),
    extension: "",
    kind: "code",
    content: "",
    loading: true,
    error: "",
  };
  tabs.value = [...tabs.value, tab];
  activePath.value = normalizedPath;
  return tab;
}

function setActiveTab(path: string) {
  activePath.value = path;
}

function deleteHighlightedCode(path: string) {
  const normalizedPath = normalizePath(path);
  const next = { ...highlightedCodeHtmlByPath.value };
  delete next[normalizedPath];
  highlightedCodeHtmlByPath.value = next;
}

function migrateTabPath(tab: FileTab, fromPath: string, toPath: string) {
  if (fromPath === toPath) return tab;
  const duplicated = tabs.value.find((item) => item !== tab && item.path === toPath);
  if (duplicated) {
    tabs.value = tabs.value.filter((item) => item !== tab);
    deleteHighlightedCode(fromPath);
    activePath.value = toPath;
    return duplicated;
  }
  const previousHighlightedHtml = highlightedCodeHtmlByPath.value[fromPath];
  if (previousHighlightedHtml) {
    const next = { ...highlightedCodeHtmlByPath.value };
    delete next[fromPath];
    next[toPath] = previousHighlightedHtml;
    highlightedCodeHtmlByPath.value = next;
  } else {
    deleteHighlightedCode(fromPath);
  }
  tab.path = toPath;
  activePath.value = toPath;
  return tab;
}

function reportFileReaderActionFailure(action: string, path: string, error: unknown) {
  const detail = error instanceof Error ? error.message : String(error);
  console.error(`[文件阅读窗口] ${action}失败`, {
    path,
    error,
  });
  actionErrorMessage.value = `${action}失败：${path}；${detail}`;
  window.setTimeout(() => {
    if (actionErrorMessage.value === `${action}失败：${path}；${detail}`) {
      actionErrorMessage.value = "";
    }
  }, 4500);
}

async function openPath(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  let tab = upsertLoadingTab(normalizedPath);
  try {
    const payload = await invokeTauri<FileReaderFilePayload>("read_file_reader_file", { path: normalizedPath });
    const resolvedPath = normalizePath(payload.path || normalizedPath);
    tab = migrateTabPath(tab, normalizedPath, resolvedPath);
    tab.title = payload.name || titleFromPath(resolvedPath);
    tab.extension = String(payload.extension || extensionFromPath(resolvedPath)).toLowerCase();
    tab.kind = String(payload.kind || "code");
    tab.content = String(payload.content || "");
    tab.error = "";
    tab.loading = false;
    activePath.value = resolvedPath;
    await updateHighlightedCode(tab);
  } catch (error) {
    tab.loading = false;
    tab.error = error instanceof Error ? error.message : String(error);
  }
}

function closeTab(path: string) {
  const index = tabs.value.findIndex((tab) => tab.path === path);
  if (index < 0) return;
  const wasActive = activePath.value === path;
  tabs.value = tabs.value.filter((tab) => tab.path !== path);
  deleteHighlightedCode(path);
  if (wasActive) {
    activePath.value = tabs.value[Math.max(0, index - 1)]?.path || tabs.value[0]?.path || "";
  }
}

function refreshActiveTab() {
  const tab = activeTab.value;
  if (!tab) return;
  void openPath(tab.path);
}

async function pickFile() {
  const picked = await open({
    multiple: false,
    directory: false,
    title: "打开文件",
  });
  if (!picked || Array.isArray(picked)) return;
  await openPath(String(picked));
}

async function showInDirectory(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await invokeTauri("open_local_file_directory", { path: normalizedPath });
  } catch (error) {
    reportFileReaderActionFailure("打开目录", normalizedPath, error);
  }
}

async function openFileDefault(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await invokeTauri("open_local_file_default", { path: normalizedPath });
  } catch (error) {
    reportFileReaderActionFailure("打开文件", normalizedPath, error);
  }
}

async function syncWindowState() {
  try {
    maximized.value = await appWindow.isMaximized();
  } catch {
    maximized.value = false;
  }
}

async function minimizeWindow() {
  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow.toggleMaximize();
  await syncWindowState();
}

async function closeWindow() {
  await appWindow.hide();
}

onMounted(() => {
  restoreThemeFromStorage();
  void syncWindowState();
  const path = new URLSearchParams(window.location.search).get("path") || "";
  void openPath(path);
  void listen<{ path?: string }>("file-reader-open-path", (event) => {
    void openPath(event.payload?.path || "");
  }).then((unlisten) => {
    unlistenOpenPath = unlisten;
  });
});

onBeforeUnmount(() => {
  unlistenOpenPath?.();
});
</script>

<style scoped>
.file-reader-content :deep(.markdown-body),
.file-reader-content :deep(.markstream-body) {
  max-width: none;
}

.file-reader-code-main {
  background: #101828;
}

.file-reader-code-view {
  min-height: 100%;
  background: #101828;
}

.file-reader-code-view :deep(.shiki) {
  min-height: 100%;
  margin: 0;
  padding: 0.75rem 0;
  border: 0;
  border-radius: 0;
  background: #101828 !important;
  box-shadow: none;
  overflow: visible;
  counter-reset: file-reader-code-line;
}

.file-reader-code-view :deep(code) {
  display: block;
  min-width: max-content;
}

.file-reader-code-view :deep(.line) {
  display: block;
  min-height: 1.5em;
  line-height: 1.5;
  counter-increment: file-reader-code-line;
}

.file-reader-code-view :deep(.line::before) {
  content: counter(file-reader-code-line);
  display: inline-block;
  width: 2.75rem;
  padding-right: 0.75rem;
  text-align: right;
  color: #64748b;
  user-select: none;
}
</style>