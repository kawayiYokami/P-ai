import { createApp } from "vue";
import ConfigApp from "./apps/config/ConfigApp.vue";
import "./style.css";
import "cropperjs/dist/cropper.css";
import "katex/dist/katex.min.css";
import { i18n } from "./i18n";

window.addEventListener("error", (event) => {
  const error = event.error || event;
  const message = error?.message || event.message || "未知错误";
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

createApp(ConfigApp).use(i18n).mount("#app");
