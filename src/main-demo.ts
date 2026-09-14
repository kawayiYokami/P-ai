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
    return () =>
      h(
        "main",
        {
          class: "min-h-screen bg-base-200/40 p-4 text-base-content antialiased",
        },
        [
          h(
            "div",
            { class: "mx-auto max-w-7xl" },
            [h(DemoTab, { initialKey: "config-cards" })],
          ),
        ],
      );
  },
};

createApp(App).use(i18n).provide(LUCIDE_CONTEXT, {}).mount("#app");
