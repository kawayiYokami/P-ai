import { createApp, defineAsyncComponent, h } from "vue";
import "./style.css";
import "./features/chat/markdown/markdown-content.css";
import "katex/dist/katex.min.css";
import { i18n } from "./i18n";
import { initMarkdownAppearance } from "./features/shell/composables/use-markdown-appearance";
import { initUiSizeAppearance } from "./features/shell/composables/use-ui-size-appearance";
import { installNativeSelectionGuard } from "./utils/native-selection";

installNativeSelectionGuard();
initMarkdownAppearance();
initUiSizeAppearance();

const IS_DEV = import.meta.env.DEV;

// 组件库只在 dev 可用：生产包里这一块永远不加载，demo.html 只是一张说明页。
const ComponentGallery = defineAsyncComponent(() => import("./features/dev-gallery/ComponentGallery.vue"));

const App = {
  setup() {
    if (!IS_DEV) {
      return () =>
        h(
          "main",
          { class: "grid min-h-screen place-items-center bg-base-200 p-6 text-base-content" },
          [h("p", { class: "text-sm text-base-content/60" }, "组件库只在开发模式下可用。")],
        );
    }
    return () => h(ComponentGallery);
  },
};

createApp(App).use(i18n).mount("#app");
