import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, emitTo, listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { AttachmentReceipt } from "./attachment-transfer";
import type { CatalogInstallResult, CatalogPage, CatalogSourceInfo, SkillSummaryItem, SkillListResult } from "../types/app";

type WebBridgeConfig = {
  chatUrl: string;
  url?: string;
  token?: string;
  workspaceRoots?: Array<{ path?: string; name?: string }>;
};

// 远程前端模式：iframe 内电脑 PAI 页面与手机 PAI 壳层之间的密码认证消息源标识。
const REMOTE_AUTH_BRIDGE_SOURCE = "pai-remote-bridge-auth";

// 远程前端模式：手机 PAI 壳层 → 本页面的会话命令消息源标识（与认证方向相反）。
const REMOTE_COMMAND_BRIDGE_SOURCE = "pai-remote-bridge-command";

// 远程前端模式：允许与 iframe 内电脑 PAI 页面做 postMessage 桥接的父窗口 origin。
// 与手机 PAI 壳层约定：壳层页面必须以该 origin 加载（Tauri Android WebView 默认
// asset 协议为 https://tauri.localhost）。桌面独立窗口（self === top）与 VSCode
// 侧边栏（走 acquireVsCodeApi，不经 postMessage）不受影响；若壳层实际 origin
// 不同，需两端同步修改。
export const REMOTE_BRIDGE_ALLOWED_ORIGIN = "https://tauri.localhost";

export type TransportHostWorkspace = {
  path: string;
  name: string;
};

export type TransportHostContext = {
  workspaceRoots: TransportHostWorkspace[];
  launchConversationId: string;
};

export type TransportConnectionState = {
  configured: boolean;
  connected: boolean;
  connecting: boolean;
  ready: boolean;
  errorText: string;
};

type PendingWebBridgeRequest = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
  timer: number | null;
};

type PendingWebAttachmentChunk = {
  resolve: (value: { transferId: string; nextOffset: number }) => void;
  reject: (reason?: unknown) => void;
  timer: number;
};

type WebBridgeState = {
  configured: boolean;
  connected: boolean;
  connecting: boolean;
  bridgeReady: boolean;
  authRequired: boolean;
  authenticated: boolean;
  errorText: string;
};

type WebBridgeGlobals = Window & {
  __PAI_SIDEBAR_BRIDGE__?: WebBridgeConfig;
  __PAI_SETTINGS_BRIDGE__?: WebBridgeConfig;
};

/**
 * 统一通信层暴露给业务层的流式通道形状。
 *
 * 业务代码只依赖 onmessage，不得直接依赖 Tauri Channel。桌面运行时
 * 这里的对象就是原生 Channel；Web 运行时则是一个可安全从请求参数中
 * 剥离的虚拟通道（流式事件由桥接通知订阅承接）。
 */
export type TransportChannel<T> = {
  onmessage: ((message: T) => void) | null;
  dispose?: () => void;
};

type TransportChannelMeta = {
  native: object | null;
};

const transportChannelMeta = new WeakMap<object, TransportChannelMeta>();

type WebTransportStreamBinding = {
  conversationId: string;
  channel: TransportChannel<unknown>;
  stopDelta: () => void;
  stopProbe: () => void;
};

const webTransportStreamBindings = new Map<string, WebTransportStreamBinding>();
const webTransportStreamBindingVersions = new Map<string, number>();

const WEB_BRIDGE_TOKEN_STORAGE_PREFIX = "easy_call.web_bridge_token.v1:";

function webBridgeTokenStorageKey(chatUrl: string): string {
  return `${WEB_BRIDGE_TOKEN_STORAGE_PREFIX}${chatUrl.trim()}`;
}

function readPersistedWebBridgeToken(chatUrl: string): string {
  if (typeof window === "undefined") return "";
  return String(window.localStorage.getItem(webBridgeTokenStorageKey(chatUrl)) || "").trim();
}

function persistWebBridgeToken(chatUrl: string, token: string) {
  if (typeof window === "undefined") return;
  const normalizedChatUrl = String(chatUrl || "").trim();
  if (!normalizedChatUrl) return;
  const normalizedToken = String(token || "").trim();
  if (!normalizedToken) {
    window.localStorage.removeItem(webBridgeTokenStorageKey(normalizedChatUrl));
    return;
  }
  window.localStorage.setItem(webBridgeTokenStorageKey(normalizedChatUrl), normalizedToken);
}

function clearPersistedWebBridgeToken(chatUrl: string) {
  persistWebBridgeToken(chatUrl, "");
}

let webBridgeConfig: WebBridgeConfig | null = null;
let webBridgeSocket: WebSocket | null = null;
let webBridgeConnectPromise: Promise<void> | null = null;
let webBridgeRequestId = 1;
const webBridgePending = new Map<number, PendingWebBridgeRequest>();
const webBridgePendingAttachmentChunks = new Map<string, PendingWebAttachmentChunk>();
const webBridgeNotificationHandlers = new Map<string, Set<(payload: unknown) => void>>();
const webBridgeStateHandlers = new Set<(state: WebBridgeState) => void>();
const transportDiscoveryHandlers = new Set<(context: TransportHostContext) => void>();
let transportHostMessageListenerInstalled = false;
let transportDiscoveryRefreshRequestedAt = 0;
let webTransportConversationContextId = "";
let webTransportConversationContextKey = "";
let webBridgeAuthenticationPromise: Promise<void> | null = null;
const webBridgeState: WebBridgeState = {
  configured: false,
  connected: false,
  connecting: false,
  bridgeReady: false,
  authRequired: false,
  authenticated: true,
  errorText: "",
};

function notifyWebBridgeStateChanged() {
  const snapshot = { ...webBridgeState };
  for (const handler of webBridgeStateHandlers) handler(snapshot);
}

const WEB_BRIDGE_DEFAULT_TIMEOUT_MS = 30000;
const WEB_BRIDGE_LONG_TIMEOUT_MS = 5 * 60 * 1000;
const WEB_BRIDGE_VERY_LONG_TIMEOUT_MS = 30 * 60 * 1000;
const WEB_BRIDGE_NO_TIMEOUT_COMMANDS = new Set([
  "apply_prepared_github_update",
  "configMigration.apply",
  "cancel_github_update",
  "configMigration.export",
  "import_angel_memories",
  "import_memories",
  "install_host_runtime_prerequisite",
  "migrate_shell_workspace_directory",
  "mcp_deploy_server",
  "mcp_refresh_mcp_and_skills",
  "mcp_remove_server",
  "mcp_undeploy_server",
  "configMigration.preview",
  "save_memory_embedding_binding",
  "start_github_update",
]);

const WEB_BRIDGE_COMMAND_TIMEOUT_MS: Record<string, number> = {
  check_github_update: WEB_BRIDGE_LONG_TIMEOUT_MS,
  cleanup_storage_legacy_items: WEB_BRIDGE_VERY_LONG_TIMEOUT_MS,
  codex_get_rate_limits: WEB_BRIDGE_LONG_TIMEOUT_MS,
  codex_consume_rate_limit_reset_credit: WEB_BRIDGE_LONG_TIMEOUT_MS,
  codex_start_oauth_login: WEB_BRIDGE_LONG_TIMEOUT_MS,
  disable_agent_private_memory: WEB_BRIDGE_VERY_LONG_TIMEOUT_MS,
  export_agent_private_memories: WEB_BRIDGE_VERY_LONG_TIMEOUT_MS,
  export_memories: WEB_BRIDGE_LONG_TIMEOUT_MS,
  export_memories_to_path: WEB_BRIDGE_VERY_LONG_TIMEOUT_MS,
  fetch_model_metadata: WEB_BRIDGE_LONG_TIMEOUT_MS,
  mcp_list_server_tools: WEB_BRIDGE_LONG_TIMEOUT_MS,
  preview_import_angel_memories: WEB_BRIDGE_LONG_TIMEOUT_MS,
  quick_genai_chat: WEB_BRIDGE_LONG_TIMEOUT_MS,
  refresh_models: WEB_BRIDGE_LONG_TIMEOUT_MS,
  remote_im_weixin_oc_get_login_status: WEB_BRIDGE_LONG_TIMEOUT_MS,
  remote_im_weixin_oc_start_login: WEB_BRIDGE_LONG_TIMEOUT_MS,
  remote_im_weixin_oc_sync_contacts: WEB_BRIDGE_LONG_TIMEOUT_MS,
  remote_im_restart_channel: WEB_BRIDGE_LONG_TIMEOUT_MS,
  search_chat_history_slices: WEB_BRIDGE_LONG_TIMEOUT_MS,
  search_memories_mixed: WEB_BRIDGE_LONG_TIMEOUT_MS,
  task_optimize_draft: WEB_BRIDGE_LONG_TIMEOUT_MS,
  test_embedding_connection: WEB_BRIDGE_LONG_TIMEOUT_MS,
  test_memory_embedding_provider: WEB_BRIDGE_LONG_TIMEOUT_MS,
  test_memory_rerank_provider: WEB_BRIDGE_LONG_TIMEOUT_MS,
  test_rerank_connection: WEB_BRIDGE_LONG_TIMEOUT_MS,
  test_voice_connection: WEB_BRIDGE_LONG_TIMEOUT_MS,
};

export type TauriRuntimeProbeWindow = {
  top?: unknown;
  self: unknown;
  __TAURI_INTERNALS__?: { invoke?: unknown };
};

/** 探测当前是否运行在桌面 Tauri 原生环境（用于宿主能力判定）。
 *  被 iframe 嵌入（远程前端 / VSCode 侧边栏）时视为 Web 宿主：宿主 WebView
 *  可能注入 __TAURI_INTERNALS__（如手机 Tauri WebView 会注入到跨域 iframe），
 *  若按原生检测会误判为桌面 Tauri 环境，从而跳过 WS 桥接走 invoke。
 *  hostWindow 参数仅测试注入用；缺省取全局 window。 */
export function isTauriRuntimeAvailable(
  hostWindow?: TauriRuntimeProbeWindow,
): boolean {
  const win = hostWindow ?? (typeof window !== "undefined" ? window : undefined);
  if (!win) return false;
  if (typeof win.top !== "undefined" && win.self !== win.top) return false;
  const internals = (win as TauriRuntimeProbeWindow).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function";
}

export type TransportCapabilities = {
  windowControls: boolean;
  localFileSystem: boolean;
  localPathPicker: boolean;
};

/**
 * 文件对话框也是传输边界的一部分。
 *
 * 业务层只消费这个与运行时无关的结果；原生插件的动态 import 只能出现在
 * 适配器中。Web 端没有可返回本机路径的文件对话框时返回 null，由调用方按
 * 能力显隐处理（浏览器附件上传走 uploadTransportAttachment）。
 */
export type TransportFileDialogOptions = {
  multiple?: boolean;
  directory?: boolean;
  recursive?: boolean;
  title?: string;
  defaultPath?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
};

export async function openTransportFileDialog(
  options: TransportFileDialogOptions = {},
): Promise<string | string[] | null> {
  if (!isTauriRuntimeAvailable()) return null;
  const dialog = await import("@tauri-apps/plugin-dialog");
  return dialog.open(options as Parameters<typeof dialog.open>[0]);
}

/** 工作区目录/文件的宿主能力也只能在传输适配器内落到原生命令。 */
export async function openTransportWorkspaceDirectory(path: string): Promise<string | null> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return null;
  return invokeTauri<string>("open_chat_shell_workspace_dir", {
    input: { workspacePath: normalizedPath },
  });
}

function resolveTransportHostFilePath(path: string): string {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || /^(?:[a-zA-Z]:[\\/]|[\\/]{2}|\/)/.test(normalizedPath)) {
    return normalizedPath;
  }
  const root = getTransportHostContext().workspaceRoots[0]?.path || "";
  if (!root) return normalizedPath;
  return `${root.replace(/[\\/]+$/, "")}/${normalizedPath.replace(/^[\\/]+/, "")}`;
}

export async function openTransportWorkspaceFile(relativePath: string): Promise<boolean> {
  const normalizedPath = String(relativePath || "").trim();
  if (!normalizedPath) return false;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("open_workspace_file", { relativePath: normalizedPath });
    return true;
  }
  return openTransportLocalFileReference(resolveTransportHostFilePath(normalizedPath));
}

export async function saveTransportFileDialog(
  options: Omit<TransportFileDialogOptions, "multiple" | "directory" | "recursive"> = {},
): Promise<string | null> {
  if (!isTauriRuntimeAvailable()) return null;
  const dialog = await import("@tauri-apps/plugin-dialog");
  return dialog.save(options as Parameters<typeof dialog.save>[0]);
}

function browserFileAcceptValue(filters: TransportFileDialogOptions["filters"]): string {
  return (filters || [])
    .flatMap((filter) => filter.extensions || [])
    .map((extension) => String(extension || "").trim().replace(/^\./, ""))
    .filter(Boolean)
    .map((extension) => `.${extension}`)
    .join(",");
}

/** 浏览器 File 选择入口。导出仅为单测可直测安卓 focus/change 时序；生产调用面不变。 */
export function pickBrowserTransportFiles(options: TransportFileDialogOptions): Promise<File[]> {
  if (typeof document === "undefined") return Promise.resolve([]);
  return new Promise<File[]>((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = options.multiple !== false;
    input.style.display = "none";
    const accept = browserFileAcceptValue(options.filters);
    if (accept) input.accept = accept;
    document.body.appendChild(input);
    let settled = false;
    let focusTimer = 0;
    const finish = (files: File[]) => {
      if (settled) return;
      settled = true;
      if (focusTimer) window.clearTimeout(focusTimer);
      window.removeEventListener("focus", handleFocus);
      input.remove();
      resolve(files);
    };
    // 安卓「照片与视频」选择器返回时 focus 先于 change 触发，且 focus 时
    // input.files 尚未填充；直接在此刻 finish 会把已选文件静默丢弃。
    // 延迟后先看 change 是否已处理（settled），给 change 让路；
    // 超时仍未收到 change 时，files 非空视为选中、为空视为用户取消。
    // 窗口取 1000ms：Photo Picker 多选返回慢，change 可能晚于 focus 数百毫秒才到。
    const handleFocus = () => {
      focusTimer = window.setTimeout(() => {
        if (settled) return;
        finish(Array.from(input.files || []));
      }, 1000);
    };
    input.addEventListener("change", () => finish(Array.from(input.files || [])), { once: true });
    input.addEventListener("cancel", () => finish([]), { once: true });
    window.addEventListener("focus", handleFocus, { once: true });
    input.click();
  });
}

function readBrowserTransportFileAsBase64(file: File): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = String(reader.result || "");
      const marker = "base64,";
      const markerIndex = dataUrl.indexOf(marker);
      resolve(markerIndex >= 0 ? dataUrl.slice(markerIndex + marker.length) : dataUrl);
    };
    reader.onerror = () => reject(reader.error || new Error("读取浏览器文件失败"));
    reader.readAsDataURL(file);
  });
}

function downloadBrowserTransportBlob(fileName: string, blob: Blob) {
  if (typeof document === "undefined" || typeof URL === "undefined") {
    throw new Error("当前宿主无法下载文件");
  }
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = String(fileName || "download").trim() || "download";
  document.body.appendChild(link);
  try {
    link.click();
  } finally {
    link.remove();
    URL.revokeObjectURL(url);
  }
}

function downloadBrowserTransportJsonFile(fileName: string, payload: unknown) {
  downloadBrowserTransportBlob(
    fileName,
    new Blob([JSON.stringify(payload, null, 2)], { type: "application/json;charset=utf-8" }),
  );
}

