import { afterEach, describe, expect, it, vi } from "vitest";

const { listeners, listenMock, invokeMock } = vi.hoisted(() => {
  const listeners = new Map<string, (event: { payload: unknown }) => void>();
  const listenMock = vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  });
  const invokeMock = vi.fn();
  return { listeners, listenMock, invokeMock };
});

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
  listen: listenMock,
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

import {
  bindTransportConversationStream,
  createTransportChannel,
  disconnectTransport,
  emitTransportEvent,
  ensureTransportReady,
  exportTransportConfigMigrationPackage,
  invokeTauri,
  isTauriRuntimeAvailable,
  onTransportNotification,
  pickBrowserTransportFiles,
  applyTransportConfigMigrationPackage,
  previewTransportConfigMigrationPackage,
  probeTransportConversationStream,
  requestRemotePasswordFromParent,
  unbindTransportConversationStream,
} from "./tauri-api";

type TestWebSocketRequest = {
  id: number;
  method: string;
  params?: Record<string, unknown>;
};

const webSockets: TestWebSocket[] = [];
const webRequestHandlers = new Map<string, (socket: TestWebSocket, request: TestWebSocketRequest) => void>();

class TestWebSocket {
  static readonly OPEN = 1;
  static readonly CLOSED = 3;
  readonly url: string;
  readyState = 0;
  sent: TestWebSocketRequest[] = [];
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    webSockets.push(this);
    queueMicrotask(() => {
      this.readyState = TestWebSocket.OPEN;
      this.onopen?.();
      this.emitNotification("bridge.ready", { authRequired: false });
    });
  }

  send(body: string) {
    const request = JSON.parse(body) as TestWebSocketRequest;
    this.sent.push(request);
    webRequestHandlers.get(request.method)?.(this, request);
  }

  close() {
    this.readyState = TestWebSocket.CLOSED;
    this.onclose?.();
  }

  respond(request: TestWebSocketRequest, result: unknown) {
    this.onmessage?.({ data: JSON.stringify({ jsonrpc: "2.0", id: request.id, result }) });
  }

  reject(request: TestWebSocketRequest, message: string) {
    this.onmessage?.({ data: JSON.stringify({ jsonrpc: "2.0", id: request.id, error: { message } }) });
  }

  emitNotification(method: string, params: unknown) {
    this.onmessage?.({ data: JSON.stringify({ jsonrpc: "2.0", method, params }) });
  }
}

function installWindow(value: Record<string, unknown>) {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    writable: true,
    value,
  });
}

function installNativeRuntime() {
  installWindow({
    __TAURI_INTERNALS__: { invoke: vi.fn() },
    parent: null,
  });
}

function installWebRuntime() {
  const storage = new Map<string, string>();
  const windowValue: Record<string, unknown> = {
    __PAI_SETTINGS_BRIDGE__: { chatUrl: "ws://test.local/chat" },
    location: { host: "test.local", protocol: "http:", pathname: "/chat", search: "" },
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, String(value)),
      removeItem: (key: string) => storage.delete(key),
    },
    setTimeout,
    clearTimeout,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  windowValue.parent = windowValue;
  installWindow(windowValue);
  Object.defineProperty(globalThis, "WebSocket", {
    configurable: true,
    writable: true,
    value: TestWebSocket,
  });
}

