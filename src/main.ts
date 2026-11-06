import { createApp } from "vue";
import ConfigApp from "./apps/config/ConfigApp.vue";
import "./style.css";
import "./features/chat/markdown/markdown-content.css";
import "katex/dist/katex.min.css";
import { i18n } from "./i18n";
import { invokeTauri } from "./services/tauri-api";
import { initMarkdownAppearance } from "./features/shell/composables/use-markdown-appearance";
import { initUiSizeAppearance } from "./features/shell/composables/use-ui-size-appearance";
import { installNativeSelectionGuard } from "./utils/native-selection";

installNativeSelectionGuard();
initMarkdownAppearance();
initUiSizeAppearance();

function reportFrontendError(tag: string, message: string, stack: string) {
  console.error(`[${tag}] 消息: ${message}, 堆栈: ${stack}`);
  void invokeTauri<boolean>("append_runtime_log_probe", {
    message: `[聊天流诊断] ${tag} ${message} :: ${stack.slice(0, 1200)}`,
  }).catch(() => {});
}

// 监听全局错误事件
window.addEventListener("error", (event) => {
  const error = event.error || event;
  const message = error?.message || event.message || "未知错误";
  const stack = error?.stack || "无堆栈信息";
  reportFrontendError("全局错误", String(message), String(stack));
});

// 监听未处理的 Promise 拒绝
window.addEventListener("unhandledrejection", (event) => {
  let message: string;
  let stack: string;
  if (event.reason instanceof Error) {
    message = event.reason.message || "未知错误";
    stack = event.reason.stack || "无堆栈信息";
  } else {
    message = String(event.reason) || "未知拒绝原因";
    stack = "无堆栈信息";
  }
  reportFrontendError("未处理的Promise拒绝", message, stack);
});

createApp(ConfigApp).use(i18n).mount("#app");