function downloadBrowserTransportBase64File(fileName: string, bytesBase64: string, mime: string) {
  if (typeof window === "undefined" || typeof window.atob !== "function") {
    throw new Error("当前宿主无法解码下载文件");
  }
  const binary = window.atob(bytesBase64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  downloadBrowserTransportBlob(fileName, new Blob([bytes], { type: mime }));
}

/**
 * 统一附件选择与入库：业务层只接收后端附件回执，不得区分本机路径和浏览器 File。
 */
export async function pickTransportAttachments<T>(
  options: TransportFileDialogOptions = { multiple: true },
): Promise<T[]> {
  if (isTauriRuntimeAvailable()) {
    const selected = await openTransportFileDialog({ ...options, directory: false });
    const paths = (Array.isArray(selected) ? selected : [selected])
      .map((value) => String(value || "").trim())
      .filter(Boolean);
    const receipts: T[] = [];
    for (const path of paths) {
      receipts.push(await invokeTauri<T>("attachment_ingest_local_path", {
        input: { path },
      }));
    }
    return receipts;
  }
  const files = await pickBrowserTransportFiles({ ...options, directory: false });
  const receipts: T[] = [];
  for (const file of files) {
    receipts.push(await uploadTransportAttachment<T>(file));
  }
  return receipts;
}

export type TransportAttachmentSource = {
  id: string;
  fileName: string;
  path?: string;
  file?: File;
};

/**
 * 统一附件选择（只取文件描述，不读取内容）。
 * 业务层先展示“上传中”占位，再对每个 source 调用 ingestTransportAttachmentSource 读取，
 * 避免大文件读取期间界面无反馈。
 */
export async function pickTransportAttachmentSources(
  options: TransportFileDialogOptions = { multiple: true },
): Promise<TransportAttachmentSource[]> {
  if (isTauriRuntimeAvailable()) {
    const selected = await openTransportFileDialog({ ...options, directory: false });
    const paths = (Array.isArray(selected) ? selected : [selected])
      .map((value) => String(value || "").trim())
      .filter(Boolean);
    return paths.map((path) => {
      const normalized = path.replace(/\\/g, "/");
      const fileName = normalized.split("/").pop() || path;
      return { id: `local:${path}`, fileName, path };
    });
  }
  const files = await pickBrowserTransportFiles({ ...options, directory: false });
  return files.map((file, index) => ({
    id: `web:${file.name}:${file.lastModified}:${index}`,
    fileName: file.name || "attachment",
    file,
  }));
}

/** 统一读取单个已选附件，返回入库回执。 */
export async function ingestTransportAttachmentSource(
  source: TransportAttachmentSource,
): Promise<AttachmentReceipt> {
  if (source.path) {
    return await ingestTransportLocalAttachment<AttachmentReceipt>({
      path: source.path,
    });
  }
  if (source.file) {
    return await uploadTransportAttachment<AttachmentReceipt>(source.file);
  }
  throw new Error("附件来源缺少读取句柄");
}

type TransportHostMessage = {
  type?: string;
  discovery?: WebBridgeConfig;
  snapshot?: Record<string, unknown>;
};

type VsCodeHostApi = {
  postMessage: (message: unknown) => void;
};

let cachedVsCodeHostApi: VsCodeHostApi | null | undefined;

function getVsCodeHostApi(): VsCodeHostApi | null {
  if (cachedVsCodeHostApi !== undefined) return cachedVsCodeHostApi;
  if (typeof window === "undefined") return null;
  const hostWindow = window as Window & { acquireVsCodeApi?: () => VsCodeHostApi };
  try {
    cachedVsCodeHostApi = typeof hostWindow.acquireVsCodeApi === "function"
      ? hostWindow.acquireVsCodeApi()
      : null;
  } catch {
    cachedVsCodeHostApi = null;
  }
  return cachedVsCodeHostApi;
}

function postTransportHostMessage(message: unknown): boolean {
  if (typeof window === "undefined") return false;
  const vscodeApi = getVsCodeHostApi();
  if (vscodeApi) {
    vscodeApi.postMessage(message);
    return true;
  }
  if (window.parent && window.parent !== window) {
    window.parent.postMessage(message, REMOTE_BRIDGE_ALLOWED_ORIGIN);
    return true;
  }
  return false;
}

export function getTransportCapabilities(): TransportCapabilities {
  const native = isTauriRuntimeAvailable();
  return {
    windowControls: native,
    localFileSystem: native,
    localPathPicker: native,
  };
}

/** 应用更新仅依赖桌面宿主的原生更新能力，业务层无需自行探测运行时。 */
export function canUseTransportGithubUpdate(): boolean {
  return isTauriRuntimeAvailable();
}

/** 本机依赖检测仅桌面宿主提供；Web/VS Code 无本机环境可查，业务层直接读语义能力。 */
export function canUseTransportHostRuntimeCheck(): boolean {
  return isTauriRuntimeAvailable();
}

/** 按住说话录音：浏览器 MediaRecorder + getUserMedia 可用即支持，
 *  Web 宿主走远程 STT 上传，桌面宿主由业务层自选远程/本机识别路径。 */
export function canUseTransportSpeechRecording(): boolean {
  return typeof navigator !== "undefined"
    && !!navigator.mediaDevices?.getUserMedia
    && typeof MediaRecorder !== "undefined";
}

/** 是否运行在桌面 Tauri 宿主；Web/VS Code 宿主为 false。业务层用于宿主相关界面形态判定。 */
export function isDesktopTauriHost(): boolean {
  return isTauriRuntimeAvailable();
}

/** 系统字体枚举仅桌面宿主可查；Web/VS Code 无此能力，业务层直接读语义能力控制显隐。 */
export function canUseTransportSystemFonts(): boolean {
  return isTauriRuntimeAvailable();
}

/** genai 内置 chat 适配器清单仅桌面宿主可查；Web/VS Code 无此能力，业务层直接读语义能力控制显隐。 */
export function canUseTransportGenaiChatAdapters(): boolean {
  return isTauriRuntimeAvailable();
}

function nativeTransportConnectionState(): TransportConnectionState {
  return {
    configured: true,
    connected: true,
    connecting: false,
    ready: true,
    errorText: "",
  };
}

function transportConnectionStateFromWebBridge(state: WebBridgeState): TransportConnectionState {
  return {
    configured: state.configured,
    connected: state.connected,
    connecting: state.connecting,
    ready: state.bridgeReady && (!state.authRequired || state.authenticated),
    errorText: state.errorText,
  };
}

/** 业务层只读取统一连接状态，不判断当前由 IPC 还是网络传输承接。 */
export function getTransportConnectionState(): TransportConnectionState {
  if (isTauriRuntimeAvailable()) return nativeTransportConnectionState();
  return transportConnectionStateFromWebBridge(getWebBridgeState());
}

export function onTransportConnectionStateChange(
  handler: (state: TransportConnectionState) => void,
): () => void {
  if (isTauriRuntimeAvailable()) {
    handler(nativeTransportConnectionState());
    return () => {};
  }
  return onWebBridgeStateChange((state) => handler(transportConnectionStateFromWebBridge(state)));
}

export async function ensureTransportReady(): Promise<TransportConnectionState> {
  if (isTauriRuntimeAvailable()) return nativeTransportConnectionState();
  await connectWebBridge();
  await ensureWebBridgeAuthenticated();
  return transportConnectionStateFromWebBridge(getWebBridgeState());
}

/** 传输连接状态恢复信号；恢复路径与业务无关，订阅方据此重拉快照兜底漏掉的事件。 */
type TransportRecoveredSource = "reconnect" | "foreground";

const transportRecoveredHandlers = new Set<(source: TransportRecoveredSource) => void>();

export function onTransportRecovered(handler: (source: TransportRecoveredSource) => void): () => void {
  transportRecoveredHandlers.add(handler);
  return () => {
    transportRecoveredHandlers.delete(handler);
  };
}

function emitTransportRecovered(source: TransportRecoveredSource): void {
  for (const handler of [...transportRecoveredHandlers]) handler(source);
}

export async function reconnectTransport(): Promise<TransportConnectionState> {
  if (isTauriRuntimeAvailable()) return nativeTransportConnectionState();
  disconnectWebBridge();
  const state = await ensureTransportReady();
  emitTransportRecovered("reconnect");
  return state;
}

export function disconnectTransport(): void {
  if (!isTauriRuntimeAvailable()) disconnectWebBridge();
}

export async function authenticateTransport(password: string): Promise<TransportConnectionState> {
  if (isTauriRuntimeAvailable()) return nativeTransportConnectionState();
  return transportConnectionStateFromWebBridge(await loginWebBridge(password));
}

export async function pingTransport(timeoutMs = 2500): Promise<void> {
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("is_backend_ready");
    return;
  }
  await invokeTauri("bridge.ping", {}, timeoutMs);
}

/** 前台重新获得焦点时统一恢复传输；IPC 与 Web 的连接细节只留在适配器。 */
export async function restoreTransportAfterForegroundWake(timeoutMs = 2500): Promise<TransportConnectionState> {
  try {
    await ensureTransportReady();
    await pingTransport(timeoutMs);
  } catch (error) {
    await reconnectTransport();
    await pingTransport(timeoutMs).catch(() => { throw error; });
  }
  emitTransportRecovered("foreground");
  return getTransportConnectionState();
}

export function getTransportHostContext(): TransportHostContext {
  const config = isTauriRuntimeAvailable() ? null : ensureWebBridgeConfig();
  const workspaceRoots = (Array.isArray(config?.workspaceRoots) ? config.workspaceRoots : [])
    .map((item) => ({
      path: String(item?.path || "").trim(),
      name: String(item?.name || "").trim(),
    }))
    .filter((item) => !!item.path);
  const params = typeof window === "undefined"
    ? null
    : new URLSearchParams(window.location.search || "");
  return {
    workspaceRoots,
    launchConversationId: String(params?.get("conversationId") || "").trim(),
  };
}

function ensureTransportHostMessageListener() {
  if (transportHostMessageListenerInstalled || typeof window === "undefined" || isTauriRuntimeAvailable()) return;
  transportHostMessageListenerInstalled = true;
  window.addEventListener("message", (event: MessageEvent<TransportHostMessage>) => {
    if (event.data?.type === "pai-ide-context-snapshot") {
      void forwardTransportHostSnapshot(event.data.snapshot);
      return;
    }
    if (event.data?.type !== "pai-discovery" || !event.data.discovery) return;
    if (!configureWebBridge(event.data.discovery)) return;
    const context = getTransportHostContext();
    for (const handler of transportDiscoveryHandlers) handler(context);
    if (webTransportConversationContextId) {
      void ensureWebTransportConversationContext(webTransportConversationContextId, true);
    }
  });
}

export function getTransportLaunchParameter(name: string): string {
  if (typeof window === "undefined") return "";
  const normalizedName = String(name || "").trim();
  if (!normalizedName) return "";
  return String(new URLSearchParams(window.location.search || "").get(normalizedName) || "").trim();
}

/** 宿主打开本机文件的差异只存在于适配器内；返回是否已由宿主接管。 */
export async function openTransportLocalFileReference(href: string): Promise<boolean> {
  const normalizedHref = String(href || "").trim();
  if (!normalizedHref || isTauriRuntimeAvailable()) return false;
  return postTransportHostMessage({ type: "pai-open-file", href: normalizedHref });
}

/** 外链打开统一入口：桌面走系统命令，嵌入式 Web 交给宿主，普通 Web 用浏览器。 */
export async function openTransportExternalUrl(url: string): Promise<boolean> {
  const normalizedUrl = String(url || "").trim();
  if (!/^https?:\/\//i.test(normalizedUrl)) return false;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("open_external_url", { url: normalizedUrl });
    return true;
  }
  if (postTransportHostMessage({ type: "pai-open-url", url: normalizedUrl })) return true;
  return !!window.open(normalizedUrl, "_blank", "noopener,noreferrer");
}

/**
 * 窗口跳转属于宿主能力，不应把 Tauri command 名称散落到业务组件。
 * Web 没有同等的原生窗口控制；调用方可据返回值决定是否显示/提示，
 * 但不需要再自行探测运行时。
 */
export type TransportWindowTarget = "main" | "chat" | "archives" | "runtimeLogs";

const TRANSPORT_WINDOW_COMMANDS: Record<TransportWindowTarget, string> = {
  main: "show_main_window",
  chat: "show_chat_window",
  archives: "show_archives_window",
  runtimeLogs: "open_runtime_logs_window",
};

export async function openTransportWindow(target: TransportWindowTarget): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  const command = TRANSPORT_WINDOW_COMMANDS[target];
  if (!command) return false;
  await invokeTauri(command);
  return true;
}

/** 统一托盘同步入口；Web 端没有托盘，安全地跳过。 */
export async function syncTransportTrayIcon(input: Record<string, unknown> = {}): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  await invokeTauri("sync_tray_icon", { input });
  return true;
}

function nativeTransportCapabilityError(capability: string): Error {
  return new Error(`WEB_NATIVE_CAPABILITY_UNAVAILABLE: 当前宿主不支持${capability}`);
}

function invokeRequiredNativeTransport<T>(
  capability: string,
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntimeAvailable()) return Promise.reject(nativeTransportCapabilityError(capability));
  return invokeTauri<T>(command, args);
}

/** 本机附件路径入库只能由统一适配器落到原生命令。 */
export function ingestTransportLocalAttachment<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机附件路径读取", "attachment_ingest_local_path", { input });
}

/** 本机二进制文件读取只服务于已有本机文件入口。 */
export function readTransportLocalBinaryFile<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机文件读取", "read_local_binary_file", { input });
}

function exportTransportMemoriesToPath<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机文件写出", "export_memories_to_path", { input });
}

type TransportMemoryExportPayload = {
  records?: unknown[];
  memories?: unknown[];
};

export type TransportMemoryExportResult = {
  path: string;
  count: number;
};

/** 记忆导出只暴露一个业务入口；保存对话框、浏览器下载与命令选择都留在适配器内。 */
export async function exportTransportMemories(input: {
  defaultFileName: string;
  scopes?: string[];
}): Promise<TransportMemoryExportResult | null> {
  const defaultFileName = String(input.defaultFileName || "memory_backup.json").trim() || "memory_backup.json";
  const scopes = (input.scopes || []).map((scope) => String(scope || "").trim()).filter(Boolean);
  if (isTauriRuntimeAvailable()) {
    const path = await saveTransportFileDialog({
      defaultPath: defaultFileName,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return null;
    return exportTransportMemoriesToPath<TransportMemoryExportResult>({
      path,
      ...(scopes.length > 0 ? { scopes } : {}),
    });
  }
  const payload = await invokeTauri<TransportMemoryExportPayload>(
    "export_memories",
    scopes.length > 0 ? { input: { scopes } } : undefined,
  );
  downloadBrowserTransportJsonFile(defaultFileName, payload);
  return {
    path: defaultFileName,
    count: Array.isArray(payload.records) ? payload.records.length : (payload.memories?.length || 0),
  };
}

export function exportTransportAgentPrivateMemories<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机文件写出", "export_agent_private_memories", { input });
}

export function exportTransportArchive<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("归档文件导出", "archives.export", { input });
}

export function importTransportConversationShare<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("会话分享文件导入", "conversation.importShare", { input });
}

export async function writeTransportUtf8TextFile(path: string, text: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("write_utf8_text_file_to_path", {
    input: { path: normalizedPath, text: String(text || "") },
  });
  return true;
}

export async function writeTransportBase64File(path: string, bytesBase64: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("write_base64_file_to_path", {
    input: { path: normalizedPath, bytesBase64: String(bytesBase64 || "") },
  });
  return true;
}

export type TransportChatImageData = {
  dataUrl: string;
  mime?: string;
  width?: number;
  height?: number;
  originalWidth?: number;
  originalHeight?: number;
};

/**
 * 读取聊天图片的唯一入口。存储媒体引用在两端都可读取；本机路径只
 * 在桌面文件能力存在时读取，Web 端返回 null 而不是把 native command
 * 错误泄漏到共享聊天状态机。
 */
export async function readTransportChatImage(input: {
  path?: string;
  mediaRef?: string;
  mime?: string;
  original?: boolean;
  maxEdge?: number;
}): Promise<TransportChatImageData | null> {
  const mediaRef = String(input.mediaRef || "").trim();
  if (mediaRef) {
    const mime = String(input.mime || "").trim();
    if (!mime) return null;
    return invokeTauri<{ dataUrl: string }>("read_chat_image_data_url", {
      input: { mediaRef, mime },
    });
  }
  const path = String(input.path || "").trim();
  if (!path) return null;
  return invokeTauri<TransportChatImageData>(
    input.original ? "read_local_chat_image_original" : "read_local_chat_image_thumbnail",
    {
      input: {
        path,
        ...(Number.isFinite(input.maxEdge) ? { maxEdge: Number(input.maxEdge) } : {}),
      },
    },
  );
}

