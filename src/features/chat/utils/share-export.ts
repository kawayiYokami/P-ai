import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessageBlock } from "../../../types/app";

export type ShareRenderableEntry = {
  id: string;
  align: "left" | "right";
  displayName: string;
  avatarSrc: string;
  createdAtText: string;
  text: string;
  reasoningText: string;
  toolCalls: Array<{ name: string; argsText: string; status?: string }>;
  images: Array<{ src: string; alt: string }>;
  attachmentNames: string[];
  audioCount: number;
  remoteContactLabel: string;
};

type PrepareShareEntriesOptions = {
  blocks: ChatMessageBlock[];
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  trigger?: string;
};

type BuildShareDocumentOptions = {
  title: string;
  subtitle?: string;
  entries: ShareRenderableEntry[];
};

type ShareThemeSnapshot = {
  pageBackground: string;
  fontFamily: string;
  base100: string;
  base200: string;
  base300: string;
  baseContent: string;
  primary: string;
  primaryContent: string;
  neutral: string;
  neutralContent: string;
  success: string;
  successContent: string;
};

const SHARE_EXPORT_WIDTH = 760;

export async function prepareShareEntries(
  options: PrepareShareEntriesOptions,
): Promise<ShareRenderableEntry[]> {
  const startedAt = performance.now();
  const entries = await Promise.all(
    options.blocks.map(async (block, index): Promise<ShareRenderableEntry> => ({
      id: String(block.id || block.sourceMessageId || `share-${index}`).trim() || `share-${index}`,
      align: isOwnShareBlock(block) ? "right" : "left",
      displayName: shareDisplayName(block, options.userAlias, options.personaNameMap),
      avatarSrc: shareAvatarSrc(block, options.userAvatarUrl, options.personaAvatarUrlMap),
      createdAtText: formatShareTime(block.createdAt),
      text: String(block.text || "").trim(),
      reasoningText: normalizeReasoningText(block),
      toolCalls: Array.isArray(block.toolCalls)
        ? block.toolCalls.map((call) => ({
          name: String(call?.name || "").trim(),
          argsText: String(call?.argsText || "").trim(),
          status: String(call?.status || "").trim() || undefined,
        })).filter((call) => !!call.name || !!call.argsText)
        : [],
      images: await Promise.allSettled(
        (Array.isArray(block.images) ? block.images : []).map(async (image, imageIndex) => {
          try {
            const src = await resolveShareImageSrc(image);
            if (!src) return null;
            return {
              src,
              alt: `image-${index + 1}-${imageIndex + 1}`,
            };
          } catch (error) {
            console.warn("[分享导出] 图片资源解析失败，已跳过", {
              fn: "prepareShareEntries",
              trigger: options.trigger || "unknown",
              blockId: block.id || block.sourceMessageId || `share-${index}`,
              imageIndex,
              error: String(error),
            });
            return null;
          }
        }),
      ).then((results) => results
        .map((result) => (result.status === "fulfilled" ? result.value : null))
        .filter((item): item is { src: string; alt: string } => !!item?.src)),
      attachmentNames: Array.isArray(block.attachmentFiles)
        ? block.attachmentFiles
          .map((item) => String(item?.fileName || "").trim())
          .filter((item) => !!item)
        : [],
      audioCount: Array.isArray(block.audios) ? block.audios.length : 0,
      remoteContactLabel: block.remoteImOrigin
        ? String(block.remoteImOrigin.senderName || block.remoteImOrigin.remoteContactName || "").trim()
        : "",
    })),
  );
  console.info("[分享导出] 消息条目准备完成", {
    task: "prepareShareEntries",
    trigger: options.trigger || "unknown",
    inputCount: options.blocks.length,
    outputCount: entries.length,
    durationMs: Math.round(performance.now() - startedAt),
  });
  return entries;
}

export function buildShareHtmlDocument(options: BuildShareDocumentOptions): string {
  const theme = readShareThemeSnapshot();
  return `<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(options.title)}</title>
    <style>${shareDocumentCss(theme)}</style>
  </head>
  <body>
    ${buildShareBodyHtml(options)}
  </body>
</html>`;
}