describe("统一传输通知适配器", () => {
  afterEach(() => {
    disconnectTransport();
    listeners.clear();
    listenMock.mockClear();
    invokeMock.mockReset();
    webRequestHandlers.clear();
    webSockets.splice(0);
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "WebSocket");
  });

  it("chat.roundFinished 同时收口 completed 与 failed 事件", async () => {
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { __TAURI_INTERNALS__: { invoke: vi.fn() }, parent: null },
    });
    const handler = vi.fn();
    const stop = onTransportNotification("chat.roundFinished", handler);
    await Promise.resolve();

    expect(listenMock).toHaveBeenCalledTimes(2);
    expect(listenMock.mock.calls.map(([event]) => event)).toEqual([
      "easy-call:round-completed",
      "easy-call:round-failed",
    ]);

    listeners.get("easy-call:round-completed")?.({ payload: { status: "completed" } });
    listeners.get("easy-call:round-failed")?.({ payload: { status: "failed" } });
    expect(handler).toHaveBeenNthCalledWith(1, { status: "completed" });
    expect(handler).toHaveBeenNthCalledWith(2, { status: "failed" });

    stop();
  });

  it("Web 本地事件与原生事件复用同一个通知语义", async () => {
    installWebRuntime();
    const handler = vi.fn();
    const stop = onTransportNotification("uiSize.changed", handler);

    await emitTransportEvent("uiSize.changed", { scale: 1.1 });

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ scale: 1.1 });
    stop();
  });

  it("原生绑定把真实 Channel 传给 invoke，保护桌面流式回归", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue(undefined);
    const channel = createTransportChannel<{ delta: string }>();

    await bindTransportConversationStream({
      bindingId: "native-binding",
      conversationId: "conversation-native",
      onDelta: channel,
    });

    const bindCall = invokeMock.mock.calls.find(([command]) => command === "bind_active_chat_view_stream");
    expect(bindCall).toBeDefined();
    expect(bindCall?.[1]).toEqual(expect.objectContaining({
      input: { bindingId: "native-binding", conversationId: "conversation-native" },
      onDelta: channel,
    }));
    await unbindTransportConversationStream({ bindingId: "native-binding" });
  });

  it("统一发送协议在桌面保留真实 Channel，在 Web 只发送同一业务 payload", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({ conversationId: "conversation-native-send" });
    const nativeChannel = createTransportChannel<{ delta: string }>();
    const input = {
      payload: { text: "hello", displayText: "hello", parts: [{ type: "text", text: "hello" }] },
      session: {
        apiConfigId: "api-1",
        agentId: "agent-1",
        conversationId: "conversation-native-send",
      },
      traceId: "trace-native-send",
    };

    await invokeTauri("chat.send", { input, onDelta: nativeChannel });
    expect(invokeMock).toHaveBeenCalledWith("submit_chat_message", {
      input,
      onDelta: nativeChannel,
    });

    installWebRuntime();
    webRequestHandlers.set("chat.send", (socket, request) => {
      socket.respond(request, { conversationId: "conversation-web-send" });
    });
    await ensureTransportReady();
    const webChannel = createTransportChannel<{ delta: string }>();
    await invokeTauri("chat.send", { input, onDelta: webChannel });
    const request = webSockets[0]?.sent.find((item) => item.method === "chat.send");
    expect(request?.params).toEqual(input);
    expect(request?.params).not.toHaveProperty("onDelta");
  });

  it("会话轻量读取协议统一映射参数包装", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({ conversationId: "conversation-read" });
    await invokeTauri("conversation.runtimeSnapshot", { conversationId: "conversation-read" });
    expect(invokeMock).toHaveBeenLastCalledWith("get_conversation_runtime_snapshot", {
      conversationId: "conversation-read",
    });
    await invokeTauri("conversation.messageById", {
      input: { conversationId: "conversation-read", messageId: "message-1" },
    });
    expect(invokeMock).toHaveBeenLastCalledWith("get_unarchived_conversation_message_by_id", {
      input: { conversationId: "conversation-read", messageId: "message-1" },
    });

    installWebRuntime();
    webRequestHandlers.set("conversation.messageById", (socket, request) => socket.respond(request, {}));
    await ensureTransportReady();
    await invokeTauri("conversation.messageById", {
      input: { conversationId: "conversation-read", messageId: "message-1" },
    });
    const request = webSockets[0]?.sent.find((item) => item.method === "conversation.messageById");
    expect(request?.params).toEqual({ conversationId: "conversation-read", messageId: "message-1" });
  });

  it("Web 与桌面都保留后端的前台流绑定判断", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({ conversationId: "conversation-native-idle", shouldBindStream: false });
    await expect(invokeTauri("conversation.foregroundLightSnapshot", {
      input: { conversationId: "conversation-native-idle", resumeProjection: true },
    })).resolves.toEqual({ conversationId: "conversation-native-idle", shouldBindStream: false });

    installWebRuntime();
    webRequestHandlers.set("conversation.foregroundLightSnapshot", (socket, request) => {
      socket.respond(request, { conversationId: "conversation-web-idle", shouldBindStream: false });
    });
    await ensureTransportReady();
    await expect(invokeTauri("conversation.foregroundLightSnapshot", {
      input: { conversationId: "conversation-web-idle", resumeProjection: true },
    })).resolves.toEqual({ conversationId: "conversation-web-idle", shouldBindStream: false });
  });

  it("统一会话维护命令由适配器映射到桌面 command，并在 Web 解包同一参数", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({ success: true });
    await invokeTauri("conversation.compact", {
      input: { conversationId: "conversation-native-maintenance" },
    });
    expect(invokeMock).toHaveBeenCalledWith("compact_conversation", {
      input: { conversationId: "conversation-native-maintenance" },
    });

    installWebRuntime();
    webRequestHandlers.set("conversation.compact", (socket, request) => {
      socket.respond(request, { success: true });
    });
    await ensureTransportReady();
    await invokeTauri("conversation.compact", {
      input: { conversationId: "conversation-web-maintenance" },
    });
    const request = webSockets[0]?.sent.find((item) => item.method === "conversation.compact");
    expect(request?.params).toEqual({ conversationId: "conversation-web-maintenance" });
  });

  it("会话与工作区 canonical 方法在桌面只映射 command，不让业务层维护 Tauri 参数形状", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({});

    await invokeTauri("conversation.list");
    expect(invokeMock).toHaveBeenLastCalledWith("list_transport_conversations", undefined);

    await invokeTauri("conversation.createOptions");
    expect(invokeMock).toHaveBeenLastCalledWith("list_conversation_create_options", undefined);

    await invokeTauri("workspace.permission", { conversationId: "conversation-workspace" });
    expect(invokeMock).toHaveBeenLastCalledWith("get_conversation_workspace_permission", {
      input: { conversationId: "conversation-workspace" },
    });

    await invokeTauri("workspace.permission.select", {
      conversationId: "conversation-workspace",
      access: "approval",
    });
    expect(invokeMock).toHaveBeenLastCalledWith("select_conversation_workspace_permission", {
      input: { conversationId: "conversation-workspace", access: "approval" },
    });

    await invokeTauri("workspace.list", { conversationId: "conversation-workspace" });
    expect(invokeMock).toHaveBeenLastCalledWith("list_conversation_workspaces", {
      input: { conversationId: "conversation-workspace" },
    });

    await invokeTauri("workspace.layout.save", {
      conversationId: "conversation-workspace",
      shellWorkMode: "directory",
      workspaces: [],
    });
    expect(invokeMock).toHaveBeenLastCalledWith("save_conversation_workspace_layout", {
      input: {
        conversationId: "conversation-workspace",
        shellWorkMode: "directory",
        workspaces: [],
      },
    });
  });

  it("配置迁移业务协议在桌面与 Web 共用同一组适配器方法", async () => {
    installNativeRuntime();
    invokeMock.mockResolvedValue({ ok: true });

    await exportTransportConfigMigrationPackage({ password: "secret1" });
    expect(invokeMock).toHaveBeenLastCalledWith("export_config_migration_package", {
      input: { password: "secret1" },
    });
    await previewTransportConfigMigrationPackage({
      password: "secret1",
      packageFileName: "migration.zip",
      packageBytesBase64: "emlw",
    });
    expect(invokeMock).toHaveBeenLastCalledWith("preview_import_config_migration_package", {
      input: {
        password: "secret1",
        packageFileName: "migration.zip",
        packageBytesBase64: "emlw",
      },
    });
    await applyTransportConfigMigrationPackage("preview-native");
    expect(invokeMock).toHaveBeenLastCalledWith("apply_import_config_migration_package", {
      input: { previewId: "preview-native" },
    });

    installWebRuntime();
    for (const method of ["configMigration.export", "configMigration.preview", "configMigration.apply"]) {
      webRequestHandlers.set(method, (socket, request) => socket.respond(request, { ok: true }));
    }
    await ensureTransportReady();
    await exportTransportConfigMigrationPackage({ password: "secret1" });
    await previewTransportConfigMigrationPackage({
      password: "secret1",
      packageFileName: "migration.zip",
      packageBytesBase64: "emlw",
    });
    await applyTransportConfigMigrationPackage("preview-web");
    const requests = webSockets[0]?.sent.filter((item) => item.method.startsWith("configMigration.")) || [];
    expect(requests.map((item) => [item.method, item.params])).toEqual([
      ["configMigration.export", { password: "secret1" }],
      ["configMigration.preview", {
        password: "secret1",
        packageFileName: "migration.zip",
        packageBytesBase64: "emlw",
      }],
      ["configMigration.apply", { previewId: "preview-web" }],
    ]);
  });

  it("Web 异步补消息使用同一命令与完成通知，不再退化为另一套恢复路径", async () => {
    installWebRuntime();
    webRequestHandlers.set("conversation.messagesAfterAsync", (socket, request) => {
      socket.respond(request, { accepted: true, requestId: "request-web-tail" });
    });
    await ensureTransportReady();
    const handler = vi.fn();
    const stop = onTransportNotification("conversation.messagesAfterSynced", handler);

    await expect(invokeTauri("conversation.messagesAfterAsync", {
      input: {
        conversationId: "conversation-web-tail",
        afterMessageId: "message-anchor",
        fallbackLimit: 20,
      },
    })).resolves.toEqual({ accepted: true, requestId: "request-web-tail" });

    const socket = webSockets[0];
    const request = socket?.sent.find((item) => item.method === "conversation.messagesAfterAsync");
    expect(request?.params).toEqual({
      conversationId: "conversation-web-tail",
      afterMessageId: "message-anchor",
      fallbackLimit: 20,
    });
    socket.emitNotification("conversation.messagesAfterSynced", {
      requestId: "request-web-tail",
      conversationId: "conversation-web-tail",
      afterMessageId: "message-anchor",
      messages: [],
      fallbackMode: null,
      error: null,
    });
    expect(handler).toHaveBeenCalledWith(expect.objectContaining({
      requestId: "request-web-tail",
      conversationId: "conversation-web-tail",
    }));
    stop();
  });

  it("Web assistantDelta 通知进入同一个虚拟 TransportChannel，probe ack 也走同一通道", async () => {
    installWebRuntime();
    webRequestHandlers.set("conversation.resumeSubscription", (socket, request) => socket.respond(request, {}));
    webRequestHandlers.set("conversation.streamProbe", (socket, request) => {
      socket.respond(request, { delivered: true });
      socket.emitNotification("chat.streamProbeAck", {
        conversationId: request.params?.conversationId,
        probeId: request.params?.probeId,
      });
    });
    await ensureTransportReady();
    const channel = createTransportChannel<{ delta?: string; kind?: string; message?: string }>();
    const received: unknown[] = [];
    channel.onmessage = (event) => received.push(event);
    const broadcastHandler = vi.fn();
    const stopBroadcast = onTransportNotification("chat.assistantDelta", broadcastHandler);

    await bindTransportConversationStream({
      bindingId: "web-binding",
      conversationId: "conversation-web",
      onDelta: channel,
    });
    const socket = webSockets[0];
    socket.emitNotification("chat.assistantDelta", {
      conversationId: "conversation-web",
      event: { delta: "网络流式" },
    });
    expect(received).toContainEqual({ delta: "网络流式" });

    socket.emitNotification("chat.assistantDelta", {
      conversationId: "conversation-web",
      event: { kind: "tool_status", toolStatus: "执行中" },
    });
    expect(received).not.toContainEqual({ kind: "tool_status", toolStatus: "执行中" });
    expect(broadcastHandler).toHaveBeenCalledWith({
      conversationId: "conversation-web",
      event: { kind: "tool_status", toolStatus: "执行中" },
    });

    const probeResult = await probeTransportConversationStream({
      bindingId: "web-binding",
      conversationId: "conversation-web",
      probeId: "probe-web",
    });
    expect(probeResult).toBe(true);
    expect(received).toContainEqual({ kind: "stream_probe", message: "probe-web" });
    stopBroadcast();
    await unbindTransportConversationStream({ bindingId: "web-binding" });
  });

  it("Web 新绑定失败时保留旧绑定并继续收流", async () => {
    installWebRuntime();
    let failNext = false;
    webRequestHandlers.set("conversation.resumeSubscription", (socket, request) => {
      if (failNext) {
        failNext = false;
        socket.reject(request, "订阅失败");
      } else {
        socket.respond(request, {});
      }
    });
    await ensureTransportReady();
    const socket = webSockets[0];
    const oldChannel = createTransportChannel<{ delta?: string }>();
    const oldReceived: unknown[] = [];
    oldChannel.onmessage = (event) => oldReceived.push(event);
    await bindTransportConversationStream({
      bindingId: "web-failing-rebind",
      conversationId: "conversation-old",
      onDelta: oldChannel,
    });

    failNext = true;
    const nextChannel = createTransportChannel<{ delta?: string }>();
    const nextReceived: unknown[] = [];
    nextChannel.onmessage = (event) => nextReceived.push(event);
    await expect(bindTransportConversationStream({
      bindingId: "web-failing-rebind",
      conversationId: "conversation-new",
      onDelta: nextChannel,
    })).rejects.toThrow("订阅失败");

    socket.emitNotification("chat.assistantDelta", {
      conversationId: "conversation-old",
      event: { delta: "旧流仍在" },
    });
    expect(oldReceived).toContainEqual({ delta: "旧流仍在" });
    expect(nextReceived).toEqual([]);
    await unbindTransportConversationStream({ bindingId: "web-failing-rebind" });
  });

  it("Web 绑定与解绑竞态不会恢复过期 binding", async () => {
    installWebRuntime();
    let delayedRequest: { socket: TestWebSocket; request: TestWebSocketRequest } | null = null;
    webRequestHandlers.set("conversation.resumeSubscription", (socket, request) => {
      if (request.params?.conversationId === "conversation-race") {
        delayedRequest = { socket, request };
      } else {
        socket.respond(request, {});
      }
    });
    await ensureTransportReady();
    const socket = webSockets[0];
    const channel = createTransportChannel<{ delta?: string }>();
    const received: unknown[] = [];
    channel.onmessage = (event) => received.push(event);
    const bindingPromise = bindTransportConversationStream({
      bindingId: "web-race",
      conversationId: "conversation-race",
      onDelta: channel,
    });
    await Promise.resolve();
    await unbindTransportConversationStream({ bindingId: "web-race" });
    const pending = delayedRequest as { socket: TestWebSocket; request: TestWebSocketRequest } | null;
    expect(pending).not.toBeNull();
    if (!pending) throw new Error("未捕获延迟订阅请求");
    pending.socket.respond(pending.request, {});
    await bindingPromise;

    socket.emitNotification("chat.assistantDelta", {
      conversationId: "conversation-race",
      event: { delta: "过期流" },
    });
    expect(received).toEqual([]);
    expect(await probeTransportConversationStream({
      bindingId: "web-race",
      conversationId: "conversation-race",
      probeId: "probe-expired",
    })).toBe(false);
  });
});