/** 本机图片操作也只保留在适配器内；Web 端由能力显隐，不抛协议错误。 */
export async function copyTransportChatImageToClipboard(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("copy_local_chat_image_to_clipboard", { input: { path: normalizedPath } });
  return true;
}

export async function saveTransportChatImageAs(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("save_local_chat_image_as", { input: { path: normalizedPath } });
  return true;
}

export async function openTransportLocalDirectory(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath) return false;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("open_local_file_directory", { path: normalizedPath });
    return true;
  }
  return postTransportHostMessage({ type: "pai-open-file", href: normalizedPath });
}

export async function openTransportFileWithDefaultProgram(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("open_file_with_default_program", { path: normalizedPath });
  return true;
}

export async function openTransportFileReaderWindow(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("open_file_reader_window_command", { path: normalizedPath });
  return true;
}

export async function updateTransportFileReaderWatchTargets(input: Record<string, unknown>): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  await invokeTauri("update_file_reader_watch_targets", { input });
  return true;
}

/** 在 VS Code 中打开文件；本机文件系统能力只存在于桌面宿主。 */
export async function openTransportFileInVscode(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("open_file_in_vscode", {
    input: { path: normalizedPath },
  });
  return true;
}

/** 在系统文件管理器中定位文件（选中该文件）；本机文件系统能力只存在于桌面宿主。 */
export async function revealTransportLocalFile(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("open_local_file_directory", { path: normalizedPath });
  return true;
}

type TransportFileRawPayload = {
  name?: string;
  bytesBase64?: string;
};

function transportFileNameFromPath(path: string): string {
  const normalized = String(path || "").replace(/\\/g, "/");
  const name = normalized.slice(normalized.lastIndexOf("/") + 1);
  return name || "download";
}

function base64ToBytes(value: string): Uint8Array<ArrayBuffer> {
  const binary = atob(value);
  const bytes = new Uint8Array(new ArrayBuffer(binary.length));
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function downloadBrowserFile(bytes: Uint8Array<ArrayBuffer>, fileName: string) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

/**
 * 另存为：桌面宿主走系统保存对话框；Web/VS Code 通过桥接取原始字节后触发浏览器下载。
 * 两端都以真实文件字节为准，不做文本转写。
 */
export async function saveTransportLocalFileAs(path: string): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  if (!normalizedPath) return false;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("save_local_file_as", { path: normalizedPath });
    return true;
  }
  const payload = await invokeTauri<TransportFileRawPayload>("fileReader.readRawFile", {
    path: normalizedPath,
  });
  const encoded = String(payload?.bytesBase64 || "");
  if (!encoded) return false;
  downloadBrowserFile(
    base64ToBytes(encoded),
    String(payload?.name || "") || transportFileNameFromPath(normalizedPath),
  );
  return true;
}

export function listTransportFileReaderDirectoryOpenTargets<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机目录打开方式", "list_file_reader_directory_open_targets");
}

/** 系统字体枚举仅桌面宿主可查，Web/VS Code 无此能力。 */
export function listTransportSystemFonts<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("系统字体枚举", "list_system_fonts");
}

/** genai 内置 chat 适配器清单仅桌面宿主可查，Web/VS Code 无此能力。 */
export function listTransportGenaiChatAdapters<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("genai 内置聊天适配器清单", "list_genai_chat_adapters");
}

/** 打开（或创建）会话草稿：Web/VS Code 经 WS 桥由后端 dispatcher 提供。 */
export function openTransportConversationDraft<T>(input: Record<string, unknown>): Promise<T> {
  return invokeTauri<T>("conversation.openDraft", { input });
}

/** 改写会话草稿字段：Web/VS Code 经 WS 桥由后端 dispatcher 提供。 */
export function updateTransportConversationDraft<T>(input: Record<string, unknown>): Promise<T> {
  return invokeTauri<T>("conversation.updateDraft", { input });
}

export async function openTransportFileReaderDirectoryTarget(
  path: string,
  targetKind: string,
): Promise<boolean> {
  const normalizedPath = String(path || "").trim();
  const normalizedTargetKind = String(targetKind || "").trim();
  if (!normalizedPath || !normalizedTargetKind || !isTauriRuntimeAvailable()) return false;
  await invokeTauri("open_file_reader_directory_target", {
    input: { path: normalizedPath, targetKind: normalizedTargetKind },
  });
  return true;
}

export function migrateTransportShellWorkspaceDirectory(input: {
  oldPath: string;
  newPath: string;
  taskId: string;
}): Promise<string> {
  return invokeRequiredNativeTransport<string>("本机会话目录迁移", "migrate_shell_workspace_directory", { input });
}

export function openTransportMcpWorkspaceDirectory(): Promise<string> {
  return invokeRequiredNativeTransport<string>("MCP 工作区目录", "mcp_open_workspace_dir");
}

export function openTransportSkillWorkspaceDirectory(): Promise<string> {
  return invokeRequiredNativeTransport<string>("Skill 工作区目录", "skill_open_workspace_dir");
}

export function openTransportSkillDirectory(path: string): Promise<string> {
  return invokeRequiredNativeTransport<string>("Skill 目录", "skill_open_item_dir", { path });
}

export function saveTransportSkill(
  path: string,
  content: string,
  name?: string,
  description?: string
): Promise<SkillSummaryItem> {
  return invokeTauri<SkillSummaryItem>("mcp_save_skill", { path, content, name, description });
}

export function readTransportSkillFile(path: string): Promise<string> {
  return invokeTauri<string>("mcp_read_skill_file", { path });
}

/** 设置单个 Skill 的全局启用状态。 */
export function setTransportSkillEnabled(name: string, enabled: boolean): Promise<SkillListResult> {
  return invokeTauri<SkillListResult>("skill_set_enabled", { name, enabled });
}

/** 删除单个 Skill（按 SKILL.md 绝对路径定位）。 */
export function removeTransportSkill(path: string): Promise<SkillListResult> {
  return invokeTauri<SkillListResult>("skill_remove", { path });
}

/** 列出能力商店的可用来源。 */
export function listTransportCatalogSources(kind: string): Promise<CatalogSourceInfo[]> {
  return invokeTauri<CatalogSourceInfo[]>("catalog_list_sources", { kind });
}

/** 按来源检索能力商店条目。 */
export function searchTransportCatalog(input: {
  source: string;
  query: string;
  page: number;
  pageSize: number;
}): Promise<CatalogPage> {
  return invokeTauri<CatalogPage>("catalog_search", { input });
}

/** 安装能力商店条目到本地工作区。 */
export function installTransportCatalogEntry(input: {
  source: string;
  entryId: string;
  envValues?: Record<string, string>;
}): Promise<CatalogInstallResult> {
  return invokeTauri<CatalogInstallResult>("catalog_install", { input });
}

export function openTransportStorageUsageItemDirectory(itemId: string): Promise<void> {
  return invokeRequiredNativeTransport<void>("存储项目录", "open_storage_usage_item_directory", {
    input: { itemId: String(itemId || "").trim() },
  });
}

export function resetTransportChatShellWorkspace(workspacePath: string): Promise<string> {
  return invokeRequiredNativeTransport<string>("本机会话工作区重置", "reset_chat_shell_workspace", {
    input: { workspacePath: String(workspacePath || "").trim() },
  });
}

export function getTransportDefaultChatShellWorkspacePath(): Promise<string> {
  return invokeRequiredNativeTransport<string>("本机会话工作区路径", "get_default_chat_shell_workspace_path");
}

export function getTransportHostRuntimePrerequisites<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("宿主运行环境检测", "get_host_runtime_prerequisites");
}

export function installTransportHostRuntimePrerequisite<T>(kind: string): Promise<T> {
  return invokeRequiredNativeTransport<T>("宿主运行环境安装", "install_host_runtime_prerequisite", {
    kind: String(kind || "").trim(),
  });
}

export function updateTransportRecordHotkey<T>(recordHotkey: string): Promise<T> {
  return invokeRequiredNativeTransport<T>("录音快捷键注册", "update_record_hotkey", {
    input: { recordHotkey: String(recordHotkey || "").trim() },
  });
}

export function updateTransportRecordBackgroundWake<T>(enabled: boolean): Promise<T> {
  return invokeRequiredNativeTransport<T>("录音后台唤醒", "update_record_background_wake", {
    input: { enabled: !!enabled },
  });
}

export function captureTransportFocusedWindow<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("当前窗口截图", "xcap", {
    input: { method: "capture_focused_window", args: {} },
  });
}

export function captureTransportDesktop<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("桌面截图", "desktop_screenshot", {
    input: { mode: "desktop" },
  });
}

export function sendTransportNativeNotificationDemo<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("本机通知演示", "demo_send_native_notification");
}

export function restartTransportApplicationDemo(): Promise<void> {
  return invokeRequiredNativeTransport<void>("应用重启演示", "demo_restart_app");
}

export function getTransportGithubUpdateState<T>(): Promise<T> {
  return invokeRequiredNativeTransport<T>("应用更新状态", "get_github_update_state");
}

export function checkTransportGithubUpdate<T>(input: Record<string, unknown>): Promise<T> {
  return invokeRequiredNativeTransport<T>("应用更新检查", "check_github_update", input);
}

export function startTransportGithubUpdate(input: Record<string, unknown>): Promise<void> {
  return invokeRequiredNativeTransport<void>("应用更新下载", "start_github_update", input);
}

export function cancelTransportGithubUpdate(): Promise<void> {
  return invokeRequiredNativeTransport<void>("应用更新取消", "cancel_github_update");
}

export function applyPreparedTransportGithubUpdate(): Promise<void> {
  return invokeRequiredNativeTransport<void>("应用更新安装", "apply_prepared_github_update");
}

export type PortablePendingManualReplace = {
  plan: {
    target_dir: string;
    target_exe_name: string;
    staging_dir: string;
    backup_root: string;
    temp_root: string;
    zip_path: string;
    log_path: string;
    parent_pid?: number;
    target_version?: string;
  };
  backup_dir: string;
  failure_reason: string;
  created_at: string;
};

export function getPortablePendingManualReplace(): Promise<PortablePendingManualReplace | null> {
  return invokeTauri<PortablePendingManualReplace | null>("get_portable_pending_manual_replace");
}
export function dismissPortablePendingManualReplace(): Promise<void> {
  return invokeTauri<void>("dismiss_portable_pending_manual_replace");
}
export function retryPortablePendingManualReplace(): Promise<void> {
  return invokeTauri<void>("retry_portable_pending_manual_replace");
}
export function openPortablePendingDir(kind: string): Promise<void> {
  return invokeTauri<void>("open_portable_pending_dir", { kind });
}

type TransportConfigMigrationExportPayload = {
  path?: unknown;
  fileName?: unknown;
  bytesBase64?: unknown;
};

export type TransportConfigMigrationExportResult = {
  path: string;
  fileName: string;
};

export type TransportConfigMigrationPackageSelection = {
  packagePath?: string;
  packageFileName?: string;
  packageBytesBase64?: string;
};

/** 迁移包导出包含最终文件交付，业务层不再判断路径返回还是 base64 返回。 */
export async function exportTransportConfigMigrationPackage(
  input: Record<string, unknown>,
): Promise<TransportConfigMigrationExportResult> {
  const result = await invokeTauri<TransportConfigMigrationExportPayload>("configMigration.export", { input });
  const fileName = String(result?.fileName || "p-ai-migration.zip").trim() || "p-ai-migration.zip";
  const bytesBase64 = String(result?.bytesBase64 || "").trim();
  if (bytesBase64) {
    downloadBrowserTransportBase64File(fileName, bytesBase64, "application/zip");
  }
  return {
    path: String(result?.path || "").trim() || fileName,
    fileName,
  };
}

/** 迁移包选择统一返回后端预检输入；本机路径与浏览器字节不泄漏到业务组件。 */
export async function pickTransportConfigMigrationPackage(): Promise<TransportConfigMigrationPackageSelection | null> {
  const options: TransportFileDialogOptions = {
    multiple: false,
    filters: [{ name: "P-AI Migration", extensions: ["zip"] }],
  };
  if (isTauriRuntimeAvailable()) {
    const selected = await openTransportFileDialog(options);
    const packagePath = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
    return packagePath ? { packagePath } : null;
  }
  const file = (await pickBrowserTransportFiles(options))[0];
  if (!file) return null;
  return {
    packageFileName: String(file.name || "p-ai-migration.zip").trim() || "p-ai-migration.zip",
    packageBytesBase64: await readBrowserTransportFileAsBase64(file),
  };
}

export function previewTransportConfigMigrationPackage<T>(input: Record<string, unknown>): Promise<T> {
  return invokeTauri<T>("configMigration.preview", { input });
}

export function applyTransportConfigMigrationPackage<T>(previewId: string): Promise<T> {
  return invokeTauri<T>("configMigration.apply", {
    input: { previewId: String(previewId || "").trim() },
  });
}

async function forwardTransportHostSnapshot(snapshot: Record<string, unknown> | undefined) {
  if (!snapshot || isTauriRuntimeAvailable()) return;
  try {
    await invokeTauri("ideContext.upsert", snapshot);
  } catch (error) {
    console.warn("[IDE 上下文] 宿主快照转发失败", {
      error: error instanceof Error ? error.message : String(error || "unknown"),
      stack: error instanceof Error ? error.stack : undefined,
    });
  }
}

/** 宿主发现与上下文消息都由适配器接收，业务入口不直接监听 postMessage。 */
export function onTransportDiscovery(
  handler: (context: TransportHostContext) => void,
): () => void {
  if (typeof window === "undefined" || isTauriRuntimeAvailable()) return () => {};
  ensureTransportHostMessageListener();
  transportDiscoveryHandlers.add(handler);
  if (ensureWebBridgeConfig()) handler(getTransportHostContext());
  return () => transportDiscoveryHandlers.delete(handler);
}

export function requestTransportDiscoveryRefresh(): boolean {
  if (isTauriRuntimeAvailable()) return false;
  ensureTransportHostMessageListener();
  return requestWebBridgeDiscoveryRefresh();
}

/**
 * 将宿主提供的工作区根注入当前会话。
 *
 * 这是唯一允许感知 Web/宿主工作区差异的入口；聊天前台本身只负责请求
 * 统一的 conversation.foregroundLightSnapshot。桌面端已经由窗口工作区
 * 逻辑承接，因此这里是无操作。
 */
export async function prepareTransportConversationContext(conversationId: string): Promise<void> {
  const normalizedConversationId = String(conversationId || "").trim();
  if (!normalizedConversationId || isTauriRuntimeAvailable()) return;
  await ensureWebTransportConversationContext(normalizedConversationId, true);
}

async function ensureWebTransportConversationContext(conversationId: string, force = false): Promise<void> {
  const normalizedConversationId = String(conversationId || "").trim();
  if (!normalizedConversationId || isTauriRuntimeAvailable()) return;
  webTransportConversationContextId = normalizedConversationId;
  const hostRoot = getTransportHostContext().workspaceRoots[0];
  if (!hostRoot?.path) {
    requestWebBridgeDiscoveryRefresh();
    return;
  }
  const contextKey = `${normalizedConversationId}\u0000${hostRoot.path}\u0000${hostRoot.name}`;
  if (!force && contextKey === webTransportConversationContextKey) return;
  await invokeWebBridge("workspace.ensureHostRoot", {
    conversationId: normalizedConversationId,
    workspacePath: hostRoot.path,
    workspaceName: hostRoot.name || undefined,
  });
  webTransportConversationContextKey = contextKey;
}

