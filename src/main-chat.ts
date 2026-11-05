import { createApp } from "vue";
import ChatApp from "./apps/chat/ChatApp.vue";
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

window.addEventListener("error", (event) => {
  const error = event.error || event;
  const message = error?.message || event.message || "未知错误";
  // 过滤 ResizeObserver 规范性无害通知，避免控制台刷屏报错
  if (typeof message === "string" && (
    message.includes("ResizeObserver loop completed with undelivered notifications") ||
    message.includes("ResizeObserver loop limit exceeded")
  )) {
    return;
  }
  const stack = error?.stack || "无堆栈信息";
  console.error(`[全局错误] 消息: ${message}, 堆栈: ${stack}`);
});

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
  console.error(`[未处理的Promise拒绝] 消息: ${message}, 堆栈: ${stack}`);
});

createApp(ChatApp).use(i18n).provide(LUCIDE_CONTEXT, {}).mount("#app");
