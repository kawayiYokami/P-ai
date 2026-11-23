import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, promptDialogMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), promptDialogMock: vi.fn() }));

vi.mock("../features/shared/services/confirm-dialog", () => ({
  promptDialog: promptDialogMock,
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: ((message: unknown) => void) | null = null;
  },
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(),
  emitTo: vi.fn(),
  listen: vi.fn(async () => () => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    innerSize: vi.fn(async () => ({ width: 1, height: 1 })),
    label: "chat",
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: vi.fn(async () => () => {}) }),
}));

type WebRequest = {
  id: number;
  method: string;
  params?: Record<string, unknown>;
};

type RequestHandler = (socket: TestWebSocket, request: WebRequest) => void;

let bridgeAuthRequired = false;
let requestHandlers = new Map<string, RequestHandler>();
let sockets: TestWebSocket[] = [];

class TestWebSocket {
  static readonly OPEN = 1;
  static readonly CLOSED = 3;
  readonly sent: WebRequest[] = [];
  readyState = 0;
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;

  constructor(readonly url: string) {
    sockets.push(this);
    queueMicrotask(() => {
      this.readyState = TestWebSocket.OPEN;
      this.onopen?.();
      this.notify("bridge.ready", { authRequired: bridgeAuthRequired });
    });
  }

  send(body: string) {
    const request = JSON.parse(body) as WebRequest;
    this.sent.push(request);
    requestHandlers.get(request.method)?.(this, request);
  }

  close() {
    this.readyState = TestWebSocket.CLOSED;
    this.onclose?.();
  }

  respond(request: WebRequest, result: unknown) {
    this.onmessage?.({ data: JSON.stringify({ jsonrpc: "2.0", id: request.id, result }) });
  }

  reject(request: WebRequest, message: string) {
    this.onmessage?.({
      data: JSON.stringify({ jsonrpc: "2.0", id: request.id, error: { message } }),
    });
  }

  notify(method: string, params: unknown) {
    this.onmessage?.({ data: JSON.stringify({ jsonrpc: "2.0", method, params }) });
  }
}

function installWebWindow(options: {
  discovery?: Record<string, unknown>;
  prompt?: () => string | null;
  vscodePostMessage?: (message: unknown) => void;
}) {
  const storage = new Map<string, string>();
  const listeners = new Map<string, (event: { data: unknown }) => void>();
  const windowValue: Record<string, unknown> = {
    location: {
      host: "",
      protocol: "vscode-webview:",
      pathname: "/sidebar.html",
      search: "",
    },
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, String(value)),
      removeItem: (key: string) => storage.delete(key),
    },
    setTimeout,
    clearTimeout,
    prompt: options.prompt || (() => null),
    addEventListener: (event: string, handler: (payload: { data: unknown }) => void) => {
      listeners.set(event, handler);
    },
    removeEventListener: vi.fn(),
  };
  if (options.discovery) windowValue.__PAI_SIDEBAR_BRIDGE__ = options.discovery;
  if (options.vscodePostMessage) {
    windowValue.acquireVsCodeApi = () => ({ postMessage: options.vscodePostMessage });
  }
  windowValue.parent = windowValue;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    writable: true,
    value: windowValue,
  });
  Object.defineProperty(globalThis, "WebSocket", {
    configurable: true,
    writable: true,
    value: TestWebSocket,
  });
  return {
    dispatchHostMessage: (data: unknown) => listeners.get("message")?.({ data }),
  };
}

describe("Web transport bootstrap", () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    promptDialogMock.mockReset();
    bridgeAuthRequired = false;
    requestHandlers = new Map();
    sockets = [];
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "WebSocket");
  });

  it("discovery 到达前先记住会话，并节流请求宿主刷新", async () => {
    const postMessage = vi.fn();
    const host = installWebWindow({ vscodePostMessage: postMessage });
    requestHandlers.set("workspace.ensureHostRoot", (socket, request) => {
      socket.respond(request, { conversationId: request.params?.conversationId });
    });
    const transport = await import("./tauri-api");

    await transport.prepareTransportConversationContext("conversation-delayed-discovery");
    await transport.prepareTransportConversationContext("conversation-delayed-discovery");
    transport.requestTransportDiscoveryRefresh();

    expect(postMessage).toHaveBeenCalledTimes(1);
    expect(postMessage).toHaveBeenCalledWith({ type: "pai-refresh-discovery" });

    host.dispatchHostMessage({
      type: "pai-discovery",
      discovery: {
        chatUrl: "ws://test.local/chat",
        workspaceRoots: [{ path: "C:/workspace", name: "workspace" }],
      },
    });

    await vi.waitFor(() => {
      const request = sockets[0]?.sent.find((item) => item.method === "workspace.ensureHostRoot");
      expect(request?.params).toEqual({
        conversationId: "conversation-delayed-discovery",
        workspacePath: "C:/workspace",
        workspaceName: "workspace",
      });
    });
    transport.disconnectTransport();
  });

  it("并发业务请求共享一次适配器认证", async () => {
    bridgeAuthRequired = true;
    const prompt = vi.fn(() => "secret");
    promptDialogMock.mockResolvedValueOnce("secret");
    installWebWindow({
      discovery: { chatUrl: "ws://test.local/chat" },
      prompt,
    });
    requestHandlers.set("auth.login", (socket, request) => {
      socket.respond(request, { authenticated: true, authToken: "token-1" });
    });
    requestHandlers.set("load_config", (socket, request) => socket.respond(request, { ok: true }));
    requestHandlers.set("load_agents", (socket, request) => socket.respond(request, []));
    const transport = await import("./tauri-api");

    await Promise.all([
      transport.invokeTauri("load_config"),
      transport.invokeTauri("load_agents"),
    ]);

    const socket = sockets[0];
    expect(promptDialogMock).toHaveBeenCalledTimes(1);
    expect(socket.sent.filter((item) => item.method === "auth.login")).toHaveLength(1);
    expect(socket.sent.find((item) => item.method === "load_config")?.params?.authToken).toBe("token-1");
    expect(socket.sent.find((item) => item.method === "load_agents")?.params?.authToken).toBe("token-1");
    transport.disconnectTransport();
  });

  it("旧 token 失效时由适配器认证并重试原请求", async () => {
    bridgeAuthRequired = true;
    const prompt = vi.fn(() => "fresh-secret");
    promptDialogMock.mockResolvedValueOnce("fresh-secret");
    installWebWindow({
      discovery: { chatUrl: "ws://test.local/chat", token: "stale-token" },
      prompt,
    });
    requestHandlers.set("auth.login", (socket, request) => {
      socket.respond(request, { authenticated: true, authToken: "fresh-token" });
    });
    let configAttempts = 0;
    requestHandlers.set("load_config", (socket, request) => {
      configAttempts += 1;
      if (configAttempts === 1) {
        socket.reject(request, "invalid authToken");
        return;
      }
      socket.respond(request, { ok: true });
    });
    const transport = await import("./tauri-api");

    await expect(transport.invokeTauri("load_config")).resolves.toEqual({ ok: true });

    const requests = sockets[0]?.sent.filter((item) => item.method === "load_config") || [];
    expect(promptDialogMock).toHaveBeenCalledTimes(1);
    expect(requests).toHaveLength(2);
    expect(requests[0]?.params?.authToken).toBe("stale-token");
    expect(requests[1]?.params?.authToken).toBe("fresh-token");
    transport.disconnectTransport();
  });
});