/** 设置入口由适配器决定打开原生窗口、宿主页或普通浏览器页面。 */
export async function openTransportSettings(): Promise<boolean> {
  if (isTauriRuntimeAvailable()) {
    return openTransportWindow("main");
  }
  if (typeof window === "undefined") return false;
  // VS Code 侧边栏：直接通知后端打开本机设置窗口，不经过扩展宿主打开外部 URL。
  if (getVsCodeHostApi()) {
    try {
      await invokeWebBridge("show_main_window");
      return true;
    } catch (error) {
      console.warn("[设置] 通知后端打开设置窗口失败:", error);
      return false;
    }
  }
  const path = window.location.pathname.endsWith(".html") ? "settings.html" : "/settings";
  const url = new URL(path, window.location.href);
  const config = ensureWebBridgeConfig();
  if (config?.chatUrl) url.searchParams.set("chatUrl", config.chatUrl);
  return openTransportExternalUrl(url.toString());
}

/** 本机资源 URL 的平台差异只允许存在于传输适配器内。 */
export function resolveLocalFileUrl(path: string): string {
  const normalized = String(path || "").trim();
  if (!normalized || !isTauriRuntimeAvailable()) return "";
  return convertFileSrc(normalized);
}

export async function getCurrentTransportWindowInnerSize(): Promise<{ width: number; height: number }> {
  if (!isTauriRuntimeAvailable()) {
    return {
      width: typeof window === "undefined" ? 0 : window.innerWidth,
      height: typeof window === "undefined" ? 0 : window.innerHeight,
    };
  }
  const size = await getCurrentWindow().innerSize();
  return { width: size.width, height: size.height };
}

function getCurrentTransportWindowLabel(): string {
  if (!isTauriRuntimeAvailable()) {
    if (typeof window === "undefined") return "config";
    const globals = window as WebBridgeGlobals;
    const pathname = String(window.location?.pathname || "").toLowerCase();
    if (globals.__PAI_SIDEBAR_BRIDGE__ || pathname.endsWith("/sidebar.html") || pathname.endsWith("/chat.html")) {
      return "chat";
    }
    if (pathname.endsWith("/archives.html")) return "archives";
    return "config";
  }
  try {
    return String(getCurrentWindow().label || "").trim() || "config";
  } catch {
    return "config";
  }
}

function isPrimaryTransportChatView(): boolean {
  return isTauriRuntimeAvailable() && getCurrentTransportWindowLabel() === "chat";
}

export type TransportWindowRole = "chat" | "archives" | "config";

export function getCurrentTransportWindowRole(): TransportWindowRole {
  const label = getCurrentTransportWindowLabel();
  if (label === "chat") return "chat";
  if (label === "archives") return "archives";
  return "config";
}

export async function setTransportChatViewActive(active: boolean): Promise<void> {
  if (!isPrimaryTransportChatView()) return;
  await invokeTauri("set_chat_window_active", { active: !!active });
}

/** 窗口外扩属于宿主窗口能力；聊天布局只消费统一布尔结果。 */
export async function setTransportChatPaneExpanded(
  side: "left" | "right",
  expanded: boolean,
  widthPhysical: number,
): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  return invokeTauri<boolean>("set_chat_window_side_expanded", {
    side,
    expanded: !!expanded,
    widthPhysical: Math.max(1, Math.round(Number(widthPhysical) || 1)),
  });
}

export async function currentTransportWindowIsAlwaysOnTop(): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  return getCurrentWindow().isAlwaysOnTop();
}

export async function currentTransportWindowIsMaximized(): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  return getCurrentWindow().isMaximized();
}

export async function hideCurrentTransportWindow(): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  try {
    await invokeTauri("hide_current_window");
  } catch {
    await getCurrentWindow().hide();
  }
}

export async function minimizeCurrentTransportWindow(): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  await getCurrentWindow().minimize();
}

export async function startCurrentTransportWindowDragging(): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  try {
    await invokeTauri("start_current_window_drag");
  } catch {
    await getCurrentWindow().startDragging();
  }
}

export async function setCurrentTransportWindowAlwaysOnTop(value: boolean): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  await getCurrentWindow().setAlwaysOnTop(value);
}

export async function toggleCurrentTransportWindowMaximize(): Promise<boolean> {
  if (!isTauriRuntimeAvailable()) return false;
  try {
    return await invokeTauri<boolean>("toggle_current_window_maximize");
  } catch {
    const currentWindow = getCurrentWindow();
    await currentWindow.toggleMaximize();
    return currentWindow.isMaximized();
  }
}

export type TransportDragDropPayload = {
  type: "enter" | "over" | "drop" | "leave";
  paths: string[];
};

export async function listenCurrentTransportFileDrop(
  handler: (payload: TransportDragDropPayload) => void,
): Promise<() => void> {
  if (!isTauriRuntimeAvailable()) return () => {};
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload as unknown as TransportDragDropPayload;
    handler({
      type: payload.type,
      paths: Array.isArray(payload.paths) ? payload.paths : [],
    });
  });
}

/**
 * 创建统一流式通道。只有通信适配器知道这里是否需要真正的 Tauri
 * Channel，其他模块不应 import @tauri-apps/api/core。
 */
export function createTransportChannel<T>(): TransportChannel<T> {
  if (isTauriRuntimeAvailable()) {
    const channel = new Channel<T>();
    transportChannelMeta.set(channel as unknown as object, { native: channel as unknown as object });
    return channel as unknown as TransportChannel<T>;
  }
  let disposed = false;
  const channel: TransportChannel<T> = {
    onmessage: null,
    dispose: () => {
      disposed = true;
      channel.onmessage = null;
    },
  };
  // Keep the marker even for the Web virtual channel so invokeTauri can
  // remove it from JSON-RPC params instead of serializing a UI callback.
  transportChannelMeta.set(channel as unknown as object, { native: null });
  // A disposed virtual channel must never be observed by accidental callers.
  Object.defineProperty(channel, "__transportDisposed", {
    configurable: false,
    enumerable: false,
    get: () => disposed,
  });
  return channel;
}

function isTransportChannel(value: unknown): value is object {
  return !!value && typeof value === "object" && transportChannelMeta.has(value as object);
}

function prepareInvokeValue(value: unknown, webRuntime: boolean, seen: WeakSet<object>): unknown {
  if (value === null || value === undefined) return value;
  if (typeof value !== "object") return value;
  const objectValue = value as object;
  const objectAny = value as any;
  if (isTransportChannel(objectValue)) {
    const native = transportChannelMeta.get(objectValue)?.native;
    // WebSocket JSON-RPC cannot carry a Tauri Channel. Omitting the field is
    // intentional: Web streaming is delivered through bridge notifications.
    return webRuntime ? undefined : native || value;
  }
  if (
    Object.prototype.toString.call(objectValue) === "[object ArrayBuffer]"
    || ArrayBuffer.isView(objectAny)
    || (typeof Blob !== "undefined" && objectAny instanceof (Blob as any))
    || (typeof File !== "undefined" && objectAny instanceof (File as any))
  ) {
    return value;
  }
  if (seen.has(objectValue)) return value;
  seen.add(objectValue);
  if (Array.isArray(value)) {
    return (value as unknown[]).map((item) => prepareInvokeValue(item, webRuntime, seen));
  }
  const record = value as Record<string, unknown>;
  const next: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(record)) {
    const prepared = prepareInvokeValue(item, webRuntime, seen);
    if (prepared !== undefined) next[key] = prepared;
  }
  return next;
}

const WEB_BRIDGE_NATIVE_ONLY_COMMANDS = new Set([
  "list_file_reader_directory",
  "list_file_reader_directory_open_targets",
  "read_file_reader_file",
  "read_file_reader_file_block",
  "read_plan_file_content",
  "open_file_reader_directory_target",
  "open_file_reader_directory_shell",
  "open_file_with_default_program",
  "open_local_file_directory",
  "open_file_in_vscode",
  "save_local_file_as",
  "open_workspace_file",
  "open_storage_usage_item_directory",
  "open_chat_shell_workspace_dir",
  "mcp_open_workspace_dir",
  "skill_open_workspace_dir",
  "skill_open_item_dir",
  "copy_local_chat_image_to_clipboard",
  "save_local_chat_image_as",
  "export_archive_to_file",
  "archives.export",
  "conversation.importShare",
  "export_memories_to_path",
  "export_agent_private_memories",
  "write_base64_file_to_path",
  "write_utf8_text_file_to_path",
  "queue_local_file_attachment",
  "attachment_transfer_begin",
  "attachment_transfer_chunk",
  "attachment_transfer_complete",
  "attachment_transfer_abort",
  "attachment_ingest_local_path",
  "update_file_reader_watch_targets",
  "migrate_shell_workspace_directory",
  "desktop_screenshot",
  "demo_send_native_notification",
  "demo_restart_app",
  "xcap",
  "start_current_window_drag",
  "toggle_current_window_maximize",
  "hide_current_window",
  "update_record_hotkey",
  "update_record_background_wake",
  "install_host_runtime_prerequisite",
  "get_host_runtime_prerequisites",
  "reset_chat_shell_workspace",
  "get_default_chat_shell_workspace_path",
  "open_external_url",
  "show_main_window",
  "show_chat_window",
  "show_archives_window",
  "show_quick_setup_window",
  "complete_quick_setup_and_open_chat",
  "list_system_fonts",
  "list_genai_chat_adapters",
  "open_runtime_logs_window",
  "sync_tray_icon",
  "get_github_update_state",
  "check_github_update",
  "start_github_update",
  "cancel_github_update",
  "apply_prepared_github_update",
  "get_portable_pending_manual_replace",
  "dismiss_portable_pending_manual_replace",
  "retry_portable_pending_manual_replace",
  "open_portable_pending_dir",
  "bind_active_chat_view_stream",
  "probe_active_chat_view_stream",
  "unbind_active_chat_view_stream",
  "clear_window_chat_view_stream_bindings_command",
  "set_chat_window_active",
  "open_file_reader_window_command",
  "read_local_binary_file",
  "set_chat_window_side_expanded",
]);

// 业务层使用同一组可读的协议方法名；桌面端的 Tauri command 名称由适配器
// 转换，避免组件/composable 再维护一套 Web/APP 分支。
const TAURI_COMMAND_ALIASES: Record<string, string> = {
  "chat.send": "submit_chat_message",
  "chat.stop": "stop_chat_message",
  "chat.queueSnapshot": "get_chat_queue_snapshot",
  "chat.sessionStateSnapshot": "get_main_session_state_snapshot",
  "chat.queueRecall": "recall_chat_queue_event",
  "chat.queueMarkGuided": "mark_chat_queue_event_guided",
  "toolReview.reports.list": "list_tool_review_reports",
  "toolReview.report.delete": "delete_tool_review_report",
  "toolReview.commitOptions.list": "list_tool_review_commit_options",
  "toolReview.code.submit": "submit_tool_review_code",
  "toolReview.batches.list": "list_tool_review_batches",
  "toolReview.item.detail": "get_tool_review_item_detail",
  "toolReview.batch.details": "get_tool_review_batch_details",
  "toolReview.item.review": "run_tool_review_for_call",
  "toolReview.batch.review": "run_tool_review_for_batch",
  "toolReview.item.decision": "set_tool_review_item_user_decision",
  "task.list": "task_list_tasks",
  "task.get": "task_get_task",
  "task.create": "task_create_task",
  "task.update": "task_update_task",
  "task.complete": "task_complete_task",
  "task.delete": "task_delete_task",
  "task.runLogs": "task_list_run_logs",
  "task.optimizeDraft": "task_optimize_draft",
  "conversation.batchArchive": "batch_archive_conversations",
  "conversation.archive": "archive_conversation",
  "conversation.compact": "compact_conversation",
  "conversation.changedSince": "list_unarchived_conversations_changed_since",
  "conversation.blockPage": "get_unarchived_conversation_block_page",
  "conversation.runtimeSnapshot": "get_conversation_runtime_snapshot",
  "conversation.freshnessSnapshot": "get_foreground_conversation_freshness_snapshot",
  "conversation.markRead": "mark_conversation_read",
  "conversation.messageById": "get_unarchived_conversation_message_by_id",
  "conversation.messagesBefore": "get_active_conversation_messages_before",
  "conversation.compactionSegmentBefore": "get_active_conversation_compaction_segment_before",
  "conversation.messagesAfterAsync": "request_conversation_messages_after_async",
  "conversation.setActive": "set_active_unarchived_conversation",
  "conversation.delete": "delete_unarchived_conversation",
  "conversation.rebindRecipient": "rebind_unarchived_conversation_recipient",
  "conversation.branchFromSelection": "branch_unarchived_conversation_from_selection",
  "conversation.branchFromMessage": "create_conversation_branch_from_message",
  "conversation.branchFromCurrent": "branch_unarchived_conversation_from_current",
  "conversation.forwardSelection": "forward_unarchived_conversation_selection",
  "conversation.forwardRemoteContact": "forward_selection_to_remote_im_contact",
  "conversation.rename": "rename_unarchived_conversation",
  "conversation.pin": "toggle_unarchived_conversation_pin",
  "conversation.autoPush": "set_conversation_auto_push_remote_contact",
  "conversation.preferredModel.set": "set_conversation_preferred_model",
  "conversation.overview.list": "list_unarchived_conversations",
  "conversation.list": "list_transport_conversations",
  "conversation.createOptions": "list_conversation_create_options",
  "conversation.create": "create_unarchived_conversation",
  "conversation.openDraft": "open_draft_conversation",
  "conversation.updateDraft": "update_draft_conversation",
  "conversation.clearChatError": "clear_chat_error",
  "conversation.createSide": "create_side_chat_conversation",
  "conversation.importShare": "import_conversation_share_from_file",
  "remoteIm.conversations.list": "remote_im_list_contact_conversations",
  "remoteIm.conversation.blockPage": "remote_im_get_contact_conversation_block_page",
  "remoteIm.conversation.clear": "remote_im_clear_contact_conversation",
  "delegate.conversations.list": "list_delegate_conversations",
  "prompt.preview": "get_prompt_preview",
  "prompt.systemPreview": "get_system_prompt_preview",
  "goal.current": "goal_get_current",
  "goal.create": "goal_create_goal",
  "goal.cancel": "goal_cancel_goal",
  "conversation.foregroundLightSnapshot": "get_foreground_conversation_light_snapshot",
  "conversation.fastRequestTurns": "get_conversation_fast_request_turns",
  "conversation.planMode.set": "set_conversation_plan_mode",
  "conversation.plan.confirm": "confirm_plan_and_continue",
  "terminalApproval.resolve": "resolve_terminal_approval",
  "terminalApproval.approveForSession": "approve_terminal_approval_for_session",
  "terminalApproval.approveForWorkspace": "approve_terminal_approval_for_workspace",
  "terminalApproval.list": "list_pending_terminal_approvals",
  "conversation.plan.readFile": "read_plan_file_content",
  "conversation.rewindPreview": "preview_rewind_conversation_from_message",
  "conversation.rewind": "rewind_conversation_from_message",
  "fileReader.directory.list": "list_file_reader_directory",
  "fileReader.readFile": "read_file_reader_file",
  "fileReader.readFileBlock": "read_file_reader_file_block",
  "delegate.statuses": "list_conversation_delegate_statuses",
  "backgroundShell.list": "list_conversation_background_shell_tasks",
  "backgroundShell.terminate": "terminate_conversation_background_shell_task",
  "delegate.abort": "abort_delegate_conversation",
  "delegate.blockPage": "get_delegate_conversation_block_page",
  "delegate.submit": "submit_user_async_delegate",
  "delegate.delete": "delete_delegate_conversation",
  "conversation.deleteDelegate": "delete_delegate_conversation",
  "conversation.deleteArchive": "delete_archive",
  "conversation.unarchive": "unarchive_archive",
  "conversation.archiveList": "list_archives",
  "conversation.archiveBlockPage": "get_archive_block_page",
  "conversation.exportShare": "export_conversation_share_json",
  "conversation.importArchives": "import_archives_from_json",
  "conversation.sectionOrders.get": "get_conversation_section_orders",
  "conversation.sectionOrders.save": "save_conversation_section_order",
  "app.bootstrapSnapshot": "load_app_bootstrap_snapshot",
  "app.language.set": "set_ui_language",
  "messageStore.migration.check": "check_message_store_migration",
  "messageStore.migration.run": "run_message_store_migration",
  "messageStore.migration.status": "get_message_store_migration_runtime_status",
  "messageStore.migration.confirm": "confirm_message_store_migration_summary",
  "configMigration.export": "export_config_migration_package",
  "configMigration.preview": "preview_import_config_migration_package",
  "configMigration.apply": "apply_import_config_migration_package",
  "remoteIm.services.start": "frontend_ready_start_remote_im_services",
  "transport.accessInfo": "get_web_access_info",
  "archives.export": "export_archive_to_file",
  "archives.list": "list_archives",
  "archives.blockPage": "get_archive_block_page",
  "archives.delete": "delete_archive",
  "archives.unarchive": "unarchive_archive",
  "ideContext.query": "query_ide_context_references",
  "remoteIm.dashboard.subscribe": "remote_im_subscribe_contact_dashboard",
  "remoteIm.dashboard.unsubscribe": "remote_im_unsubscribe_contact_dashboard",
  "remoteIm.dashboard.sync": "remote_im_sync_contact_dashboard",
  "workspace.directory.list": "list_file_reader_directory",
  "workspace.gitRootCheck": "check_git_workspace_root",
  "workspace.permission": "get_conversation_workspace_permission",
  "workspace.permission.select": "select_conversation_workspace_permission",
  "workspace.layout.save": "save_conversation_workspace_layout",
  "workspace.list": "list_conversation_workspaces",
  "workspace.branch.record": "record_conversation_workspace_branch",
};

