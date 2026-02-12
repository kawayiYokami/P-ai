import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type ViewMode = "chat" | "archives" | "config";

type AppBootstrapOptions = {
  setViewMode: (mode: ViewMode) => void;
  initWindowMode: () => ViewMode;
  onThemeChanged: (theme: string) => void;
  onLocaleChanged: (locale: string) => void;
  onRefreshSignal: () => Promise<void>;
};

export function useAppBootstrap(options: AppBootstrapOptions) {
  const unlisteners: UnlistenFn[] = [];

  async function mount() {
    const mode = options.initWindowMode();
    options.setViewMode(mode);

    unlisteners.push(
      await listen<string>("easy-call:theme-changed", (event) => {
        options.onThemeChanged(event.payload);
      }),
    );
    unlisteners.push(
      await listen<string>("easy-call:locale-changed", (event) => {
        options.onLocaleChanged(event.payload);
      }),
    );
    unlisteners.push(
      await listen("easy-call:refresh", async () => {
        await options.onRefreshSignal();
      }),
    );
  }

  function unmount() {
    while (unlisteners.length > 0) {
      const fn = unlisteners.pop();
      if (fn) fn();
    }
  }

  return {
    mount,
    unmount,
  };
}