export async function renderShareDocumentToPngDataUrl(
  options: BuildShareDocumentOptions,
): Promise<string> {
  const theme = readShareThemeSnapshot();
  const host = document.createElement("div");
  host.setAttribute(
    "style",
    `position:fixed;left:-100000px;top:0;width:${SHARE_EXPORT_WIDTH}px;pointer-events:none;z-index:-1;`,
  );
  host.innerHTML = `<style>${shareDocumentCss(theme)}</style>${buildShareBodyHtml(options)}`;
  document.body.appendChild(host);

  try {
    let page: HTMLElement | null = null;
    let width = SHARE_EXPORT_WIDTH;
    let height = 200;
    let pixelRatio = 1;
    let svgUrl = "";
    try {
      await waitForImages(host);
      page = host.querySelector(".pai-share-page") as HTMLElement | null;
      if (!page) {
        throw new Error("分享预览渲染失败：未找到渲染页面节点");
      }
      width = Math.max(SHARE_EXPORT_WIDTH, Math.ceil(page.scrollWidth));
      height = Math.max(200, Math.ceil(page.scrollHeight));
      const svg = buildShareSvg({
        width,
        height,
        bodyHtml: buildShareBodyHtml(options),
        cssText: shareDocumentCss(theme),
      });
      const svgBlob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
      svgUrl = URL.createObjectURL(svgBlob);
      const image = await loadImage(svgUrl);
      pixelRatio = Math.min(Math.max(window.devicePixelRatio || 1, 1), 2);
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.round(width * pixelRatio));
      canvas.height = Math.max(1, Math.round(height * pixelRatio));
      const context = canvas.getContext("2d");
      if (!context) {
        throw new Error("创建分享画布失败：无法获取 2D 绘图上下文");
      }
      context.scale(pixelRatio, pixelRatio);
      context.fillStyle = theme.pageBackground;
      context.fillRect(0, 0, width, height);
      context.drawImage(image, 0, 0, width, height);
      return canvas.toDataURL("image/png");
    } catch (error) {
      console.warn("[分享导出] 图片渲染失败", {
        fn: "renderShareDocumentToPngDataUrl",
        pageFound: !!page,
        width,
        height,
        pixelRatio,
        title: options.title,
        subtitle: options.subtitle || "",
        entryCount: options.entries.length,
        error: String(error),
      });
      throw error instanceof Error
        ? new Error(`${error.message} (fn=renderShareDocumentToPngDataUrl width=${width} height=${height} pixelRatio=${pixelRatio} entries=${options.entries.length})`)
        : new Error(`分享图片导出失败 (fn=renderShareDocumentToPngDataUrl width=${width} height=${height} pixelRatio=${pixelRatio} entries=${options.entries.length} error=${String(error)})`);
    } finally {
      if (svgUrl) {
        URL.revokeObjectURL(svgUrl);
      }
    }
  } finally {
    host.remove();
  }
}

export function buildShareExportFileName(kind: "html" | "png"): string {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return kind === "html"
    ? `p-ai-share-${stamp}.html`
    : `p-ai-share-${stamp}.png`;
}

function buildShareBodyHtml(options: BuildShareDocumentOptions): string {
  const subtitle = String(options.subtitle || "").trim();
  return `<main class="pai-share-page">
    <header class="pai-share-header">
      <div class="pai-share-title">${escapeHtml(options.title)}</div>
      ${subtitle ? `<div class="pai-share-subtitle">${escapeHtml(subtitle)}</div>` : ""}
    </header>
    <section class="card bg-base-100 card-sm overflow-hidden pai-share-shell">
      <div class="card-body pai-share-list">
      ${options.entries.map(renderShareEntryHtml).join("")}
      </div>
    </section>
  </main>`;
}