const TRANSPORT_COMMAND_CANONICAL_NAMES: Record<string, string> = Object.entries(TAURI_COMMAND_ALIASES)
  .reduce<Record<string, string>>((result, [portable, tauriCommand]) => {
    result[tauriCommand] = portable;
    return result;
  }, {});

const TAURI_INPUT_WRAPPED_COMMANDS = new Set([
  "chat.send",
  "chat.stop",
  "toolReview.reports.list",
  "toolReview.report.delete",
  "toolReview.commitOptions.list",
  "toolReview.code.submit",
  "toolReview.batches.list",
  "toolReview.item.detail",
  "toolReview.batch.details",
  "toolReview.item.review",
  "toolReview.batch.review",
  "toolReview.item.decision",
  "task.get",
  "task.create",
  "task.update",
  "task.complete",
  "task.delete",
  "task.runLogs",
  "task.optimizeDraft",
  "conversation.batchArchive",
  "conversation.archive",
  "conversation.compact",
  "conversation.changedSince",
  "conversation.create",
  "conversation.openDraft",
  "conversation.updateDraft",
  "conversation.clearChatError",
  "conversation.createSide",
  "conversation.importShare",
  "conversation.blockPage",
  "conversation.freshnessSnapshot",
  "conversation.markRead",
  "conversation.messageById",
  "conversation.messagesBefore",
  "conversation.compactionSegmentBefore",
  "conversation.messagesAfterAsync",
  "conversation.setActive",
  "conversation.delete",
  "conversation.rebindRecipient",
  "conversation.branchFromSelection",
  "conversation.branchFromMessage",
  "conversation.branchFromCurrent",
  "conversation.forwardSelection",
  "conversation.forwardRemoteContact",
  "conversation.rename",
  "conversation.pin",
  "conversation.autoPush",
  "conversation.preferredModel.set",
  "remoteIm.conversation.blockPage",
  "remoteIm.conversation.clear",
  "prompt.preview",
  "prompt.systemPreview",
  "conversation.exportShare",
  "conversation.importArchives",
  "conversation.sectionOrders.save",
  "archives.export",
  "goal.create",
  "goal.cancel",
  "configMigration.export",
  "configMigration.preview",
  "configMigration.apply",
  "conversation.foregroundLightSnapshot",
  "conversation.fastRequestTurns",
  "conversation.planMode.set",
  "conversation.plan.confirm",
  "terminalApproval.resolve",
  "terminalApproval.approveForSession",
  "terminalApproval.approveForWorkspace",
  "conversation.rewindPreview",
  "conversation.rewind",
  "delegate.statuses",
  "backgroundShell.list",
  "backgroundShell.terminate",
  "delegate.abort",
  "delegate.blockPage",
  "delegate.submit",
  "delegate.delete",
  "conversation.deleteDelegate",
  "conversation.archiveBlockPage",
  "archives.blockPage",
  "ideContext.query",
  "remoteIm.dashboard.subscribe",
  "remoteIm.dashboard.unsubscribe",
  "remoteIm.dashboard.sync",
  "workspace.gitRootCheck",
  "workspace.permission",
  "workspace.permission.select",
  "workspace.layout.save",
  "workspace.list",
  "workspace.branch.record",
]);

function prepareRuntimeArgs(
  canonicalCommand: string,
  args: Record<string, unknown> | undefined,
  webRuntime: boolean,
): Record<string, unknown> | undefined {
  if (!args || !TAURI_INPUT_WRAPPED_COMMANDS.has(canonicalCommand)) return args;
  if (webRuntime) {
    const input = args.input;
    return input && typeof input === "object" && !Array.isArray(input)
      ? input as Record<string, unknown>
      : args;
  }
  if (Object.prototype.hasOwnProperty.call(args, "input")) return args;
  return { input: args };
}

function wireCommandForRuntime(command: string, webRuntime: boolean): string {
  const canonical = TRANSPORT_COMMAND_CANONICAL_NAMES[command] || command;
  return webRuntime ? canonical : TAURI_COMMAND_ALIASES[canonical] || canonical;
}

function transportHostShellWorkspaces(): Array<Record<string, unknown>> {
  return getTransportHostContext().workspaceRoots.map((workspace) => ({
    id: `host-workspace-${workspace.path}`,
    name: workspace.name || workspace.path,
    path: workspace.path,
    level: "main",
    access: "approval",
  }));
}

/** Web 宿主（VS Code 侧边栏）注入的工作区条目；桌面 Tauri 运行时或宿主未注入工作区时为空数组。 */
export function transportDraftHostWorkspaces(): Array<{
  id: string;
  name: string;
  path: string;
  level: "main";
  access: "approval";
}> {
  if (isTauriRuntimeAvailable()) return [];
  return transportHostWorkspaceShellEntries();
}

function transportHostWorkspaceShellEntries(): Array<{
  id: string;
  name: string;
  path: string;
  level: "main";
  access: "approval";
}> {
  return transportHostShellWorkspaces().map((item) => ({
    id: String(item.id),
    name: String(item.name),
    path: String(item.path),
    level: "main" as const,
    access: "approval" as const,
  }));
}

function mergeTransportHostWorkspaces(existing: unknown): Array<Record<string, unknown>> {
  const merged = new Map<string, Record<string, unknown>>();
  for (const item of Array.isArray(existing) ? existing : []) {
    if (!item || typeof item !== "object" || Array.isArray(item)) continue;
    const record = item as Record<string, unknown>;
    const path = String(record.path || "").trim();
    if (path) merged.set(path.replace(/\\/g, "/").toLowerCase(), record);
  }
  for (const item of transportHostShellWorkspaces()) {
    const path = String(item.path || "").trim();
    if (!path) continue;
    const key = path.replace(/\\/g, "/").toLowerCase();
    if (!merged.has(key)) merged.set(key, item);
  }
  return Array.from(merged.values());
}

function prepareWebHostContextArgs(
  canonicalCommand: string,
  args: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (!args || canonicalCommand !== "conversation.create") return args;
  const current = Array.isArray(args.shellWorkspaces) ? args.shellWorkspaces : [];
  if (current.length > 0) return args;
  const shellWorkspaces = transportHostShellWorkspaces();
  return shellWorkspaces.length > 0 ? { ...args, shellWorkspaces } : args;
}

function normalizeTransportResult(canonicalCommand: string, result: unknown, webRuntime: boolean): unknown {
  if (canonicalCommand === "conversation.plan.readFile") {
    if (result && typeof result === "object") {
      return String((result as Record<string, unknown>).content || "");
    }
    return String(result || "");
  }
  if (canonicalCommand === "workspace.directory.list" && result && typeof result === "object") {
    const record = result as Record<string, unknown>;
    const directories = Array.isArray(record.directories)
      ? record.directories
      : (Array.isArray(record.entries)
          ? record.entries.filter((item) => !!item && typeof item === "object" && !!(item as { isDirectory?: unknown }).isDirectory)
          : []);
    return { ...record, directories };
  }
  if (canonicalCommand === "app.bootstrapSnapshot" && webRuntime && result && typeof result === "object") {
    const snapshot = result as Record<string, unknown>;
    const config = snapshot.config;
    if (config && typeof config === "object" && !Array.isArray(config)) {
      const configRecord = config as Record<string, unknown>;
      return {
        ...snapshot,
        config: {
          ...configRecord,
          shellWorkspaces: mergeTransportHostWorkspaces(configRecord.shellWorkspaces),
        },
      };
    }
  }
  return result;
}

export function invokeTauri<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs?: number,
): Promise<T> {
  const webRuntime = !isTauriRuntimeAvailable();
  const canonicalCommand = TRANSPORT_COMMAND_CANONICAL_NAMES[command] || command;
  const wireCommand = wireCommandForRuntime(command, webRuntime);
  if (webRuntime) {
    if (WEB_BRIDGE_NATIVE_ONLY_COMMANDS.has(canonicalCommand) || WEB_BRIDGE_NATIVE_ONLY_COMMANDS.has(wireCommand)) {
      return Promise.reject(new Error(`WEB_NATIVE_CAPABILITY_UNAVAILABLE: Web 端不支持本机能力：${canonicalCommand}`));
    }
    const runtimeArgs = prepareWebHostContextArgs(
      canonicalCommand,
      prepareRuntimeArgs(canonicalCommand, args, true),
    );
    const prepared = runtimeArgs === undefined
      ? undefined
      : prepareInvokeValue(runtimeArgs, true, new WeakSet<object>()) as Record<string, unknown>;
    const conversationId = canonicalCommand === "conversation.foregroundLightSnapshot"
      ? String(prepared?.conversationId || "").trim()
      : "";
    const prepareContext = conversationId
      ? ensureWebTransportConversationContext(conversationId)
      : Promise.resolve();
    return prepareContext.then(() => invokeWebBridge<T>(wireCommand, prepared, timeoutMs))
      .then((result) => normalizeTransportResult(canonicalCommand, result, true) as T);
  }
  const runtimeArgs = prepareRuntimeArgs(canonicalCommand, args, false);
  const prepared = runtimeArgs === undefined
    ? undefined
    : prepareInvokeValue(runtimeArgs, false, new WeakSet<object>()) as Record<string, unknown>;
  return invoke<T>(wireCommand, prepared)
    .then((result) => normalizeTransportResult(canonicalCommand, result, false) as T);
}

/** 二进制 IPC 也必须经过同一通信适配器。 */
function invokeNativeTransportBinary<T>(
  command: string,
  payload: Uint8Array,
  options?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntimeAvailable()) {
    return Promise.reject(new Error(`WEB_NATIVE_CAPABILITY_UNAVAILABLE: Web 端不支持本机能力：${command}`));
  }
  return invoke<T>(command, payload, options as any);
}

async function listenTauriEvent<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (!isTauriRuntimeAvailable()) return () => {};
  return listen<T>(event, handler);
}

export async function emitTransportEvent<T>(method: string, payload?: T): Promise<void> {
  const canonicalMethod = canonicalTransportNotificationMethod(method);
  if (!isTauriRuntimeAvailable()) {
    emitLocalTransportNotification(canonicalMethod, payload);
    return;
  }
  const eventNames = transportNotificationEventNames(canonicalMethod);
  await emit(eventNames[0] || canonicalMethod, payload);
}

/** 目标窗口事件也必须经过统一传输适配器。Web 端没有本机窗口目标，安全地忽略。 */
export async function emitTransportEventTo<T>(target: string, method: string, payload?: T): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  const normalizedTarget = String(target || "").trim();
  const canonicalMethod = canonicalTransportNotificationMethod(method);
  if (!normalizedTarget || !canonicalMethod) return;
  const eventNames = transportNotificationEventNames(canonicalMethod);
  await emitTo(normalizedTarget, eventNames[0] || canonicalMethod, payload);
}

export type TransportResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

export async function startCurrentTransportWindowResizeDragging(
  direction: TransportResizeDirection,
): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  await getCurrentWindow().startResizeDragging(direction);
}

const TRANSPORT_NOTIFICATION_EVENT_ALIASES: Record<string, string | string[]> = {
  "chat.queueSnapshotUpdated": "easy-call:chat-queue-snapshot",
  "chat.historyFlushed": "easy-call:history-flushed",
  "chat.roundStarted": "easy-call:round-started",
  "chat.roundFinished": ["easy-call:round-completed", "easy-call:round-failed"],
  "chat.roundFailed": "easy-call:round-failed",
  "chat.assistantDelta": "easy-call:assistant-delta",
  "chat.streamRebindRequired": "easy-call:stream-rebind-required",
  "conversation.messageAppended": "easy-call:conversation-message-appended",
  "chat.rewindCompleted": "easy-call:chat-rewind-completed",
  "conversation.overviewUpdated": "easy-call:conversation-overview-updated",
  "conversation.overviewItemUpdated": "easy-call:conversation-overview-item-updated",
  "conversation.todosUpdated": "easy-call:conversation-todos-updated",
  "conversation.pinUpdated": "easy-call:conversation-pin-updated",
  "conversation.goalUpdated": "easy-call:conversation-goal-updated",
  "conversation.runtimeStateUpdated": "easy-call:conversation-runtime-state-updated",
  "conversation.messagesAfterSynced": "easy-call:conversation-messages-after-synced",
  "conversation.chatErrorCleared": "easy-call:chat-error-cleared",
  "monitor.changed": "easy-call:monitor-changed",
  "ideContext.updated": "ide-context-updated",
  "remoteIm.dashboard.updated": "easy-call:remote-im-contact-dashboard-updated",
  "conversation.workStatus": "conversation_work_status",
  "theme.changed": "easy-call:theme-changed",
  "locale.changed": "easy-call:locale-changed",
  "terminalApproval.requested": "easy-call:terminal-approval-request",
  "conversation.apiUpdated": "easy-call:conversation-api-updated",
  "chat.settingsUpdated": "easy-call:chat-settings-updated",
  "config.updated": "easy-call:config-updated",
  "agentWork.started": "easy-call:agent-work-start",
  "agentWork.stopped": "easy-call:agent-work-stop",
  "recordHotkey.probe": "easy-call:record-hotkey-probe",
  "toolReview.reportsUpdated": "easy-call:tool-review-reports-updated",
  "fileReader.openPath": "file-reader-open-path",
  "fileReader.addToChat": "easy-call:file-reader-add-to-chat",
  "codeReview.requested": "code-review-requested",
  "uiSize.changed": "easy-call:ui-size-changed",
  "markdownAppearance.changed": "easy-call:markdown-appearance-changed",
  "chatMessageAppearance.changed": "easy-call:chat-message-appearance-changed",
  "chatComposerAppearance.changed": "easy-call:chat-composer-appearance-changed",
  "fileReaderAppearance.changed": "easy-call:file-reader-appearance-changed",
  "workspace.migrationProgress": "easy-call:workspace-migration-progress",
  "fileReader.watchChanged": "easy-call:file-reader-watch-changed",
  "gitPanel.watchChanged": "easy-call:git-panel-changed",
};

const localTransportNotificationHandlers = new Map<string, Set<(payload: unknown) => void>>();

function canonicalTransportNotificationMethod(method: string): string {
  const normalized = String(method || "").trim();
  if (!normalized || Object.prototype.hasOwnProperty.call(TRANSPORT_NOTIFICATION_EVENT_ALIASES, normalized)) {
    return normalized;
  }
  for (const [canonical, mapped] of Object.entries(TRANSPORT_NOTIFICATION_EVENT_ALIASES)) {
    const eventNames = Array.isArray(mapped) ? mapped : [mapped];
    if (eventNames.includes(normalized)) return canonical;
  }
  return normalized;
}

