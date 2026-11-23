import { createApp, nextTick, ref, type App } from "vue";
import ConfirmDialogHost from "../components/ConfirmDialogHost.vue";

export type ConfirmOptions = {
  message: string;
  title?: string;
  confirmText?: string;
  cancelText?: string;
};

export type AlertOptions = {
  message: string;
  title?: string;
  confirmText?: string;
};

export type PromptOptions = ConfirmOptions & {
  defaultValue?: string;
  inputType?: "text" | "password";
  placeholder?: string;
};

type DialogKind = "confirm" | "alert" | "prompt";

type DialogRequest = {
  id: number;
  kind: DialogKind;
  options: ConfirmOptions | AlertOptions | PromptOptions;
  resolve: (value: unknown) => void;
};

/** 当前展示的弹窗；host 订阅它渲染，服务层不依赖组件上下文。 */
export const activeDialog = ref<DialogRequest | null>(null);

const queue: DialogRequest[] = [];
let hostApp: App | null = null;
let triggerElement: HTMLElement | null = null;
let sequence = 0;

// host 懒挂载到 body：所有入口（桌面与 Web）都无需改根组件即可调用。
// i18n 也在此刻才加载，避免服务模块被纯逻辑 spec 引入时触发 createI18n。
async function ensureHost() {
  if (hostApp || typeof document === "undefined") return;
  const { i18n } = await import("../../../i18n");
  const container = document.createElement("div");
  container.id = "pai-confirm-dialog-host";
  document.body.appendChild(container);
  hostApp = createApp(ConfirmDialogHost);
  hostApp.use(i18n);
  hostApp.mount(container);
}

function openNext() {
  const next = queue.shift() || null;
  activeDialog.value = next;
  if (next) void ensureHost();
}

function enqueue<T>(kind: DialogKind, options: ConfirmOptions | AlertOptions | PromptOptions): Promise<T> {
  return new Promise<T>((resolve) => {
    if (!activeDialog.value && triggerElement === null && typeof document !== "undefined") {
      const active = document.activeElement;
      triggerElement = active instanceof HTMLElement ? active : null;
    }
    queue.push({
      id: ++sequence,
      kind,
      options,
      resolve: resolve as (value: unknown) => void,
    });
    if (!activeDialog.value) openNext();
  });
}

/** 供 host 调用：结束当前弹窗，解析 Promise，并按需展示队列里的下一个。 */
export async function settleDialog(value: unknown): Promise<void> {
  const current = activeDialog.value;
  if (!current) return;
  activeDialog.value = null;
  current.resolve(value);
  if (queue.length > 0) {
    openNext();
    return;
  }
  // 等 host 把原生 dialog 关掉，再把焦点还给触发元素。
  await nextTick();
  if (triggerElement?.isConnected) triggerElement.focus();
  triggerElement = null;
}

export function confirmDialog(options: ConfirmOptions): Promise<boolean> {
  return enqueue<boolean>("confirm", options);
}

export function alertDialog(options: AlertOptions): Promise<void> {
  return enqueue<void>("alert", options);
}

export function promptDialog(options: PromptOptions): Promise<string | null> {
  return enqueue<string | null>("prompt", options);
}
