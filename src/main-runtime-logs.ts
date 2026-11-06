import { createApp } from "vue";
import RuntimeLogsApp from "./apps/runtime-logs/RuntimeLogsApp.vue";
import "./style.css";
import { initMarkdownAppearance } from "./features/shell/composables/use-markdown-appearance";
import { initUiSizeAppearance } from "./features/shell/composables/use-ui-size-appearance";
import { installNativeSelectionGuard } from "./utils/native-selection";

installNativeSelectionGuard();
initMarkdownAppearance();
initUiSizeAppearance();

window.addEventListener("error", (event) => {
  const error = event.error || event;
  const message = error?.message || event.message || "未知错误";
  const stack = error?.stack || "无堆栈信息";
  console.error(`[运行日志窗口][全局错误] 消息: ${message}, 堆栈: ${stack}`);
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
  console.error(`[运行日志窗口][未处理的Promise拒绝] 消息: ${message}, 堆栈: ${stack}`);
});

createApp(RuntimeLogsApp).mount("#app");
