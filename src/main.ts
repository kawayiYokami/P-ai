import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import "cropperjs/dist/cropper.css";
import { i18n } from "./i18n";

window.addEventListener("error", (event) => {
  console.error("[GLOBAL-ERROR]", event.error || event.message || event);
});

window.addEventListener("unhandledrejection", (event) => {
  console.error("[GLOBAL-UNHANDLED-REJECTION]", event.reason);
});

createApp(App).use(i18n).mount("#app");
