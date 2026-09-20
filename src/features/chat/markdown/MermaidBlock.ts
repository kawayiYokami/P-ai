import { defineComponent, h, onBeforeUnmount, ref, watch } from "vue";
import { Check, Copy } from "@lucide/vue";

export function normalizeMermaidCodeForRender(code: string): string {
  return code.replace(/\\n/gi, "<br/>");
}

// mermaid 渲染时会先移除文档里同 id 的元素；blockKey 是位置派生的（code-1、code-2…），
// 两处内容相同的图会算出同一个 id 并互相顶掉，所以 id 里必须带实例序号。
let mermaidInstanceSeq = 0;

const MermaidBlock = defineComponent({
  name: "MermaidBlock",
  props: {
    code: { type: String, default: "" },
    blockKey: { type: String, default: "" },
    isDark: { type: Boolean, default: false },
    streaming: { type: Boolean, default: false },
    copyText: { type: String, default: "Copy" },
    copiedText: { type: String, default: "Copied" },
    preparingText: { type: String, default: "" },
  },
  setup(mermaidProps) {
    const svgHtml = ref("");
    const error = ref("");
    const copied = ref(false);
    const renderPending = ref(false);
    const containerRef = ref<HTMLElement | null>(null);
    const instanceId = ++mermaidInstanceSeq;
    let renderCount = 0;
    let renderTimer = 0;
    let copyTimer = 0;

    async function renderMermaid() {
      const code = mermaidProps.code.trim();
      if (!code) {
        svgHtml.value = "";
        error.value = "";
        renderPending.value = false;
        return;
      }
      renderPending.value = true;
      renderCount += 1;
      const currentRender = renderCount;
      try {
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({
          startOnLoad: false,
          theme: mermaidProps.isDark ? "dark" : "default",
          securityLevel: "strict",
        });
        const id = `ecall-mermaid-${mermaidProps.blockKey}-i${instanceId}-${currentRender}`;
        const { svg } = await mermaid.render(id, normalizeMermaidCodeForRender(code));
        if (currentRender !== renderCount) return;
        svgHtml.value = svg;
        error.value = "";
      } catch (e: any) {
        if (currentRender !== renderCount) return;
        if (!svgHtml.value) {
          error.value = String(e?.message || "Mermaid render error");
        } else {
          error.value = "";
          console.warn("[Mermaid] 增量渲染失败，保留上一版图表", e);
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
      const delay = mermaidProps.streaming ? 280 : 40;
      renderTimer = window.setTimeout(() => {
        renderTimer = 0;
        void renderMermaid();
      }, delay);
    }

    async function copyMermaidCode() {
      try {
        await navigator.clipboard.writeText(mermaidProps.code || "");
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
          title: copied.value ? mermaidProps.copiedText : mermaidProps.copyText,
          "aria-label": copied.value ? mermaidProps.copiedText : mermaidProps.copyText,
          onClick: copyMermaidCode,
        }, [h(copied.value ? Check : Copy, { class: "ecall-md-code-action-icon" })])
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

