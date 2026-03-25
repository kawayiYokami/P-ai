import { ref, shallowRef } from "vue";
import { getCurrentWindow, Window as WebviewWindow } from "@tauri-apps/api/window";

export function useWindowShell() {
  const appWindow = shallowRef<WebviewWindow | null>(null);
  const windowReady = ref(false);
  const alwaysOnTop = ref(false);
  const maximized = ref(false);

  function initWindow(): "chat" | "archives" | "config" {
    const win = getCurrentWindow();
    appWindow.value = win;
    windowReady.value = true;
    void syncWindowControlsState();
    if (win.label === "chat") return "chat";
    if (win.label === "archives") return "archives";
    return "config";
  }

  async function syncWindowControlsState() {
    if (!appWindow.value) return;
    try {
      alwaysOnTop.value = await appWindow.value.isAlwaysOnTop();
    } catch {
      alwaysOnTop.value = false;
    }
    try {
      maximized.value = await appWindow.value.isMaximized();
    } catch {
      maximized.value = false;
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
    const desired = !alwaysOnTop.value;
    try {
      await appWindow.value.setAlwaysOnTop(desired);
      alwaysOnTop.value = desired;
    } catch (error) {
      console.error("[WINDOW] setAlwaysOnTop failed:", error);
    }
  }

  async function minimizeWindow() {
    if (!appWindow.value) return;
    try {
      await appWindow.value.minimize();
    } catch (error) {
      console.error("[WINDOW] minimize failed:", error);
    }
  }

  async function toggleMaximizeWindow() {
    if (!appWindow.value) return;
    try {
      await appWindow.value.toggleMaximize();
      maximized.value = await appWindow.value.isMaximized();
    } catch (error) {
      console.error("[WINDOW] toggleMaximize failed:", error);
    }
  }

  return {
    appWindow,
    windowReady,
    alwaysOnTop,
    maximized,
    initWindow,
    syncWindowControlsState,
    closeWindow,
    startDrag,
    toggleAlwaysOnTop,
    minimizeWindow,
    toggleMaximizeWindow,
  };
}
