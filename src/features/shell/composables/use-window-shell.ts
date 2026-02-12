import { ref, shallowRef } from "vue";
import { getCurrentWindow, Window as WebviewWindow } from "@tauri-apps/api/window";

export function useWindowShell() {
  const appWindow = shallowRef<WebviewWindow | null>(null);
  const windowReady = ref(false);
  const alwaysOnTop = ref(false);

  function initWindow(): "chat" | "archives" | "config" {
    const win = getCurrentWindow();
    appWindow.value = win;
    windowReady.value = true;
    if (win.label === "chat") return "chat";
    if (win.label === "archives") return "archives";
    return "config";
  }

  async function syncAlwaysOnTop() {
    if (!appWindow.value) return;
    try {
      alwaysOnTop.value = await appWindow.value.isAlwaysOnTop();
    } catch {
      alwaysOnTop.value = false;
    }
  }

  async function closeWindow() {
    if (!appWindow.value) return;
    await appWindow.value.hide();
  }

  async function startDrag() {
    if (!appWindow.value) return;
    await appWindow.value.startDragging();
  }

  async function toggleAlwaysOnTop() {
    if (!appWindow.value) return;
    alwaysOnTop.value = !alwaysOnTop.value;
    await appWindow.value.setAlwaysOnTop(alwaysOnTop.value);
  }

  return {
    appWindow,
    windowReady,
    alwaysOnTop,
    initWindow,
    syncAlwaysOnTop,
    closeWindow,
    startDrag,
    toggleAlwaysOnTop,
  };
}

