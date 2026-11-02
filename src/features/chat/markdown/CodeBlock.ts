import { computed, defineComponent, h, inject, onBeforeUnmount, ref, watch } from "vue";
import { Check, Copy, Maximize2, Play } from "@lucide/vue";
import CodeBlockPreviewDialog from "../components/dialogs/CodeBlockPreviewDialog.vue";
import InteractiveHtmlSandbox from "../components/InteractiveHtmlSandbox.vue";
import MermaidBlock from "./MermaidBlock";
import { createHighlightStream } from "./streaming-highlight";
import { isInteractiveHtmlLang, isPlainHtmlLang } from "../utils/sandbox-html-builder";
import { CHAT_HTML_READER_KEY } from "../utils/chat-html-reader";

const HIGHLIGHT_CACHE_MAX = 80;
const highlightCache = new Map<string, string>();
function highlightCacheKey(code: string, lang: string, isDark: boolean): string {
  return `${lang || "text"}::${isDark ? "dark" : "light"}::${code}`;
}
function themeFor(isDark: boolean): "github-dark" | "github-light" {
  return isDark ? "github-dark" : "github-light";
}

const CodeBlock = defineComponent({
  name: "CodeBlock",
  props: {
    lang: { type: String, default: "" },
    code: { type: String, default: "" },
    blockKey: { type: String, default: "" },
    isDark: { type: Boolean, default: false },
    streaming: { type: Boolean, default: false },
    copyText: { type: String, default: "Copy" },
    copiedText: { type: String, default: "Copied" },
    expandText: { type: String, default: "Expand" },
    preparingText: { type: String, default: "" },
  },
  setup(codeProps) {
    const highlightedHtml = ref("");
    const copied = ref(false);
    const previewOpen = ref(false);
    let copyTimer = 0;
    let highlightAbort: AbortController | null = null;
    let highlightDebounceTimer = 0;
    const streamCtrl = createHighlightStream();
    let lastStreamCode = "";
    let lastStreamLang = "";
    let lastStreamDark: boolean | null = null;

    const isMermaid = computed(() => codeProps.lang === "mermaid");
    const isInteractiveHtml = computed(() => isInteractiveHtmlLang(codeProps.lang));
    const isPlainHtml = computed(() => isPlainHtmlLang(codeProps.lang));
    const sandboxEnabled = ref(false);
    // 「在右栏打开」由 ChatView 顶层提供；右栏自己渲染时拿不到（面板已 provide null）
    const htmlReader = inject(CHAT_HTML_READER_KEY, null);

    async function highlightFull() {
      if (isMermaid.value || isInteractiveHtml.value) return;
      if (!codeProps.code) {
        highlightedHtml.value = "";
        return;
      }
      const key = highlightCacheKey(codeProps.code, codeProps.lang, codeProps.isDark);
      const cached = highlightCache.get(key);
      if (cached !== undefined) {
        if (highlightAbort) {
          highlightAbort.abort();
          highlightAbort = null;
        }
        highlightedHtml.value = cached;
        return;
      }
      if (highlightAbort) highlightAbort.abort();
      highlightAbort = new AbortController();
      const signal = highlightAbort.signal;

      try {
        const { codeToHtml } = await import("shiki");
        if (signal.aborted) return;
        const html = await codeToHtml(codeProps.code, {
          lang: codeProps.lang || "text",
          theme: themeFor(codeProps.isDark),
        });
        if (signal.aborted) return;
        if (highlightCache.size >= HIGHLIGHT_CACHE_MAX) {
          const first = highlightCache.keys().next().value as string | undefined;
          if (first !== undefined) highlightCache.delete(first);
        }
        highlightCache.set(key, html);
        highlightedHtml.value = html;
      } catch {
        if (!signal.aborted) highlightedHtml.value = "";
      }
    }

    async function highlightStreamingIncremental() {
      if (isMermaid.value || isInteractiveHtml.value) return;
      const code = codeProps.code || "";
      if (!code) {
        highlightedHtml.value = "";
        lastStreamCode = "";
        return;
      }
      if (code.length > 8000) {
        // 超大块流式期仍保留纯文本，结束再走全量
        return;
      }
      const lang = codeProps.lang || "text";
      const theme = themeFor(codeProps.isDark);
      const forceReset = lastStreamLang !== lang || lastStreamDark !== codeProps.isDark || !code.startsWith(lastStreamCode);
      try {
        const html = await streamCtrl.update(code, lang, theme, forceReset);
        if (html) {
          highlightedHtml.value = html;
          const key = highlightCacheKey(code, lang, codeProps.isDark);
          if (highlightCache.size >= HIGHLIGHT_CACHE_MAX) {
            const first = highlightCache.keys().next().value as string | undefined;
            if (first !== undefined) highlightCache.delete(first);
          }
          highlightCache.set(key, html);
        }
      } catch {
        // 回退到全量
        await highlightFull();
      }
      lastStreamCode = code;
      lastStreamLang = lang;
      lastStreamDark = codeProps.isDark;
    }

    function scheduleHighlight() {
      if (isMermaid.value || isInteractiveHtml.value) return;
      if (!codeProps.code) {
        if (highlightDebounceTimer) {
          clearTimeout(highlightDebounceTimer);
          highlightDebounceTimer = 0;
        }
        if (highlightAbort) {
          highlightAbort.abort();
          highlightAbort = null;
        }
        streamCtrl.reset();
        lastStreamCode = "";
        highlightedHtml.value = "";
        return;
      }
      const key = highlightCacheKey(codeProps.code, codeProps.lang, codeProps.isDark);
      const cached = highlightCache.get(key);
      if (cached !== undefined) {
        if (highlightDebounceTimer) {
          clearTimeout(highlightDebounceTimer);
          highlightDebounceTimer = 0;
        }
        if (highlightAbort) {
          highlightAbort.abort();
          highlightAbort = null;
        }
        highlightedHtml.value = cached;
        lastStreamCode = codeProps.code;
        lastStreamLang = codeProps.lang || "text";
        lastStreamDark = codeProps.isDark;
        return;
      }
      if (codeProps.streaming) {
        // 流式走增量 tokenizer，每帧只 enqueue 增量
        if (highlightDebounceTimer) clearTimeout(highlightDebounceTimer);
        // 轻度合批：16ms 内多次 delta 合并为一次 enqueue，避免过细碎
        highlightDebounceTimer = window.setTimeout(() => {
          highlightDebounceTimer = 0;
          void highlightStreamingIncremental();
        }, 16);
        return;
      }
      // 非流式：语言/主题变化需重置流式状态，否则切主题后旧 tokenizer 残留
      streamCtrl.reset();
      lastStreamCode = "";
      if (highlightDebounceTimer) {
        clearTimeout(highlightDebounceTimer);
        highlightDebounceTimer = 0;
      }
      void highlightFull();
    }

    watch(
      () => [codeProps.code, codeProps.lang, codeProps.isDark, codeProps.streaming],
      (next, prev) => {
        const nextStreaming = !!next[3];
        const prevStreaming = !!(prev ? (prev as any[])[3] : false);
        if (prevStreaming && !nextStreaming) {
          // 流式结束：用全量结果收口一次，保证与非流式一致
          if (highlightDebounceTimer) {
            clearTimeout(highlightDebounceTimer);
            highlightDebounceTimer = 0;
          }
          void highlightFull();
          return;
        }
        // 语言或主题变化时重置增量状态，避免高亮错乱
        const nextLang = String(next[1] || "text");
        const prevLang = prev ? String((prev as any[])[1] || "text") : "";
        const nextDark = !!next[2];
        const prevDark = prev ? !!(prev as any[])[2] : null;
        if (prev && (nextLang !== prevLang || nextDark !== prevDark)) {
          streamCtrl.reset();
          lastStreamCode = "";
        }
        scheduleHighlight();
      },
      { immediate: true },
    );

    async function copyCode() {
      try {
        await navigator.clipboard.writeText(codeProps.code || "");
        copied.value = true;
        if (copyTimer) clearTimeout(copyTimer);
        copyTimer = window.setTimeout(() => {
          copied.value = false;
          copyTimer = 0;
        }, 1500);
      } catch {
        copied.value = false;
      }
    }

    function openPreview() {
      previewOpen.value = true;
    }

    function closePreview() {
      previewOpen.value = false;
    }

    onBeforeUnmount(() => {
      if (copyTimer) {
        clearTimeout(copyTimer);
        copyTimer = 0;
      }
      if (highlightDebounceTimer) {
        clearTimeout(highlightDebounceTimer);
        highlightDebounceTimer = 0;
      }
      if (highlightAbort) {
        highlightAbort.abort();
        highlightAbort = null;
      }
      streamCtrl.dispose();
    });

    return () => {
      if (isMermaid.value) {
        return h(MermaidBlock, {
          code: codeProps.code,
          blockKey: codeProps.blockKey,
          isDark: codeProps.isDark,
          streaming: codeProps.streaming,
          copyText: codeProps.copyText,
          copiedText: codeProps.copiedText,
          preparingText: codeProps.preparingText,
        });
      }

      // ```html:interactive —— 交互式沙箱：流式期间展示骨架卡片，闭合后挂载沙箱
      if (isInteractiveHtml.value || (isPlainHtml.value && sandboxEnabled.value)) {
        if (codeProps.streaming && isInteractiveHtml.value) {
          return h("div", { class: "ecall-md-sandbox-skeleton" }, [
            h("div", { class: "ecall-sandbox-toolbar" }, [
              h("span", { class: "ecall-sandbox-title" }, "正在编写交互组件…"),
            ]),
            h("div", { class: "ecall-sandbox-skeleton-body" }, [
              h("span", { class: "ecall-sandbox-loading-dot" }),
            ]),
          ]);
        }
        return h(InteractiveHtmlSandbox, {
          code: codeProps.code,
          isDark: codeProps.isDark,
          exitTitle: isInteractiveHtml.value ? "" : "返回代码",
          onExit: () => { sandboxEnabled.value = false; },
          // 流式期间不给入口：此时卡片还没写完，送到右栏的会是一份残缺 html
          openInReaderTitle: codeProps.streaming ? "" : (htmlReader?.label || ""),
          onOpenInReader: () => { htmlReader?.open({ code: codeProps.code }); },
        });
      }

      // 标题栏：左边语言名，右边复制按钮
      const titleBar = h("div", { class: "ecall-md-code-title" }, [
        h("span", { class: "ecall-md-code-lang" }, codeProps.lang || "text"),
        h("div", { class: "ecall-md-code-actions" }, [
          isPlainHtml.value
            ? h("button", {
              type: "button",
              class: "ecall-md-code-action",
              title: "交互预览",
              onClick: () => { sandboxEnabled.value = true; },
            }, [h(Play, { class: "ecall-md-code-action-icon" })])
            : null,
          h("button", {
            type: "button",
            class: "ecall-md-code-action",
            title: codeProps.expandText,
            onClick: openPreview,
          }, [h(Maximize2, { class: "ecall-md-code-action-icon" })]),
          h("button", {
            type: "button",
            class: "ecall-md-code-action ecall-md-code-copy",
            title: copied.value ? codeProps.copiedText : codeProps.copyText,
            "aria-label": copied.value ? codeProps.copiedText : codeProps.copyText,
            onClick: copyCode,
          }, [h(copied.value ? Check : Copy, { class: "ecall-md-code-action-icon" })]),
        ]),
      ]);

      // 代码区
      const codeArea = highlightedHtml.value
        ? h("div", { class: "ecall-md-code-body", innerHTML: highlightedHtml.value })
        : h("pre", { class: "ecall-md-code-body ecall-md-code-plain" }, [h("code", null, codeProps.code)]);

      // 圆角外壳
      return h("div", { class: "ecall-md-code-block" }, [
        titleBar,
        codeArea,
        h(CodeBlockPreviewDialog, {
          open: previewOpen.value,
          lang: codeProps.lang,
          code: codeProps.code,
          isDark: codeProps.isDark,
          onClose: closePreview,
        }),
      ]);
    };
  },
});

export default CodeBlock;

