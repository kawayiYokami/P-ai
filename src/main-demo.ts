import { createApp, h } from "vue";
import DemoTab from "./features/config/views/config-tabs/DemoTab.vue";
import "./style.css";
import "./features/chat/markdown/markdown-content.css";
import "katex/dist/katex.min.css";
import { i18n } from "./i18n";
import { initMarkdownAppearance } from "./features/shell/composables/use-markdown-appearance";
import { initUiSizeAppearance } from "./features/shell/composables/use-ui-size-appearance";
import { LUCIDE_CONTEXT } from "./lucide-context";
import { installNativeSelectionGuard } from "./utils/native-selection";

installNativeSelectionGuard();
initMarkdownAppearance();
initUiSizeAppearance();

const App = {
  setup() {
    const params = new URLSearchParams(window.location.search);
    const hasKey = !!params.get("key")?.trim();
    const bare = params.get("bare") === "1" || hasKey;
    // ?key= 直达截图：保留页面背景与留白，模拟配置页内容区，避免只看到裸内容没有参照。
    const pageClass = hasKey
      ? "min-h-screen bg-base-200 text-base-content antialiased"
      : bare
        ? "min-h-screen bg-white text-base-content antialiased"
        : "min-h-screen bg-base-200/40 p-4 text-base-content antialiased";
    return () =>
      h(
        "main",
        { class: pageClass },
        [
          h(
            "div",
            { class: bare ? "w-full" : "mx-auto max-w-7xl" },
            [h(DemoTab, { initialKey: demoInitialKey(), bare })],
          ),
        ],
      );
  },
};

// 允许用 ?key=persona-permission 之类的参数直达某一块 demo，便于无头截图。
function demoInitialKey(): string {
  const key = new URLSearchParams(window.location.search).get("key");
  return key && key.trim() ? key.trim() : "config-cards";
}

createApp(App).use(i18n).provide(LUCIDE_CONTEXT, {}).mount("#app");