describe("isTauriRuntimeAvailable", () => {
  it("iframe 嵌入（self !== top）时即使宿主注入 __TAURI_INTERNALS__ 也视为 Web 宿主", () => {
    expect(
      isTauriRuntimeAvailable({
        self: {},
        top: {},
        __TAURI_INTERNALS__: { invoke: (() => undefined) as unknown },
      }),
    ).toBe(false);
  });

  it("桌面独立窗口（self === top）无 __TAURI_INTERNALS__ 时返回 false", () => {
    const self = {} as unknown;
    expect(isTauriRuntimeAvailable({ self, top: self })).toBe(false);
  });

  it("桌面独立窗口存在 __TAURI_INTERNALS__.invoke 时返回 true", () => {
    const self = {} as unknown;
    expect(
      isTauriRuntimeAvailable({
        self,
        top: self,
        __TAURI_INTERNALS__: { invoke: (() => undefined) as unknown },
      }),
    ).toBe(true);
  });
});

describe("requestRemotePasswordFromParent", () => {
  const REMOTE_AUTH_SOURCE = "pai-remote-bridge-auth";
  const ALLOWED_ORIGIN = "https://tauri.localhost";

  function createHostWindowMock() {
    const parent = { postMessage: vi.fn() } as unknown as Window;
    const listeners = new Set<(event: MessageEvent) => void>();
    let timeoutCallback: (() => void) | undefined;
    const win = {
      parent,
      addEventListener: vi.fn(
        (_type: string, listener: (event: MessageEvent) => void) => {
          listeners.add(listener);
        },
      ),
      removeEventListener: vi.fn(
        (_type: string, listener: (event: MessageEvent) => void) => {
          listeners.delete(listener);
        },
      ),
      setTimeout: vi.fn((callback: () => void) => {
        timeoutCallback = callback;
        return 1;
      }),
      clearTimeout: vi.fn(),
    } as unknown as Window;
    return {
      win,
      parent,
      listeners,
      runTimeout: () => timeoutCallback?.(),
    };
  }

  function makeMessageEvent(
    source: unknown,
    origin: string,
    password: string,
  ) {
    return {
      origin,
      source,
      data: {
        source: REMOTE_AUTH_SOURCE,
        method: "password",
        payload: { password },
      },
    } as unknown as MessageEvent;
  }

  it("来源为 window.parent 且 origin 白名单匹配时返回密码", async () => {
    const { win, parent, listeners } = createHostWindowMock();
    const promise = requestRemotePasswordFromParent(win);
    const listener = [...listeners][0];
    listener(makeMessageEvent(parent, ALLOWED_ORIGIN, "secret"));
    await expect(promise).resolves.toBe("secret");
  });

  it("拒绝非 window.parent 来源的密码消息", async () => {
    const { win, listeners, runTimeout } = createHostWindowMock();
    const promise = requestRemotePasswordFromParent(win);
    const listener = [...listeners][0];
    // 伪造来源：恶意 iframe 以自身作为 event.source，而非 window.parent。
    listener(makeMessageEvent({} as Window, ALLOWED_ORIGIN, "evil"));
    // 拒绝路径不 settle，最终由 1500ms 超时兜底返回空串。
    runTimeout();
    await expect(promise).resolves.toBe("");
  });

  it("拒绝非白名单 origin 的密码消息", async () => {
    const { win, parent, listeners, runTimeout } = createHostWindowMock();
    const promise = requestRemotePasswordFromParent(win);
    const listener = [...listeners][0];
    listener(
      makeMessageEvent(parent, "https://evil.example.com", "evil"),
    );
    runTimeout();
    await expect(promise).resolves.toBe("");
  });
});

