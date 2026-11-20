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
} from "./tauri-api";

const clipboardWrite = vi.fn(async (_items: unknown[]) => {});
const createImageBitmapMock = vi.fn(async () => ({ width: 2, height: 2 }));
const convertToBlobMock = vi.fn(async () => new Blob(["png"], { type: "image/png" }));

class FakeClipboardItem {
  constructor(public readonly items: Record<string, Blob>) {}
}

beforeEach(() => {
  clipboardWrite.mockClear();
  createImageBitmapMock.mockClear();
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
    });
  });

  it("解析 png data URL", () => {
    expect(parseClipboardImageDataUrl("data:image/png;base64,QUJD")).toEqual({
      mime: "image/png",
      base64: "QUJD",
    });
  });

  it("吃掉中间参数段（charset）", () => {
    expect(parseClipboardImageDataUrl("data:image/png;charset=utf-8;base64,QUJD")).toEqual({
      mime: "image/png",
      base64: "QUJD",
    });
  });

  it("无 base64 段时抛明确错误，不误报成解码失败", () => {
    expect(() => parseClipboardImageDataUrl("data:image/svg+xml,<svg/>")).toThrow(
      /缺少 ;base64 段/,
    );
  });

  it("裸 base64 按传入 mime 处理", () => {
    expect(parseClipboardImageDataUrl("QUJD", "image/webp")).toEqual({
      mime: "image/webp",
      base64: "QUJD",
    });
  });
});

describe("writeTransportClipboardImage 的 Web 写入路径", () => {
  it("png 直写，不经过 createImageBitmap", async () => {
    const dataUrl = `data:image/png;base64,${btoa("png-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(true);
    expect(createImageBitmapMock).not.toHaveBeenCalled();
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    expect(item.items).toHaveProperty("image/png");
    expect(item.items["image/png"].type).toBe("image/png");
  });

  it("webp 先转 png 再写", async () => {
    const dataUrl = `data:image/webp;base64,${btoa("webp-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(true);
    expect(createImageBitmapMock).toHaveBeenCalledTimes(1);
    expect(convertToBlobMock).toHaveBeenCalledWith({ type: "image/png" });
    expect(clipboardWrite).toHaveBeenCalledTimes(1);
    const item = clipboardWrite.mock.calls[0][0][0] as FakeClipboardItem;
    expect(item.items).toHaveProperty("image/png");
    expect(item.items["image/png"].type).toBe("image/png");
  });

  it("转码失败返回 false，不写剪贴板", async () => {
    createImageBitmapMock.mockRejectedValueOnce(new Error("decode failed"));
    const dataUrl = `data:image/webp;base64,${btoa("webp-bytes")}`;
    await expect(writeTransportClipboardImage({ base64: dataUrl })).resolves.toBe(false);
    expect(clipboardWrite).not.toHaveBeenCalled();
  });
});
