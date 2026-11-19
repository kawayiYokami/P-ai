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

// 组件库是独立的整页文档，靠窗口滚动。主应用给桌面窗口定的满高与隐藏溢出在这里不适用，
// 内联样式优先级高于 style.css，不必用 !important 去抢。
document.documentElement.style.height = "auto";
document.body.style.height = "auto";
document.body.style.minHeight = "100%";
document.body.style.overflowY = "auto";

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