function transportNotificationEventNames(method: string): string[] {
  const canonicalMethod = canonicalTransportNotificationMethod(method);
  const mapped = TRANSPORT_NOTIFICATION_EVENT_ALIASES[canonicalMethod] || canonicalMethod;
  return Array.isArray(mapped) ? mapped : [mapped];
}

function emitLocalTransportNotification<T>(method: string, payload: T): void {
  const handlers = localTransportNotificationHandlers.get(canonicalTransportNotificationMethod(method));
  if (!handlers) return;
  for (const handler of handlers) handler(payload);
}

function onLocalTransportNotification<T>(method: string, handler: (payload: T) => void): () => void {
  const canonicalMethod = canonicalTransportNotificationMethod(method);
  const handlers = localTransportNotificationHandlers.get(canonicalMethod) || new Set<(payload: unknown) => void>();
  const wrapped = (payload: unknown) => handler(payload as T);
  handlers.add(wrapped);
  localTransportNotificationHandlers.set(canonicalMethod, handlers);
  return () => {
    handlers.delete(wrapped);
    if (handlers.size === 0) localTransportNotificationHandlers.delete(canonicalMethod);
  };
}

/** 统一事件订阅；Tauri event 与 Web bridge notification 的名字映射只在此处维护。 */
export function appendTransportProbeLog(
  prefix: string,
  tag: string,
  data?: Record<string, unknown>,
  level: "debug" | "info" | "warn" | "error" = "info",
): void {
  let detail = "";
  if (data) {
    try {
      detail = ` ${JSON.stringify(data)}`;
    } catch {
      detail = " [unserializable]";
    }
  }
  // 探针只用于排障，任何环境（含 invoke 未接线的测试/纯浏览器）都不能反过来影响链路
  try {
    void invokeTauri<boolean>("append_runtime_log_probe", {
      message: `${prefix} ${tag}${detail}`,
      level,
    }).catch(() => {});
  } catch {
    // 忽略：诊断探针失败不得影响主链路
  }
}

function probeTransportLog(tag: string, data?: Record<string, unknown>): void {
  appendTransportProbeLog("[聊天流诊断]", tag, data);
}

// 只对链路关键事件打诊断日志：这几个事件是定位流式链路断点的锚点，
// 其余高频事件（状态推送、队列快照等）逐条记录会淹没日志。
export const PROBE_NOTIFICATION_METHODS = new Set([
  "chat.roundStarted",
  "chat.roundFinished",
  "chat.roundFailed",
  "chat.streamRebindRequired",
  "chat.historyFlushed",
]);

/** 统一事件订阅；Tauri event 与 Web bridge notification 的名字映射只在此处维护。 */
export function onTransportNotification<T = unknown>(
  method: string,
  handler: (payload: T) => void,
): () => void {
  const canonicalMethod = canonicalTransportNotificationMethod(method);
  if (!isTauriRuntimeAvailable()) {
    probeTransportLog("监听降级", { method: canonicalMethod, reason: "not_tauri" });
    const stopLocal = onLocalTransportNotification(canonicalMethod, handler);
    const stopBridge = onWebBridgeNotification(canonicalMethod, (payload) => handler(payload as T));
    return () => {
      stopLocal();
      stopBridge();
    };
  }
  let active = true;
  const unlisteners = new Set<() => void>();
  const eventNames = transportNotificationEventNames(canonicalMethod);
  for (const eventName of eventNames) {
    void listenTauriEvent<T>(eventName, (event) => {
      if (PROBE_NOTIFICATION_METHODS.has(canonicalMethod)) {
        probeTransportLog("事件回调", { method: canonicalMethod, event: eventName });
      }
      handler(event.payload);
    }).then((stop) => {
      if (!active) {
        stop();
        return;
      }
      unlisteners.add(stop);
    }).catch((error: unknown) => {
      probeTransportLog("监听失败", {
        method: canonicalMethod,
        event: eventName,
        error: String((error as { message?: unknown })?.message || error || ""),
      });
    });
  }
  return () => {
    active = false;
    for (const stop of unlisteners) stop();
    unlisteners.clear();
  };
}

function nextWebTransportStreamBindingVersion(bindingId: string): number {
  const next = (webTransportStreamBindingVersions.get(bindingId) || 0) + 1;
  webTransportStreamBindingVersions.set(bindingId, next);
  return next;
}

function disposeWebTransportStreamBinding(binding: WebTransportStreamBinding | undefined): void {
  if (!binding) return;
  binding.stopDelta();
  binding.stopProbe();
  binding.channel.dispose?.();
}

function createWebTransportStreamBinding(
  conversationId: string,
  channel: TransportChannel<unknown>,
): WebTransportStreamBinding {
  const stopDelta = onWebBridgeNotification("chat.assistantDelta", (payload) => {
    const record = payload && typeof payload === "object"
      ? payload as { conversationId?: unknown; event?: unknown }
      : null;
    const payloadConversationId = String(record?.conversationId || "").trim();
    if (payloadConversationId !== conversationId) return;
    const event = record?.event;
    const kind = event && typeof event === "object"
      ? String((event as { kind?: unknown }).kind || "").trim()
      : "";
    // 与桌面 Channel 保持同一语义：低频广播事件只走统一通知订阅，
    // 不能同时再灌入流通道，否则 Web 会把同一状态处理两次。
    if (kind === "tool_status" || kind === "context_usage_update") return;
    channel.onmessage?.(event);
  });
  const stopProbe = onWebBridgeNotification("chat.streamProbeAck", (payload) => {
    const record = payload && typeof payload === "object"
      ? payload as { conversationId?: unknown; probeId?: unknown }
      : null;
    if (String(record?.conversationId || "").trim() !== conversationId) return;
    const probeId = String(record?.probeId || "").trim();
    if (!probeId) return;
    channel.onmessage?.({ kind: "stream_probe", message: probeId });
  });
  return { conversationId, channel, stopDelta, stopProbe };
}

/**
 * 统一前台流式绑定。桌面端注册 Tauri Channel；网络端把同一份后端通知
 * 适配成 TransportChannel，聊天状态机不再维护第二套订阅实现。
 *
 * 当前传输下聊天流是否需要前端显式发起绑定：
 * 桌面端 sendChat 的原生 Tauri Channel 已覆盖流式，再 bind 会双通道双写；
 * Web 端 Channel 无法穿过 JSON-RPC，必须显式 bind 才能收到正文 delta。
 */
export function chatStreamNeedsFrontendBind(): boolean {
  return !isTauriRuntimeAvailable();
}

export async function bindTransportConversationStream<T>(input: {
  bindingId: string;
  conversationId?: string;
  onDelta: TransportChannel<T>;
}): Promise<void> {
  const bindingId = String(input.bindingId || "").trim();
  const conversationId = String(input.conversationId || "").trim();
  if (!bindingId || !conversationId) return;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("bind_active_chat_view_stream", {
      input: { bindingId, conversationId },
      onDelta: input.onDelta,
    });
    return;
  }

  const version = nextWebTransportStreamBindingVersion(bindingId);
  const previous = webTransportStreamBindings.get(bindingId);
  const channel = input.onDelta as unknown as TransportChannel<unknown>;
  const next = createWebTransportStreamBinding(conversationId, channel);
  try {
    await invokeTauri("conversation.resumeSubscription", { conversationId });
  } catch (error) {
    // 新订阅失败时旧订阅仍保持工作，避免桌面/网页切会话过程中出现“旧流也被
    // 销毁、新流又没建立”的空窗。若期间已有更新操作，则不触碰更新后的绑定。
    if (webTransportStreamBindingVersions.get(bindingId) === version) {
      disposeWebTransportStreamBinding(next);
      if (previous && webTransportStreamBindings.get(bindingId) !== previous) {
        webTransportStreamBindings.set(bindingId, previous);
      }
    } else {
      disposeWebTransportStreamBinding(next);
    }
    throw error;
  }
  if (webTransportStreamBindingVersions.get(bindingId) !== version) {
    // 绑定完成前已经发生了更新/解绑；该结果已过期，不能覆盖最新状态。
    disposeWebTransportStreamBinding(next);
    return;
  }
  webTransportStreamBindings.set(bindingId, next);
  if (previous && previous !== next) disposeWebTransportStreamBinding(previous);
}

export async function unbindTransportConversationStream(input: { bindingId: string }): Promise<void> {
  const bindingId = String(input.bindingId || "").trim();
  if (!bindingId) return;
  if (isTauriRuntimeAvailable()) {
    await invokeTauri("unbind_active_chat_view_stream", { input: { bindingId } });
    return;
  }
  nextWebTransportStreamBindingVersion(bindingId);
  const binding = webTransportStreamBindings.get(bindingId);
  if (!binding) return;
  disposeWebTransportStreamBinding(binding);
  webTransportStreamBindings.delete(bindingId);
}

/**
 * 清空当前窗口的全部活动聊天流绑定。
 *
 * 前端页面重载（HMR / 手动刷新 / 崩溃重建）后，旧 bindingId 的 channel 在 JS 侧
 * 已失效，但 Rust 侧注册仍残留；Tauri 的 Channel::send 在 callback 不存在时仍返回
 * Ok，僵尸注册无法通过 send 失败自动清理，流式期间会反复投递失效 channel 并刷
 * `Couldn't find callback id` 警告。窗口启动/重载后调用本函数先清残留，再重新绑定。
 */
export async function clearWindowChatViewStreamBindings(): Promise<void> {
  if (!isTauriRuntimeAvailable()) return;
  try {
    await invokeTauri("clear_window_chat_view_stream_bindings_command", {});
  } catch (error) {
    console.warn("[聊天] 清理本窗口残留流式绑定失败", {
      message: String((error as { message?: string })?.message ?? error ?? ""),
    });
  }
}

export async function probeTransportConversationStream(input: {
  bindingId: string;
  conversationId?: string;
  probeId: string;
}): Promise<boolean> {
  const bindingId = String(input.bindingId || "").trim();
  const conversationId = String(input.conversationId || "").trim();
  const probeId = String(input.probeId || "").trim();
  if (!bindingId || !conversationId || !probeId) return false;
  if (isTauriRuntimeAvailable()) {
    return invokeTauri<boolean>("probe_active_chat_view_stream", {
      input: { bindingId, conversationId, probeId },
    });
  }
  const binding = webTransportStreamBindings.get(bindingId);
  if (!binding || binding.conversationId !== conversationId) return false;
  const result = await invokeTauri<{ delivered?: boolean }>(
    "conversation.streamProbe",
    { conversationId, probeId },
    1500,
  );
  return !!result?.delivered;
}

function onWebBridgeNotification(method: string, handler: (payload: unknown) => void): () => void {
  const normalized = method.trim();
  if (!normalized) return () => {};
  const handlers = webBridgeNotificationHandlers.get(normalized) || new Set<(payload: unknown) => void>();
  handlers.add(handler);
  webBridgeNotificationHandlers.set(normalized, handlers);
  return () => {
    handlers.delete(handler);
    if (handlers.size === 0) {
      webBridgeNotificationHandlers.delete(normalized);
    }
  };
}

function onWebBridgeStateChange(handler: (state: WebBridgeState) => void): () => void {
  webBridgeStateHandlers.add(handler);
  handler({ ...webBridgeState });
  return () => webBridgeStateHandlers.delete(handler);
}

function getWebBridgeState(): WebBridgeState {
  ensureWebBridgeConfig();
  return { ...webBridgeState };
}

function getWebBridgeConfig(): WebBridgeConfig | null {
  return ensureWebBridgeConfig();
}

function configureWebBridge(config: WebBridgeConfig | null | undefined): WebBridgeConfig | null {
  const normalized = normalizeWebBridgeConfig(config || null);
  if (!normalized) return null;
  webBridgeConfig = normalized;
  webBridgeState.configured = true;
  notifyWebBridgeStateChanged();
  return normalized;
}

function disconnectWebBridge() {
  try {
    webBridgeSocket?.close();
  } catch {
    resetWebBridgeConnectionState("连接已断开");
  }
  resetWebBridgeConnectionState("连接已断开");
}

async function connectWebBridge(): Promise<WebBridgeState> {
  const config = ensureWebBridgeConfig();
  if (!config) {
    webBridgeState.configured = false;
    webBridgeState.errorText = "缺少 PAI Web 桥接配置。";
    throw new Error(webBridgeState.errorText);
  }
  if (webBridgeSocket?.readyState === WebSocket.OPEN && webBridgeState.bridgeReady) {
    return getWebBridgeState();
  }
  await openWebBridgeSocket(config);
  return getWebBridgeState();
}

async function loginWebBridge(password: string): Promise<WebBridgeState> {
  const normalizedPassword = String(password || "").trim();
  await invokeWebBridge("auth.login", {
    ...(normalizedPassword ? { password: normalizedPassword } : {}),
  }, 10000);
  return getWebBridgeState();
}

async function requestWebBridgePassword(): Promise<string> {
  // 远程前端模式：iframe 嵌入时优先向父窗口（手机 PAI 壳层）请求已保存密码，
  // 避免 Android WebView 对跨域 iframe 的 window.prompt 静默拦截。
  if (typeof window !== "undefined" && window.self !== window.top) {
    const fromParent = await requestRemotePasswordFromParent();
    if (fromParent) return fromParent;
  }
  if (typeof window === "undefined" || typeof window.prompt !== "function") {
    throw new Error("远程访问需要密码，但当前宿主无法显示认证输入框");
  }
  const password = String(window.prompt("请输入 PAI 远程访问密码") || "").trim();
  if (!password) throw new Error("远程访问认证已取消");
  return password;
}

/**
 * 订阅远程前端壳层的会话命令（toggle-conversation-list / create-conversation）。
 * 只接受约定壳层 origin 与来源标识的消息，防恶意父页面伪造会话操作。
 * 返回取消订阅函数；桌面独立窗口（self === top）不注册监听。
 */
export function onTransportRemoteChatCommand(handler: (method: string) => void): () => void {
  if (typeof window === "undefined" || window.self === window.top) return () => {};
  const listener = (event: MessageEvent) => {
    if (event.origin !== REMOTE_BRIDGE_ALLOWED_ORIGIN) return;
    const data = event.data as { source?: unknown; method?: unknown } | null;
    if (!data || typeof data !== "object") return;
    if (data.source !== REMOTE_COMMAND_BRIDGE_SOURCE) return;
    const method = String(data.method || "").trim();
    if (method) handler(method);
  };
  window.addEventListener("message", listener);
  return () => window.removeEventListener("message", listener);
}

/** 向父窗口（手机 PAI 壳层）请求远程访问密码；父窗口未回复或超时返回空串。
 *  HostWindow 由调用方注入，便于在测试中提供可控的父窗口与消息监听。 */
export async function requestRemotePasswordFromParent(
  hostWindow?: Window,
): Promise<string> {
  const win =
    hostWindow ?? (typeof window !== "undefined" ? window : undefined);
  if (!win || typeof win.parent === "undefined") {
    return "";
  }
  return new Promise((resolve) => {
    let settled = false;
    const settle = (password: string) => {
      if (settled) return;
      settled = true;
      win.removeEventListener("message", listener);
      win.clearTimeout(timer);
      resolve(password);
    };
    const listener = (event: MessageEvent) => {
      // 只接受约定壳层 origin 与父窗口来源的消息，防恶意页面伪造密码注入。
      if (event.origin !== REMOTE_BRIDGE_ALLOWED_ORIGIN) return;
      if (event.source !== win.parent) return;
      const data = event.data as { source?: unknown; method?: unknown; payload?: unknown } | null;
      if (!data || typeof data !== "object") return;
      if (data.source !== REMOTE_AUTH_BRIDGE_SOURCE) return;
      if (data.method !== "password") return;
      const password = String((data.payload as { password?: unknown } | null)?.password || "").trim();
      settle(password);
    };
    const timer = win.setTimeout(() => settle(""), 1500);
    win.addEventListener("message", listener);
    try {
      win.parent.postMessage(
        { source: REMOTE_AUTH_BRIDGE_SOURCE, method: "request-password" },
        REMOTE_BRIDGE_ALLOWED_ORIGIN,
      );
    } catch {
      settle("");
    }
  });
}

