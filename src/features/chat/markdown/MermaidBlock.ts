import { defineComponent, h, onBeforeUnmount, ref, watch } from "vue";
import { renderMermaidSVG } from "beautiful-mermaid";
import { writeTransportClipboardText } from "../../../services/tauri-api";
import AppIcon from "../../shared/components/AppIcon.vue";

export function normalizeMermaidCodeForRender(code: string): string {
  return code.replace(/\\n/gi, "<br/>");
}

export function patchSvgToUsePaiFontSystem(svg: string): string {
  // 1. 移除 beautiful-mermaid 自带的 Google Fonts @import 外网请求，避免离线/弱网超时与请求失败
  // 2. 将 text 统一对齐 PAI 界面字体系统 --app-font-family
  // 3. 将 .mono 代码/字段统一对齐 PAI 内置代码字体系统 --app-code-font-family ("Easy Call JetBrains Mono")
  return svg
    .replace(/@import\s+url\([^)]+\);\s*/gi, "")
    .replace(/text\s*\{\s*font-family:\s*[^;]+;/gi, "text { font-family: var(--app-font-family, system-ui, sans-serif);")
    .replace(/\.mono\s*\{\s*font-family:\s*[^;]+;/gi, ".mono { font-family: var(--app-code-font-family); }");
}

const MermaidBlock = defineComponent({
  name: "MermaidBlock",
  props: {
    code: { type: String, default: "" },
    blockKey: { type: String, default: "" },
    isDark: { type: Boolean, default: false },
    streaming: { type: Boolean, default: false },
    copyText: { type: String, default: "Copy" },
    copiedText: { type: String, default: "Copied" },
    copyFailedText: { type: String, default: "Copy failed" },
    preparingText: { type: String, default: "" },
  },
  setup(mermaidProps) {
    const svgHtml = ref("");
    const error = ref("");
    const copied = ref(false);
    const copyFailed = ref(false);
    const renderPending = ref(false);
    const containerRef = ref<HTMLElement | null>(null);
    let renderCount = 0;
    let renderTimer = 0;
    let copyTimer = 0;

    function renderMermaid() {
      const rawCode = mermaidProps.code.trim();
      if (!rawCode) {
        svgHtml.value = "";
        error.value = "";
        renderPending.value = false;
        return;
      }
      renderPending.value = true;
      renderCount += 1;
      const currentRender = renderCount;

      const normalizedCode = normalizeMermaidCodeForRender(rawCode);

      try {
        const rawSvg = renderMermaidSVG(normalizedCode, {
          transparent: true,
          // 纯粹取 PAI 当前生效的主题变量，严格分为两档文字颜色体系：
          // 1. 主文字色 (fg): 100% --color-base-content（主视线高亮）
          // 2. 次级文字色 (muted): 60% base-content 混底色（连线分支“是/否”、属性类型、坐标刻度）
          bg: "var(--color-base-100)",
          fg: "var(--color-base-content)",
          surface: "color-mix(in srgb, var(--color-primary) 8%, var(--color-base-100))",
          border: "color-mix(in srgb, var(--color-primary) 30%, var(--color-base-300))",
          line: "color-mix(in srgb, var(--color-primary) 35%, color-mix(in srgb, var(--color-base-content) 35%, var(--color-base-100)))",
          accent: "var(--color-primary)",
          muted: "color-mix(in srgb, var(--color-base-content) 60%, var(--color-base-100))",
        });
        if (currentRender !== renderCount) return;
        svgHtml.value = patchSvgToUsePaiFontSystem(rawSvg);
        error.value = "";
      } catch (e: any) {
        if (currentRender !== renderCount) return;
        if (!svgHtml.value) {
          error.value = String(e?.message || "Mermaid render error");
        } else {
          error.value = "";
          console.warn("[Mermaid] 增量解析失败，保留上一版图表", e);
        }
      } finally {
        if (currentRender === renderCount) {
          renderPending.value = false;
        }
      }
    }

    function scheduleRenderMermaid() {
      if (renderTimer) {
        clearTimeout(renderTimer);
        renderTimer = 0;
      }
      if (!mermaidProps.code.trim()) {
        renderCount += 1;
        svgHtml.value = "";
        error.value = "";
        renderPending.value = false;
        return;
      }
      renderPending.value = true;
      const delay = mermaidProps.streaming ? 240 : 20;
      renderTimer = window.setTimeout(() => {
        renderTimer = 0;
        renderMermaid();
      }, delay);
    }

    function flashCopyState(failed: boolean) {
      if (copyTimer) clearTimeout(copyTimer);
      copied.value = !failed;
      copyFailed.value = failed;
      copyTimer = window.setTimeout(() => {
        copied.value = false;
        copyFailed.value = false;
        copyTimer = 0;
      }, 1500);
    }

    async function copyMermaidCode() {
      try {
        flashCopyState(!(await writeTransportClipboardText(mermaidProps.code || "")));
      } catch {
        flashCopyState(true);
      }
    }

    watch(
      () => [mermaidProps.code, mermaidProps.isDark],
      () => scheduleRenderMermaid(),
      { immediate: true },
    );

    onBeforeUnmount(() => {
      if (renderTimer) {
        clearTimeout(renderTimer);
        renderTimer = 0;
      }
      if (copyTimer) {
        clearTimeout(copyTimer);
        copyTimer = 0;
      }
    });

    return () => {
      const copyButton = mermaidProps.code.trim()
        ? h("button", {
          type: "button",
          class: "ecall-md-mermaid-copy ecall-md-code-action",
          title: copyFailed.value ? mermaidProps.copyFailedText : (copied.value ? mermaidProps.copiedText : mermaidProps.copyText),
          "aria-label": copyFailed.value ? mermaidProps.copyFailedText : (copied.value ? mermaidProps.copiedText : mermaidProps.copyText),
          onClick: copyMermaidCode,
        }, [h(AppIcon, { name: copied.value ? "check" : "copy", class: "ecall-md-code-action-icon" })])
        : null;
      if (!svgHtml.value && !error.value) {
        return h("div", { class: "ecall-md-mermaid-shell" }, [
          copyButton,
          h("div", { class: "ecall-md-mermaid-loading" }, mermaidProps.preparingText),
        ]);
      }
      if (error.value) {
        return h("div", { class: "ecall-md-mermaid-shell ecall-md-mermaid-error" }, [
          copyButton,
          h("pre", null, [h("code", null, mermaidProps.code)]),
          h("div", { class: "ecall-md-mermaid-error-msg" }, error.value),
        ]);
      }
      return h("div", { class: "ecall-md-mermaid-shell" }, [
        copyButton,
        h("div", {
          ref: containerRef,
          class: ["ecall-md-mermaid-block", renderPending.value ? "ecall-md-mermaid-block-buffering" : ""],
          innerHTML: svgHtml.value,
        }),
      ]);
    };
  },
});

export default MermaidBlock;