describe("pickBrowserTransportFiles 安卓 focus/change 时序", () => {
  type FakeInput = {
    type: string;
    multiple: boolean;
    style: Record<string, string>;
    accept: string;
    files: File[] | null;
    listeners: Map<string, () => void>;
    addEventListener: ReturnType<typeof vi.fn>;
    removeEventListener: ReturnType<typeof vi.fn>;
    click: ReturnType<typeof vi.fn>;
    remove: ReturnType<typeof vi.fn>;
  };

  function createFileDialogHost() {
    const focusListeners = new Set<() => void>();
    const input: FakeInput = {
      type: "",
      multiple: false,
      style: {},
      accept: "",
      files: null,
      listeners: new Map(),
      addEventListener: vi.fn((type: string, listener: () => void) => {
        input.listeners.set(type, listener);
      }),
      removeEventListener: vi.fn(() => undefined),
      click: vi.fn(() => undefined),
      remove: vi.fn(() => undefined),
    };
    const doc = {
      createElement: vi.fn((tag: string) => {
        if (tag !== "input") throw new Error(`unexpected tag ${tag}`);
        return input;
      }),
      body: { appendChild: vi.fn(() => undefined) },
    };
    const win = {
      setTimeout: (cb: () => void, ms?: number) => setTimeout(cb, ms ?? 0),
      clearTimeout: (id: number) => clearTimeout(id),
      addEventListener: vi.fn((type: string, listener: () => void) => {
        if (type === "focus") focusListeners.add(listener);
      }),
      removeEventListener: vi.fn((type: string, listener: () => void) => {
        if (type === "focus") focusListeners.delete(listener);
      }),
    };
    const prevDoc = globalThis.document;
    const prevWin = globalThis.window;
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      writable: true,
      value: doc,
    });
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      writable: true,
      value: win,
    });
    return {
      input,
      doc,
      win,
      focusListeners,
      triggerFocus: () => {
        for (const listener of [...focusListeners]) listener();
      },
      triggerChange: () => {
        input.listeners.get("change")?.();
      },
      triggerCancel: () => {
        input.listeners.get("cancel")?.();
      },
      restore: () => {
        Object.defineProperty(globalThis, "document", {
          configurable: true,
          writable: true,
          value: prevDoc,
        });
        Object.defineProperty(globalThis, "window", {
          configurable: true,
          writable: true,
          value: prevWin,
        });
      },
    };
  }

  it("focus 先于 change 触发时不丢弃已选文件（安卓 Photo Picker 时序）", async () => {
    const host = createFileDialogHost();
    try {
      const file = new File(["a"], "photo.jpg", { type: "image/jpeg" });
      const promise = pickBrowserTransportFiles({ multiple: true });
      expect(host.input.click).toHaveBeenCalledTimes(1);

      // 安卓时序：返回页面先触发 window focus，此时 files 尚未填充。
      host.input.files = null;
      host.triggerFocus();
      // focus 兜底定时器未到点，change 随后到达且已带文件。
      host.input.files = [file];
      host.triggerChange();
      await expect(promise).resolves.toEqual([file]);
    } finally {
      host.restore();
    }
  });

  it("focus 触发后 change 未到且 files 为空时视为取消返回空数组", async () => {
    vi.useFakeTimers();
    const host = createFileDialogHost();
    try {
      const promise = pickBrowserTransportFiles({ multiple: true });
      host.input.files = null;
      host.triggerFocus();
      // 模拟 focus 兜底 1000ms 到点，仍无 change → 视为取消。
      await vi.advanceTimersByTimeAsync(1100);
      await expect(promise).resolves.toEqual([]);
    } finally {
      vi.useRealTimers();
      host.restore();
    }
  });

  it("change 正常先触发时直接返回已选文件", async () => {
    const host = createFileDialogHost();
    try {
      const file = new File(["b"], "doc.txt", { type: "text/plain" });
      const promise = pickBrowserTransportFiles({ multiple: true });
      host.input.files = [file];
      host.triggerChange();
      await expect(promise).resolves.toEqual([file]);
    } finally {
      host.restore();
    }
  });
});