async function ensureWebBridgeAuthenticated(): Promise<void> {
  if (!webBridgeState.authRequired || webBridgeState.authenticated) return;
  if (!webBridgeAuthenticationPromise) {
    webBridgeAuthenticationPromise = (async () => {
      const password = await requestWebBridgePassword();
      await loginWebBridge(password);
      if (!webBridgeState.authenticated) throw new Error("远程访问认证失败");
    })().finally(() => {
      webBridgeAuthenticationPromise = null;
    });
  }
  await webBridgeAuthenticationPromise;
}

function normalizeWebBridgeConfig(config: WebBridgeConfig | null): WebBridgeConfig | null {
  const chatUrl = String(config?.chatUrl || "").trim();
  const fallbackUrl = String(config?.url || "").trim();
  const resolvedChatUrl = chatUrl || fallbackUrl.replace(/\/ide-context$/, "/chat");
  if (!resolvedChatUrl) return null;
  const persistedToken = config?.token ? "" : readPersistedWebBridgeToken(resolvedChatUrl);
  const token = String(config?.token || persistedToken || "").trim();
  return {
    ...config,
    chatUrl: resolvedChatUrl,
    token: token || undefined,
  };
}

function bridgeUrlFromCurrentLocation(): string {
  if (typeof window === "undefined" || !window.location?.host) return "";
  if (!/^https?:$/i.test(window.location.protocol)) return "";
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/chat`;
}

function ensureWebBridgeConfig(): WebBridgeConfig | null {
  if (typeof window === "undefined") return null;
  ensureTransportHostMessageListener();
  if (webBridgeConfig) return webBridgeConfig;
  const globals = window as WebBridgeGlobals;
  const injected = normalizeWebBridgeConfig(globals.__PAI_SETTINGS_BRIDGE__ || globals.__PAI_SIDEBAR_BRIDGE__ || null);
  if (injected) return configureWebBridge(injected);
  const params = new URLSearchParams(window.location.search || "");
  const fromQuery = normalizeWebBridgeConfig({
    chatUrl: params.get("chatUrl") || bridgeUrlFromCurrentLocation(),
    token: params.get("token") || undefined,
  });
  if (fromQuery) return configureWebBridge(fromQuery);
  requestWebBridgeDiscoveryRefresh();
  return null;
}

function requestWebBridgeDiscoveryRefresh(): boolean {
  if (typeof window === "undefined") return false;
  const now = Date.now();
  if (now - transportDiscoveryRefreshRequestedAt < 1000) return false;
  const delivered = postTransportHostMessage({ type: "pai-refresh-discovery" });
  if (delivered) transportDiscoveryRefreshRequestedAt = now;
  return delivered;
}

function resetWebBridgeConnectionState(errorText = "") {
  webBridgeSocket = null;
  webBridgeConnectPromise = null;
  webBridgeState.connected = false;
  webBridgeState.connecting = false;
  webBridgeState.bridgeReady = false;
  webBridgeState.authRequired = false;
  webBridgeState.authenticated = true;
  webBridgeState.errorText = errorText;
  webBridgeAuthenticationPromise = null;
  for (const [id, request] of webBridgePending.entries()) {
    if (request.timer !== null) window.clearTimeout(request.timer);
    request.reject(new Error(errorText || "连接已断开"));
    webBridgePending.delete(id);
  }
  for (const [transferId, request] of webBridgePendingAttachmentChunks.entries()) {
    window.clearTimeout(request.timer);
    request.reject(new Error(errorText || "连接已断开"));
    webBridgePendingAttachmentChunks.delete(transferId);
  }
  notifyWebBridgeStateChanged();
}

function settleWebBridgeRequest(id: number, payload: Record<string, unknown>) {
  const request = webBridgePending.get(id);
  if (!request) return;
  webBridgePending.delete(id);
  if (request.timer !== null) window.clearTimeout(request.timer);
  if (payload.error) {
    const error = payload.error as { message?: string };
    const message = String(error?.message || "请求失败");
    const rejected = new Error(message) as Error & { code?: string; type?: string };
    if (isWebBridgeAuthenticationRefreshError(message)) {
      const currentChatUrl = String(webBridgeConfig?.chatUrl || "").trim();
      if (currentChatUrl) {
        clearPersistedWebBridgeToken(currentChatUrl);
      }
      if (webBridgeConfig) {
        webBridgeConfig = { ...webBridgeConfig, token: undefined };
      }
      webBridgeState.authRequired = true;
      webBridgeState.authenticated = false;
      notifyWebBridgeStateChanged();
    }
    const codeMatch = message.match(/^([A-Z][A-Z0-9_]+):\s*/);
    if (codeMatch?.[1]) {
      rejected.code = codeMatch[1];
      rejected.type = codeMatch[1];
    }
    request.reject(rejected);
    return;
  }
  if (payload.result && typeof payload.result === "object" && (payload.result as { authenticated?: unknown }).authenticated === true) {
    const authToken = String((payload.result as { authToken?: unknown }).authToken || "").trim();
    const currentChatUrl = String(webBridgeConfig?.chatUrl || "").trim();
    if (authToken && currentChatUrl) {
      persistWebBridgeToken(currentChatUrl, authToken);
      if (webBridgeConfig) {
        webBridgeConfig = { ...webBridgeConfig, token: authToken };
      }
    }
    webBridgeState.authenticated = true;
    webBridgeState.authRequired = false;
    notifyWebBridgeStateChanged();
  }
  request.resolve(payload.result);
}

function isWebBridgeAuthenticationRefreshError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error || "");
  return message.includes("token expired")
    || message.includes("discovery refreshed")
    || message.includes("invalid authToken");
}

function emitWebBridgeNotification(method: string, payload: unknown) {
  // 远程前端场景：sidebar 页面被外部宿主以 iframe 嵌入时（window.self !== window.top），
  // 把 bridge 通知转发给父窗口（如手机端 PAI 壳层），供宿主消费同一事件流；
  // 桌面独立窗口 self === top 不触发，既有行为不变。
  if (typeof window !== "undefined" && window.self !== window.top) {
    try {
      window.parent.postMessage(
        { source: "pai-remote-bridge", method, payload },
        REMOTE_BRIDGE_ALLOWED_ORIGIN,
      );
    } catch {
      // 转发失败不影响本地事件分发
    }
  }
  const handlers = webBridgeNotificationHandlers.get(method);
  if (!handlers) return;
  for (const handler of handlers) handler(payload);
}

function settleWebAttachmentChunk(payload: Record<string, unknown>) {
  const params = (payload.params || {}) as { transferId?: unknown; nextOffset?: unknown };
  const transferId = String(params.transferId || "").trim();
  if (payload.method === "attachment.chunkAck" && transferId) {
    const request = webBridgePendingAttachmentChunks.get(transferId);
    if (!request) return;
    webBridgePendingAttachmentChunks.delete(transferId);
    window.clearTimeout(request.timer);
    const nextOffset = Number(params.nextOffset);
    if (!Number.isSafeInteger(nextOffset) || nextOffset < 0) {
      request.reject(new Error("附件分块确认 offset 无效"));
      return;
    }
    request.resolve({ transferId, nextOffset });
    return;
  }
  if (payload.error && webBridgePendingAttachmentChunks.size > 0) {
    const error = payload.error as { message?: unknown };
    const reason = new Error(String(error?.message || "附件分块传输失败"));
    for (const [pendingTransferId, request] of webBridgePendingAttachmentChunks.entries()) {
      webBridgePendingAttachmentChunks.delete(pendingTransferId);
      window.clearTimeout(request.timer);
      request.reject(reason);
    }
  }
}

function handleWebBridgeMessage(event: MessageEvent<string>, ready: () => void) {
  let payload: Record<string, unknown>;
  try {
    payload = JSON.parse(String(event.data || "{}"));
  } catch {
    return;
  }
  if (typeof payload.id === "number") {
    settleWebBridgeRequest(payload.id, payload);
    return;
  }
  const method = String(payload.method || "");
  if (method === "attachment.chunkAck") {
    settleWebAttachmentChunk(payload);
    return;
  }
  if (payload.error) {
    settleWebAttachmentChunk(payload);
  }
  if (method === "bridge.ready") {
    const params = (payload.params || {}) as { authRequired?: unknown };
    const hasAuthToken = !!String(webBridgeConfig?.token || "").trim();
    webBridgeState.bridgeReady = true;
    webBridgeState.authRequired = !!params.authRequired;
    webBridgeState.authenticated = !webBridgeState.authRequired || hasAuthToken;
    notifyWebBridgeStateChanged();
    ready();
    return;
  }
  if (method === "bridge.shutdown") {
    emitWebBridgeNotification(method, payload.params);
    webBridgeState.errorText = "网络访问已关闭";
    notifyWebBridgeStateChanged();
    try {
      webBridgeSocket?.close();
    } catch {
      resetWebBridgeConnectionState("网络访问已关闭");
    }
    return;
  }
  if (method) emitWebBridgeNotification(method, payload.params);
}

function openWebBridgeSocket(config: WebBridgeConfig): Promise<void> {
  if (webBridgeConnectPromise) return webBridgeConnectPromise;
  if (webBridgeSocket && webBridgeSocket.readyState !== WebSocket.CLOSED) {
    try {
      webBridgeSocket.close();
    } catch {
      // ignore close errors before reconnecting
    }
  }
  webBridgeState.configured = true;
  webBridgeState.connecting = true;
  webBridgeState.connected = false;
  webBridgeState.bridgeReady = false;
  webBridgeState.authRequired = false;
  webBridgeState.authenticated = true;
  webBridgeState.errorText = "";
  notifyWebBridgeStateChanged();

  webBridgeConnectPromise = new Promise<void>((resolve, reject) => {
    let settled = false;
    let readyTimer: number | null = null;
    const finishReady = () => {
      if (settled) return;
      settled = true;
      if (readyTimer !== null) window.clearTimeout(readyTimer);
      webBridgeState.connecting = false;
      webBridgeState.connected = true;
      notifyWebBridgeStateChanged();
      resolve();
    };
    const fail = (error: unknown) => {
      if (readyTimer !== null) window.clearTimeout(readyTimer);
      resetWebBridgeConnectionState(String(error || "PAI 未运行"));
      if (!settled) {
        settled = true;
        reject(error instanceof Error ? error : new Error(String(error || "PAI 未运行")));
      }
    };
    try {
      const socket = new WebSocket(config.chatUrl);
      webBridgeSocket = socket;
      socket.onopen = () => {
        webBridgeState.connected = true;
        readyTimer = window.setTimeout(() => {
          if (!webBridgeState.bridgeReady) fail(new Error("等待 PAI Web 桥接就绪超时"));
        }, 5000);
      };
      socket.onerror = () => fail(new Error("PAI 未运行"));
      socket.onclose = () => {
        if (!settled) {
          fail(new Error("PAI 未运行"));
        } else {
          resetWebBridgeConnectionState("连接已断开");
        }
      };
      socket.onmessage = (event) => handleWebBridgeMessage(event, finishReady);
    } catch (error) {
      fail(error);
    }
  }).finally(() => {
    webBridgeConnectPromise = null;
  });
  return webBridgeConnectPromise;
}

function webBridgeTimeoutForCommand(command: string, requestedTimeoutMs?: number): number | null {
  if (typeof requestedTimeoutMs === "number") return requestedTimeoutMs;
  if (WEB_BRIDGE_NO_TIMEOUT_COMMANDS.has(command)) return null;
  return WEB_BRIDGE_COMMAND_TIMEOUT_MS[command] ?? WEB_BRIDGE_DEFAULT_TIMEOUT_MS;
}

async function invokeWebBridge<T>(command: string, args?: Record<string, unknown>, timeoutMs?: number): Promise<T> {
  if (!ensureWebBridgeConfig()) throw new Error("缺少 PAI Web 桥接配置。");
  await connectWebBridge();
  if (command !== "auth.login" && webBridgeState.authRequired && !webBridgeState.authenticated) {
    await ensureWebBridgeAuthenticated();
  }

  const send = () => {
    const currentSocket = webBridgeSocket;
    if (!currentSocket || currentSocket.readyState !== WebSocket.OPEN) {
      throw new Error("PAI 未运行");
    }
    const id = webBridgeRequestId++;
    const authToken = String(webBridgeConfig?.token || "").trim();
    const params = authToken ? { authToken, ...(args || {}) } : (args || {});
    const body = { jsonrpc: "2.0", id, method: command, params };
    return new Promise<T>((resolve, reject) => {
      const resolvedTimeoutMs = webBridgeTimeoutForCommand(command, timeoutMs);
      const timer = resolvedTimeoutMs === null
        ? null
        : window.setTimeout(() => {
            webBridgePending.delete(id);
            reject(new Error("请求超时"));
          }, resolvedTimeoutMs);
      webBridgePending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      try {
        currentSocket.send(JSON.stringify(body));
      } catch (error) {
        webBridgePending.delete(id);
        if (timer !== null) window.clearTimeout(timer);
        reject(error);
      }
    });
  };

  try {
    return await send();
  } catch (error) {
    if (command === "auth.login" || !isWebBridgeAuthenticationRefreshError(error)) throw error;
    await ensureWebBridgeAuthenticated();
    return send();
  }
}

function webAttachmentUuidToBytes(value: string): Uint8Array {
  const normalized = String(value || "").replace(/-/g, "").trim();
  if (!/^[0-9a-f]{32}$/i.test(normalized)) {
    throw new Error("附件传输 ID 无效");
  }
  const bytes = new Uint8Array(16);
  for (let index = 0; index < 16; index += 1) {
    bytes[index] = Number.parseInt(normalized.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function waitForWebAttachmentChunk(
  transferId: string,
  timeoutMs = 30000,
): Promise<{ transferId: string; nextOffset: number }> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      webBridgePendingAttachmentChunks.delete(transferId);
      reject(new Error("附件分块确认超时"));
    }, timeoutMs);
    webBridgePendingAttachmentChunks.set(transferId, { resolve, reject, timer });
  });
}

function clearPendingWebAttachmentChunk(transferId: string) {
  const request = webBridgePendingAttachmentChunks.get(transferId);
  if (!request) return;
  webBridgePendingAttachmentChunks.delete(transferId);
  window.clearTimeout(request.timer);
}

async function uploadWebBridgeAttachment<T>(file: File): Promise<T> {
  const size = Number(file.size || 0);
  if (size > 50 * 1024 * 1024) {
    const error = new Error("FILE_TOO_LARGE: 文件太大，单个文件不能超过 50 MiB") as Error & { code?: string };
    error.code = "FILE_TOO_LARGE";
    throw error;
  }
  const begin = await invokeWebBridge<{ transferId: string; nextOffset: number; chunkSize?: number }>(
    "attachment.transfer.begin",
    {
      fileName: String(file.name || "attachment").trim() || "attachment",
      mime: String(file.type || "").trim(),
      size,
    },
    30000,
  );
  const transferId = String(begin?.transferId || "").trim();
  if (!transferId) throw new Error("附件传输未返回 transferId");
  const chunkSize = Math.max(1, Math.min(Number(begin?.chunkSize || 256 * 1024), 256 * 1024));
  let offset = Number(begin?.nextOffset || 0);
  try {
    while (offset < size) {
      const end = Math.min(size, offset + chunkSize);
      const chunk = new Uint8Array(await file.slice(offset, end).arrayBuffer());
      if (chunk.length === 0) throw new Error("附件分块为空");
      const frame = new Uint8Array(29 + chunk.length);
      frame[0] = 1;
      frame.set(webAttachmentUuidToBytes(transferId), 1);
      const frameView = new DataView(frame.buffer);
      frameView.setUint32(17, Math.floor(offset / 0x100000000), false);
      frameView.setUint32(21, offset >>> 0, false);
      frameView.setUint32(25, chunk.length, false);
      frame.set(chunk, 29);
      let ack: { transferId: string; nextOffset: number } | null = null;
      for (let attempt = 0; attempt < 2; attempt += 1) {
        const ackPromise = waitForWebAttachmentChunk(transferId);
        try {
          if (!webBridgeSocket || webBridgeSocket.readyState !== WebSocket.OPEN) {
            throw new Error("连接已断开");
          }
          webBridgeSocket.send(frame);
          ack = await ackPromise;
          break;
        } catch (error) {
          clearPendingWebAttachmentChunk(transferId);
          if (attempt === 1) throw error;
        }
      }
      if (!ack) throw new Error("附件分块传输失败");
      if (ack.nextOffset <= offset || ack.nextOffset > size) {
        throw new Error(`附件分块确认 offset 无效：${ack.nextOffset}`);
      }
      offset = ack.nextOffset;
    }
    return await invokeWebBridge<T>("attachment.transfer.complete", { transferId }, 60000);
  } catch (error) {
    try {
      await invokeWebBridge("attachment.transfer.abort", { transferId }, 5000);
    } catch {
      // 连接断开或会话已清理时无需重复报告 abort 错误。
    }
    throw error;
  }
}

async function abortNativeAttachmentTransfer(transferId: string) {
  try {
    await invokeTauri("attachment_transfer_abort", { input: { transferId } });
  } catch {
    // 完成、断线或后端已清理时无需重复暴露 abort 错误。
  }
}

async function uploadNativeAttachment<T>(file: File): Promise<T> {
  const maxChunkSize = 256 * 1024;
  const begin = await invokeTauri<{ transferId: string; nextOffset: number; chunkSize?: number }>(
    "attachment_transfer_begin",
    {
      input: {
        fileName: String(file.name || "attachment").trim() || "attachment",
        mime: String(file.type || "").trim(),
        size: Number(file.size || 0),
      },
    },
  );
  const transferId = String(begin?.transferId || "").trim();
  if (!transferId) throw new Error("附件传输未返回 transferId");
  const chunkSize = Math.max(1, Math.min(Number(begin?.chunkSize || maxChunkSize), maxChunkSize));
  let offset = Number(begin?.nextOffset || 0);
  try {
    while (offset < file.size) {
      const end = Math.min(file.size, offset + chunkSize);
      const chunk = new Uint8Array(await file.slice(offset, end).arrayBuffer());
      if (chunk.length === 0) throw new Error("附件分块为空");
      let ack: { transferId: string; nextOffset: number } | null = null;
      let lastError: unknown = null;
      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          ack = await invokeNativeTransportBinary("attachment_transfer_chunk", chunk, {
            headers: {
              "x-pai-transfer-id": transferId,
              "x-pai-transfer-offset": String(offset),
            },
          });
          break;
        } catch (error) {
          lastError = error;
          if (attempt === 1) throw error;
        }
      }
      if (!ack) throw lastError instanceof Error ? lastError : new Error("附件分块传输失败");
      const nextOffset = Number(ack.nextOffset);
      if (!Number.isSafeInteger(nextOffset) || nextOffset <= offset || nextOffset > file.size) {
        throw new Error(`附件分块确认 offset 无效：${String(ack.nextOffset)}`);
      }
      offset = nextOffset;
    }
    return await invokeTauri<T>("attachment_transfer_complete", { input: { transferId } });
  } catch (error) {
    await abortNativeAttachmentTransfer(transferId);
    throw error;
  }
}

/** 浏览器 File 上传只经过统一传输适配器，业务层不再选择 IPC 或 WebSocket。 */
export function uploadTransportAttachment<T>(file: File): Promise<T> {
  return isTauriRuntimeAvailable()
    ? uploadNativeAttachment<T>(file)
    : uploadWebBridgeAttachment<T>(file);
}

// ==================== Git 面板 ====================

export type GitPanelDetectOutput = {
  gitAvailable: boolean;
  repoRoot?: string | null;
  checked: boolean;
  error?: string | null;
};

export type GitPanelRunOutput = {
  stdout: string;
  stderr: string;
  exitCode: number;
};

export type GitPanelStatusEntry = {
  path: string;
  stagedStatus: string;
  unstagedStatus: string;
};

export type GitPanelStatusOutput = {
  repoRoot: string;
  branch: string;
  entries: GitPanelStatusEntry[];
  truncated: boolean;
  /** 截断前暂存组实际数量（折叠条尾部显示） */
  stagedTotal: number;
  /** 截断前更改组实际数量（折叠条尾部显示） */
  unstagedTotal: number;
};

export type GitPanelDiffOutput = {
  diff: string;
};

export type GitPanelCommitFileEntry = {
  path: string;
  status: string;
};

export type GitPanelCommitFilesOutput = {
  entries: GitPanelCommitFileEntry[];
};

export type GitPanelBranchEntry = {
  name: string;
  isCurrent: boolean;
  isRemote: boolean;
  /** 分支尖端那次提交的日期（ISO 8601）；用于「新的放前面」排序 */
  committerDate: string;
  /** 上游分支短名（如 origin/main）；空串表示没有配置上游 */
  upstream: string;
  /** 上游已被删除（远程分支没了） */
  upstreamMissing: boolean;
  /** 本地领先上游的提交数 */
  ahead: number;
  /** 本地落后上游的提交数 */
  behind: number;
};

export type GitPanelRemoteEntry = {
  name: string;
  url: string;
};

export type GitPanelStashEntry = {
  reference: string;
  message: string;
};

export type GitPanelLogEntry = {
  hash: string;
  shortHash: string;
  author: string;
  date: string;
  message: string;
  parents: string[];
  refs: string;
};

export type GitPanelLogOutput = {
  entries: GitPanelLogEntry[];
};

export type GitPanelRepoEntry = {
  path: string;
  name: string;
};

export type GitPanelReposOutput = {
  repos: GitPanelRepoEntry[];
};

export type GitPanelDiscoverOutput = {
  gitAvailable: boolean;
  currentRepoRoot?: string | null;
  repos: GitPanelRepoEntry[];
  defaultRepoRoot?: string | null;
  checked: boolean;
  error?: string | null;
};

export type GitPanelWorktreeEntry = {
  path: string;
  name: string;
  branch: string;
  /** 主工作树（仓库根本身） */
  isMain: boolean;
  /** 分离头指针 */
  detached: boolean;
};

export type GitPanelWorktreesOutput = {
  repoRoot: string;
  worktrees: GitPanelWorktreeEntry[];
};

function gitPanelWorkspaceArgs(workspacePath: string): Record<string, unknown> {
  return { input: { workspacePath: String(workspacePath || "").trim() } };
}

function gitPanelPathsArgs(workspacePath: string, paths: string[]): Record<string, unknown> {
  const normalizedPaths = Array.isArray(paths)
    ? paths.map((item) => String(item || "").trim()).filter(Boolean)
    : [];
  if (normalizedPaths.length === 0) throw new Error("缺少文件路径");
  return {
    input: {
      workspacePath: String(workspacePath || "").trim(),
      paths: normalizedPaths,
    },
  };
}

function gitPanelRequiredWorkspace(workspacePath: string): string {
  const normalized = String(workspacePath || "").trim();
  if (!normalized) throw new Error("缺少工作区路径");
  return normalized;
}

export async function gitPanelDetect(workspacePath: string): Promise<GitPanelDetectOutput> {
  return invokeTauri<GitPanelDetectOutput>("git_panel_detect", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelRepos(workspacePath: string, refresh = false): Promise<GitPanelReposOutput> {
  return invokeTauri<GitPanelReposOutput>("git_panel_repos", {
    ...gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)),
    refresh,
  });
}

export async function gitPanelDiscover(workspacePath: string, refresh = false): Promise<GitPanelDiscoverOutput> {
  return invokeTauri<GitPanelDiscoverOutput>("git_panel_discover", {
    ...gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)),
    refresh,
  });
}

/**
 * 当前仓库的工作树列表（主工作树 + linked worktree），按该工作区的「最近打开」历史排序。
 * workspacePath 用于定位历史分组，repoRoot 用于定位仓库；仓库根为空时直接返回空列表。
 */
export async function gitPanelWorktrees(
  workspacePath: string,
  repoRoot: string,
): Promise<GitPanelWorktreesOutput> {
  const normalizedRoot = String(repoRoot || "").trim();
  if (!normalizedRoot) return { repoRoot: "", worktrees: [] };
  return invokeTauri<GitPanelWorktreesOutput>("git_panel_worktrees", {
    input: {
      workspacePath: String(workspacePath || "").trim(),
      repoRoot: normalizedRoot,
    },
  });
}

/** 记录一次「打开过的仓库/工作树」，供仓库栏按最近打开排序；入参不完整时不做任何事 */
export async function gitPanelRememberRepo(workspacePath: string, repoRoot: string): Promise<void> {
  const normalizedRoot = String(repoRoot || "").trim();
  const normalizedWorkspace = String(workspacePath || "").trim();
  if (!normalizedRoot || !normalizedWorkspace) return;
  await invokeTauri<void>("git_panel_remember_repo", {
    input: { workspacePath: normalizedWorkspace, repoRoot: normalizedRoot },
  });
}

export async function gitPanelStatus(workspacePath: string): Promise<GitPanelStatusOutput> {
  return invokeTauri<GitPanelStatusOutput>("git_panel_status", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

/** Git 面板仓库监视事件载荷（后端按路径分类后推送） */
export interface GitPanelWatchEventPayload {
  workspacePath: string;
  workdirChanged: boolean;
  headChanged: boolean;
  refsChanged: boolean;
}

export async function gitPanelWatchStart(workspacePath: string): Promise<void> {
  await invokeTauri<void>("git_panel_watch_start", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelWatchStop(workspacePath: string): Promise<void> {
  await invokeTauri<void>("git_panel_watch_stop", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelDiff(input: {
  workspacePath: string;
  path: string;
  staged?: boolean;
  hash?: string;
  context?: number;
}): Promise<GitPanelDiffOutput> {
  const workspacePath = gitPanelRequiredWorkspace(input.workspacePath);
  const path = String(input.path || "").trim();
  if (!path) throw new Error("缺少文件路径");
  return invokeTauri<GitPanelDiffOutput>("git_panel_diff", {
    input: {
      workspacePath,
      path,
      staged: !!input.staged,
      hash: String(input.hash || "").trim(),
      context: input.context,
    },
  });
}

export async function gitPanelStage(workspacePath: string, paths: string[]): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_stage", gitPanelPathsArgs(gitPanelRequiredWorkspace(workspacePath), paths));
}

export async function gitPanelUnstage(workspacePath: string, paths: string[]): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_unstage", gitPanelPathsArgs(gitPanelRequiredWorkspace(workspacePath), paths));
}

export async function gitPanelCommit(workspacePath: string, message: string, amend = false): Promise<GitPanelRunOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedMessage = String(message || "").trim();
  if (!normalizedMessage) throw new Error("提交信息不能为空");
  return invokeTauri<GitPanelRunOutput>("git_panel_commit", {
    input: { workspacePath: normalizedWorkspace, message: normalizedMessage, amend: !!amend },
  });
}

export async function gitPanelDiscard(workspacePath: string, paths: string[]): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_discard", gitPanelPathsArgs(gitPanelRequiredWorkspace(workspacePath), paths));
}

export async function gitPanelStashList(workspacePath: string): Promise<GitPanelStashEntry[]> {
  return invokeTauri<GitPanelStashEntry[]>("git_panel_stash_list", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelStashFiles(workspacePath: string, stashRef: string): Promise<GitPanelCommitFilesOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedRef = String(stashRef || "").trim();
  if (!normalizedRef) throw new Error("缺少存储引用");
  return invokeTauri<GitPanelCommitFilesOutput>("git_panel_stash_files", {
    input: { workspacePath: normalizedWorkspace, stashRef: normalizedRef },
  });
}

export async function gitPanelStashCreate(workspacePath: string, message = "", staged = false): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_stash_create", {
    input: { workspacePath: gitPanelRequiredWorkspace(workspacePath), message: String(message || "").trim(), staged },
  });
}

export async function gitPanelStashApply(workspacePath: string, stashRef: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_stash_apply", {
    input: { workspacePath: gitPanelRequiredWorkspace(workspacePath), stashRef: String(stashRef || "").trim() },
  });
}

export async function gitPanelStashPop(workspacePath: string, stashRef: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_stash_pop", {
    input: { workspacePath: gitPanelRequiredWorkspace(workspacePath), stashRef: String(stashRef || "").trim() },
  });
}

export async function gitPanelStashDrop(workspacePath: string, stashRef: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_stash_drop", {
    input: { workspacePath: gitPanelRequiredWorkspace(workspacePath), stashRef: String(stashRef || "").trim() },
  });
}

export async function gitPanelBranchList(workspacePath: string): Promise<GitPanelBranchEntry[]> {
  return invokeTauri<GitPanelBranchEntry[]>("git_panel_branch_list", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelBranchCreate(workspacePath: string, name: string, startPoint = ""): Promise<GitPanelRunOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedName = String(name || "").trim();
  if (!normalizedName) throw new Error("分支名不能为空");
  return invokeTauri<GitPanelRunOutput>("git_panel_branch_create", {
    input: { workspacePath: normalizedWorkspace, name: normalizedName, startPoint: String(startPoint || "").trim() },
  });
}

export async function gitPanelCheckout(workspacePath: string, reference: string): Promise<GitPanelRunOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedReference = String(reference || "").trim();
  if (!normalizedReference) throw new Error("引用不能为空");
  return invokeTauri<GitPanelRunOutput>("git_panel_checkout", {
    input: { workspacePath: normalizedWorkspace, reference: normalizedReference },
  });
}

export type GitPanelCheckoutCheckOutput = {
  dirtyPaths: string[];
  changedPaths: string[];
  conflictingPaths: string[];
};

export async function gitPanelCheckoutCheck(workspacePath: string, reference: string): Promise<GitPanelCheckoutCheckOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedReference = String(reference || "").trim();
  if (!normalizedReference) throw new Error("引用不能为空");
  return invokeTauri<GitPanelCheckoutCheckOutput>("git_panel_checkout_check", {
    input: { workspacePath: normalizedWorkspace, reference: normalizedReference },
  });
}

export async function gitPanelResetSoft(workspacePath: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_reset_soft", {
    input: { workspacePath: gitPanelRequiredWorkspace(workspacePath) },
  });
}

export async function gitPanelRemoteList(workspacePath: string): Promise<GitPanelRemoteEntry[]> {
  return invokeTauri<GitPanelRemoteEntry[]>("git_panel_remote_list", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelFetch(workspacePath: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_fetch", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelPull(workspacePath: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_pull", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelPush(workspacePath: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_push", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelSync(workspacePath: string): Promise<GitPanelRunOutput> {
  return invokeTauri<GitPanelRunOutput>("git_panel_sync", gitPanelWorkspaceArgs(gitPanelRequiredWorkspace(workspacePath)));
}

export async function gitPanelLog(workspacePath: string, limit?: number, skip?: number): Promise<GitPanelLogOutput> {
  return invokeTauri<GitPanelLogOutput>("git_panel_log", {
    input: {
      workspacePath: gitPanelRequiredWorkspace(workspacePath),
      limit: Number(limit) || undefined,
      skip: Number(skip) || undefined,
    },
  });
}

export async function gitPanelShow(workspacePath: string, hash: string, path = "", context?: number): Promise<GitPanelDiffOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedHash = String(hash || "").trim();
  if (!normalizedHash) throw new Error("缺少提交哈希");
  return invokeTauri<GitPanelDiffOutput>("git_panel_show", {
    input: { workspacePath: normalizedWorkspace, hash: normalizedHash, path: String(path || "").trim(), context },
  });
}

export async function gitPanelCommitFiles(workspacePath: string, hash: string): Promise<GitPanelCommitFilesOutput> {
  const normalizedWorkspace = gitPanelRequiredWorkspace(workspacePath);
  const normalizedHash = String(hash || "").trim();
  if (!normalizedHash) throw new Error("缺少提交哈希");
  return invokeTauri<GitPanelCommitFilesOutput>("git_panel_commit_files", {
    input: { workspacePath: normalizedWorkspace, hash: normalizedHash },
  });
}