function renderShareEntryHtml(entry: ShareRenderableEntry): string {
  const reasoningHtml = entry.reasoningText
    ? `<div class="pai-share-extra pai-share-reasoning"><div class="pai-share-extra-label">思维链</div><div class="pai-share-extra-body">${renderTextHtml(entry.reasoningText)}</div></div>`
    : "";
  const toolsHtml = entry.toolCalls.length > 0
    ? `<div class="pai-share-extra pai-share-tools"><div class="pai-share-extra-label">工具</div><div class="pai-share-tool-summary">${escapeHtml(summarizeShareToolCalls(entry.toolCalls))}</div></div>`
    : "";
  const textHtml = entry.text
    ? `<div class="pai-share-text">${renderTextHtml(entry.text)}</div>`
    : "";
  const imageHtml = entry.images.length > 0
    ? `<div class="pai-share-images">${entry.images.map((image) => `<img class="pai-share-image" src="${escapeHtmlAttribute(image.src)}" alt="${escapeHtmlAttribute(image.alt)}" />`).join("")}</div>`
    : "";
  const filesHtml = entry.attachmentNames.length > 0 || entry.audioCount > 0
    ? `<div class="pai-share-meta-row">${entry.attachmentNames.map((name) => `<span class="pai-share-chip">附件 · ${escapeHtml(name)}</span>`).join("")}${entry.audioCount > 0 ? `<span class="pai-share-chip">语音 × ${entry.audioCount}</span>` : ""}</div>`
    : "";
  const remoteLabel = entry.remoteContactLabel
    ? escapeHtml(entry.remoteContactLabel)
    : "";
  const avatarInnerHtml = entry.avatarSrc
    ? `<img class="pai-share-avatar-image" src="${escapeHtmlAttribute(entry.avatarSrc)}" alt="${escapeHtmlAttribute(entry.displayName)}" />`
    : escapeHtml(shareAvatarText(entry.displayName));
  const avatarHtml = `<div class="chat-image avatar pai-share-avatar-col">
      <div class="w-8 rounded-full pai-share-avatar">${avatarInnerHtml}</div>
    </div>
  `;
  const mainHtml = `<div class="pai-share-main">
    <div class="chat-header pai-share-entry-header">
      <span class="pai-share-display-name">${escapeHtml(entry.displayName)}</span>
      <time class="pai-share-time">${escapeHtml(entry.createdAtText)}</time>
    </div>
    ${remoteLabel ? `<div class="chat-footer pai-share-remote-label">${remoteLabel}</div>` : ""}
    <div class="chat-bubble pai-share-bubble pai-share-bubble-${entry.align}">
      ${reasoningHtml}
      ${toolsHtml}
      ${textHtml}
      ${imageHtml}
      ${filesHtml}
    </div>
    </div>
  `;
  return `<article class="chat pai-share-chat chat-${entry.align === "right" ? "end" : "start"}">
    ${avatarHtml}${mainHtml}
  </article>`;
}

