import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

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

import {
  parseClipboardImageDataUrl,
  writeTransportClipboardImage,
  writeTransportClipboardText,
} from "./tauri-api";

const clipboardWrite = vi.fn(async (items: unknown[]) => {
  // 模拟浏览器内部等待 ClipboardItem 里的 Promise<Blob>。
  const item = items[0] as FakeClipboardItem;
  await Promise.all(Object.values(item.items));
});
const bitmapClose = vi.fn();
const createImageBitmapMock = vi.fn(async () => ({ width: 2, height: 2, close: bitmapClose }));
const convertToBlobMock = vi.fn(async () => new Blob(["png"], { type: "image/png" }));

class FakeClipboardItem {
  constructor(public readonly items: Record<string, Blob | Promise<Blob>>) {}
}

beforeEach(() => {
  clipboardWrite.mockClear();
  createImageBitmapMock.mockClear();
  bitmapClose.mockClear();
  convertToBlobMock.mockClear();
  invokeMock.mockClear();
  vi.stubGlobal("navigator", { clipboard: { write: clipboardWrite } });
  vi.stubGlobal("ClipboardItem", FakeClipboardItem);
  vi.stubGlobal("createImageBitmap", createImageBitmapMock);
  vi.stubGlobal("OffscreenCanvas", class {
    constructor(public readonly width: number, public readonly height: number) {}
    getContext() {
      return { drawImage: vi.fn() };
    }
    convertToBlob = convertToBlobMock;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("parseClipboardImageDataUrl", () => {
  it("解析聊天常见的 webp data URL", () => {
    expect(parseClipboardImageDataUrl("data:image/webp;base64,QUJD")).toEqual({
      mime: "image/webp",
      base64: "QUJD",
      isDataUrl: true,
    });
  });

  it("解析 png data URL", () => {
    expect(parseClipboardImageDataUrl("data:image/png;base64,QUJD")).toEqual({
      mime: "image/png",
      base64: "QUJD",
      isDataUrl: true,
    });
  });

  it("吃掉中间参数段（charset）", () => {
    expect(parseClipboardImageDataUrl("data:image/png;charset=utf-8;base64,QUJD")).toEqual({
      mime: "image/png",
      base64: "QUJD",
      isDataUrl: true,
    });
  });

  it("大写 data URL 前缀同样解析", () => {
    expect(parseClipboardImageDataUrl("DATA:IMAGE/PNG;BASE64,QUJD")).toEqual({
      mime: "image/png",
      base64: "QUJD",
      isDataUrl: true,
    });
  });

  it("无 base64 段时抛明确错误，不误报成解码失败", () => {
    expect(() => parseClipboardImageDataUrl("data:image/svg+xml,<svg/>")).toThrow(
      /缺少 ;base64 段/,
    );
  });

  it("有 ;base64 但缺 mime 时报错文案指向 mime", () => {
    expect(() => parseClipboardImageDataUrl("data:;base64,AAA")).toThrow(/缺少 mime 类型/);
  });

  it("空字符串报图片数据为空", () => {
    expect(() => parseClipboardImageDataUrl("")).toThrow(/图片数据为空/);
  });

  it("裸 base64 不带 mime 标记，必须走转码", () => {
    expect(parseClipboardImageDataUrl("QUJD")).toEqual({
      mime: "",
      base64: "QUJD",
      isDataUrl: false,
    });
  });
});

describe("writeTransportClipboardImage 的 Web 写入路径", () => {
  it("png data URL 直写，不经过 createImageBitmap", async () => {
    const dataUrl = `data:image/png;base64,${btoa("png-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(true);
    expect(createImageBitmapMock).not.toHaveBeenCalled();
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    expect(item.items).toHaveProperty("image/png");
  });

  it("webp data URL 先转 png 再写，并释放 ImageBitmap", async () => {
    const dataUrl = `data:image/webp;base64,${btoa("webp-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(true);
    expect(createImageBitmapMock).toHaveBeenCalledTimes(1);
    expect(bitmapClose).toHaveBeenCalledTimes(1);
    expect(convertToBlobMock).toHaveBeenCalledWith({ type: "image/png" });
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    expect(item.items).toHaveProperty("image/png");
  });

  it("裸 base64 一律转码，不按猜测的 png 直写", async () => {
    await expect(writeTransportClipboardImage({ base64: btoa("webp-bytes") })).resolves.toBe(true);
    expect(createImageBitmapMock).toHaveBeenCalledTimes(1);
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    expect(item.items).toHaveProperty("image/png");
  });

  it("空 payload 的 data URL 返回 false", async () => {
    await expect(writeTransportClipboardImage({ base64: "data:image/png;base64," })).resolves.toBe(false);
    expect(clipboardWrite).not.toHaveBeenCalled();
  });

  it("解析失败返回 false，不写剪贴板", async () => {
    await expect(writeTransportClipboardImage({ base64: "data:image/svg+xml,<svg/>" })).resolves.toBe(false);
    expect(clipboardWrite).not.toHaveBeenCalled();
  });

  it("转码失败返回 false，write 已同步发起但内部拒绝", async () => {
    createImageBitmapMock.mockRejectedValueOnce(new Error("decode failed"));
    const dataUrl = `data:image/webp;base64,${btoa("webp-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(false);
    // Promise 形式下 write 是同步发起的，失败体现在它内部的 rejected promise。
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
  });

  it("ClipboardItem 收到的是 Promise，写入不先等转码完成", async () => {
    const dataUrl = `data:image/webp;base64,${btoa("webp-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(true);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    const value = item.items["image/png"];
    expect(typeof (value as Promise<Blob>).then).toBe("function");
  });

  it("clipboard.write 被拒（手势/权限）返回 false，不抛异常", async () => {
    clipboardWrite.mockRejectedValueOnce(new Error("NotAllowedError"));
    const dataUrl = `data:image/png;base64,${btoa("png-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(false);
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
  });
});

describe("writeTransportClipboardText 的非安全上下文兜底", () => {
  it("navigator.clipboard 缺失时走 execCommand，并恢复原焦点", async () => {
    class FakeHTMLElement {
      focus = vi.fn();
    }
    const originalFocus = new FakeHTMLElement();
    const execCommand = vi.fn(() => true);
    const appendChild = vi.fn();
    const removeChild = vi.fn();
    const textarea = {
      value: "",
      style: {} as CSSStyleDeclaration,
      setAttribute: vi.fn(),
      focus: vi.fn(),
      select: vi.fn(),
    };
    const fakeDocument = {
      activeElement: originalFocus,
      createElement: vi.fn(() => textarea),
      body: { appendChild, removeChild },
      execCommand,
    };
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("HTMLElement", FakeHTMLElement);
    vi.stubGlobal("document", fakeDocument);

    await expect(writeTransportClipboardText("需要复制的文本")).resolves.toBe(true);
    expect(execCommand).toHaveBeenCalledWith("copy");
    expect(textarea.value).toBe("需要复制的文本");
    expect(appendChild).toHaveBeenCalledWith(textarea);
    expect(removeChild).toHaveBeenCalledWith(textarea);
    expect(originalFocus.focus).toHaveBeenCalledTimes(1);
  });

  it("execCommand 返回 false 时返回 false", async () => {
    class FakeHTMLElement {
      focus = vi.fn();
    }
    const textarea = {
      value: "",
      style: {} as CSSStyleDeclaration,
      setAttribute: vi.fn(),
      focus: vi.fn(),
      select: vi.fn(),
    };
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("HTMLElement", FakeHTMLElement);
    vi.stubGlobal("document", {
      activeElement: null,
      createElement: vi.fn(() => textarea),
      body: { appendChild: vi.fn(), removeChild: vi.fn() },
      execCommand: vi.fn(() => false),
    });

    await expect(writeTransportClipboardText("text")).resolves.toBe(false);
  });
});