function shareDocumentCss(theme: ShareThemeSnapshot): string {
  return `
    :root {
      --color-base-100: ${theme.base100};
      --color-base-200: ${theme.base200};
      --color-base-300: ${theme.base300};
      --color-base-content: ${theme.baseContent};
      --color-primary: ${theme.primary};
      --color-primary-content: ${theme.primaryContent};
      --color-neutral: ${theme.neutral};
      --color-neutral-content: ${theme.neutralContent};
      --color-success: ${theme.success};
      --color-success-content: ${theme.successContent};
    }
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      background: ${theme.pageBackground};
      color: var(--color-base-content);
      font-family: ${theme.fontFamily};
    }
    .pai-share-page {
      width: ${SHARE_EXPORT_WIDTH}px;
      margin: 0 auto;
      padding: 18px 14px 24px;
      background: ${theme.pageBackground};
    }
    .pai-share-header {
      margin-bottom: 10px;
      padding: 0 6px;
    }
    .pai-share-title {
      font-size: 14px;
      font-weight: 700;
      line-height: 1.5;
      opacity: 0.9;
    }
    .pai-share-subtitle {
      margin-top: 2px;
      font-size: 11px;
      line-height: 1.5;
      opacity: 0.6;
    }
    .pai-share-shell {
      background: var(--color-base-100);
      border-radius: 16px;
      overflow: hidden;
    }
    .pai-share-list {
      display: flex;
      flex-direction: column;
      gap: 14px;
      padding: 14px;
    }
    .pai-share-chat {
      display: flex;
      align-items: flex-start;
      gap: 10px;
    }
    .chat-start {
      flex-direction: row;
    }
    .chat-end {
      flex-direction: row-reverse;
    }
    .chat-image,
    .pai-share-avatar-col {
      display: flex;
      align-items: flex-start;
      justify-content: center;
      flex: 0 0 2rem;
    }
    .avatar > div {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 2rem;
      height: 2rem;
      border-radius: 9999px;
    }
    .pai-share-avatar {
      background: var(--color-base-200);
      color: var(--color-base-content);
      font-size: 12px;
      font-weight: 700;
      border: 1px solid var(--color-base-300);
      overflow: hidden;
    }
    .pai-share-avatar-image {
      display: block;
      width: 100%;
      height: 100%;
      object-fit: cover;
    }
    .pai-share-main {
      display: flex;
      flex-direction: column;
      gap: 4px;
      min-width: 0;
      flex: 1 1 auto;
    }
    .chat-end .pai-share-main {
      align-items: flex-end;
    }
    .chat-start .pai-share-main {
      align-items: flex-start;
    }
    .pai-share-entry-header {
      display: flex;
      align-items: baseline;
      gap: 8px;
      font-size: 12px;
      line-height: 1.4;
      padding: 0 2px;
    }
    .pai-share-display-name {
      font-weight: 600;
      opacity: 0.8;
    }
    .pai-share-time {
      white-space: nowrap;
      opacity: 0.55;
    }
    .pai-share-remote-label {
      font-size: 11px;
      opacity: 0.55;
    }
    .pai-share-bubble {
      max-width: 88%;
      padding: 12px 14px;
      border-radius: 16px;
      border: 1px solid var(--color-base-300);
      white-space: normal;
      background: var(--color-base-100);
    }
    .pai-share-bubble-left {
      background: var(--color-base-100);
    }
    .pai-share-bubble-right {
      background: var(--color-base-200);
    }
    .pai-share-text,
    .pai-share-extra-body,
    .pai-share-tool-summary {
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
      line-height: 1.7;
      font-size: 14px;
    }
    .pai-share-extra {
      margin-bottom: 10px;
      padding: 8px 10px;
      border-radius: 12px;
      background: var(--color-base-200);
      border: 1px solid var(--color-base-300);
    }
    .pai-share-extra-label {
      margin-bottom: 4px;
      font-size: 11px;
      font-weight: 700;
      opacity: 0.6;
    }
    .pai-share-images {
      margin-top: 10px;
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }
    .pai-share-image {
      max-width: 220px;
      max-height: 220px;
      border-radius: 14px;
      display: block;
      background: var(--color-base-200);
      object-fit: contain;
      border: 1px solid var(--color-base-300);
    }
    .pai-share-meta-row {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin-top: 10px;
    }
    .pai-share-chip {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      border-radius: 999px;
      padding: 4px 10px;
      background: var(--color-base-200);
      border: 1px solid var(--color-base-300);
      font-size: 11px;
      opacity: 0.76;
    }
  `;
}

function buildShareSvg(options: {
  width: number;
  height: number;
  bodyHtml: string;
  cssText: string;
}): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${options.width}" height="${options.height}" viewBox="0 0 ${options.width} ${options.height}">
  <foreignObject width="100%" height="100%">
    <div xmlns="http://www.w3.org/1999/xhtml">
      <style>${options.cssText}</style>
      ${options.bodyHtml}
    </div>
  </foreignObject>
</svg>`;
}

async function resolveShareImageSrc(image: {
  mime: string;
  bytesBase64?: string;
  mediaRef?: string;
}): Promise<string> {
  const mime = String(image.mime || "").trim() || "image/png";
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  if (bytesBase64) {
    return `data:${mime};base64,${bytesBase64}`;
  }
  const mediaRef = String(image.mediaRef || "").trim();
  if (!mediaRef) return "";
  try {
    const result = await invokeTauri<{ dataUrl: string }>("read_chat_image_data_url", {
      input: {
        mediaRef,
        mime,
      },
    });
    return String(result?.dataUrl || "").trim();
  } catch (error) {
    console.warn("[分享导出] 读取图片数据失败，已跳过", {
      fn: "resolveShareImageSrc",
      mediaRef,
      mime,
      error: String(error),
    });
    return "";
  }
}

function shareDisplayName(
  block: ChatMessageBlock,
  userAlias: string,
  personaNameMap: Record<string, string>,
): string {
  if (block.remoteImOrigin) {
    return String(
      block.remoteImOrigin.senderName
      || block.remoteImOrigin.remoteContactName
      || "联系人",
    ).trim();
  }
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  if (!speakerAgentId || speakerAgentId === "user-persona" || block.role === "user") {
    return String(userAlias || "用户").trim() || "用户";
  }
  const mapped = String(personaNameMap[speakerAgentId] || "").trim();
  if (mapped) return mapped;
  return speakerAgentId || String(block.role || "assistant").trim() || "assistant";
}

function isOwnShareBlock(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  if (block.role === "user") return true;
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  return speakerAgentId === "user-persona";
}

function normalizeReasoningText(block: ChatMessageBlock): string {
  const standard = String(block.reasoningStandard || "").trim();
  if (standard) return standard;
  return String(block.reasoningInline || "").trim();
}

function formatShareTime(input?: string): string {
  const raw = String(input || "").trim();
  if (!raw) return "";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;
  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function renderTextHtml(text: string): string {
  return escapeHtml(text).replace(/\n/g, "<br/>");
}

function summarizeShareToolCalls(toolCalls: ShareRenderableEntry["toolCalls"]): string {
  const names = toolCalls
    .map((toolCall) => String(toolCall.name || "").trim())
    .filter(Boolean);
  if (names.length === 0) return "";
  const first = names[0];
  const extra = names.length - 1;
  return extra > 0 ? `调用了 ${first}（+${extra}）` : `调用了 ${first}`;
}

function shareAvatarText(displayName: string): string {
  const text = String(displayName || "").trim();
  if (!text) return "?";
  return Array.from(text)[0] || "?";
}

function shareAvatarSrc(
  block: ChatMessageBlock,
  userAvatarUrl: string,
  personaAvatarUrlMap: Record<string, string>,
): string {
  if (block.remoteImOrigin) return "";
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  if (!speakerAgentId || speakerAgentId === "user-persona" || block.role === "user") {
    return String(userAvatarUrl || "").trim();
  }
  return String(personaAvatarUrlMap[speakerAgentId] || "").trim();
}

function readShareThemeSnapshot(): ShareThemeSnapshot {
  const rootStyle = getComputedStyle(document.documentElement);
  const bodyStyle = getComputedStyle(document.body);
  const readVar = (name: string, fallback: string) => {
    const value = rootStyle.getPropertyValue(name).trim();
    return value || fallback;
  };
  return {
    pageBackground: bodyStyle.backgroundColor.trim() || "#f3f4f6",
    fontFamily: bodyStyle.fontFamily.trim() || "\"Segoe UI\", \"Microsoft YaHei\", sans-serif",
    base100: readVar("--color-base-100", "#ffffff"),
    base200: readVar("--color-base-200", "#f3f4f6"),
    base300: readVar("--color-base-300", "#d1d5db"),
    baseContent: readVar("--color-base-content", "#111827"),
    primary: readVar("--color-primary", "#2563eb"),
    primaryContent: readVar("--color-primary-content", "#ffffff"),
    neutral: readVar("--color-neutral", "#374151"),
    neutralContent: readVar("--color-neutral-content", "#ffffff"),
    success: readVar("--color-success", "#16a34a"),
    successContent: readVar("--color-success-content", "#ffffff"),
  };
}

function escapeHtml(value: string): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeHtmlAttribute(value: string): string {
  return escapeHtml(value).replace(/\n/g, " ");
}

async function waitForImages(root: HTMLElement): Promise<void> {
  const images = Array.from(root.querySelectorAll("img"));
  if (images.length === 0) return;
  await Promise.all(
    images.map((image) => new Promise<void>((resolve, reject) => {
      if (image.complete && image.naturalWidth > 0) {
        resolve();
        return;
      }
      image.onload = () => resolve();
      image.onerror = () => reject(new Error(`图片加载失败: ${image.currentSrc || image.src || "unknown"}`));
    })),
  );
}

async function loadImage(url: string): Promise<HTMLImageElement> {
  return await new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("分享图片渲染失败"));
    image.src = url;
  });
}
