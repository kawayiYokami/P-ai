import { createApp, h, nextTick } from "vue";
import { createI18n } from "vue-i18n";
import { domToPng } from "modern-screenshot";
import { i18n as appI18n } from "../../../i18n";
import { invokeTauri, readTransportChatImage } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";
import {
  projectMessageForDisplay,
  stripToolcallMarkers,
  TOOL_TEXT_BREAK_PLACEHOLDER,
} from "../../../utils/chat-message-semantics";
import ShareDocument from "../components/ShareDocument.vue";
import type { ShareDocumentEntry } from "../components/share-document-types";
import projectQrUrl from "../../../assets/pai-project-qr.png";
import appIconUrl from "../../../../src-tauri/icons/128x128.png";

const t = appI18n.global.t;

export const SHARE_PROJECT_URL = "https://github.com/kawayiYokami/P-ai";
export const SHARE_EXPORT_WIDTH = 760;

export type GenerateShareFromMessageIdsInput = {
  conversationId: string;
  messageIds: string[];
  formats: Array<"html" | "png">;
  title?: string;
  subtitle?: string;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  trigger?: string;
};

export type GenerateShareFromMessageIdsResult = {
  html?: string;
  pngDataUrl?: string;
  usedMessageIds: string[];
  skippedMessageIds: string[];
};

type ShareRenderModel = ShareDocumentEntry;

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

export function isDarkShareTheme(themeName: string, colorScheme: string): boolean {
  const theme = String(themeName || "").trim().toLowerCase();
  const scheme = String(colorScheme || "").trim().toLowerCase();
  return scheme.split(/\s+/).includes("dark")
    || ["dark", "dracula", "business", "dim", "crimson", "forest"].includes(theme);
}

function isDarkTheme(): boolean {
  const theme = String(document.documentElement.getAttribute("data-theme") || "").trim().toLowerCase();
  const colorScheme = getComputedStyle(document.documentElement).colorScheme;
  return isDarkShareTheme(theme, colorScheme);
}

function isOwnMessage(message: ChatMessage, speakerAgentId: string): boolean {
  if (String(message.role || "").trim() === "user") return true;
  return speakerAgentId === "user-persona";
}

function resolveDisplayName(
  message: ChatMessage,
  speakerAgentId: string,
  userAlias: string,
  personaNameMap: Record<string, string>,
  projection: ReturnType<typeof projectMessageForDisplay>,
): string {
  if (projection.remoteImOrigin) {
    return String(
      projection.remoteImOrigin.senderName
      || projection.remoteImOrigin.remoteContactName
      || t("chat.shareExport.contact"),
    ).trim();
  }
  if (isOwnMessage(message, speakerAgentId)) {
    return String(userAlias || t("chat.shareExport.user")).trim() || t("chat.shareExport.user");
  }
  const mapped = String(personaNameMap[speakerAgentId] || "").trim();
  if (mapped) return mapped;
  return speakerAgentId || String(message.role || "assistant").trim() || "assistant";
}

function resolveAvatarUrl(
  message: ChatMessage,
  speakerAgentId: string,
  userAvatarUrl: string,
  personaAvatarUrlMap: Record<string, string>,
  projection: ReturnType<typeof projectMessageForDisplay>,
): string {
  if (projection.remoteImOrigin) return "";
  if (isOwnMessage(message, speakerAgentId)) {
    return String(userAvatarUrl || "").trim();
  }
  return String(personaAvatarUrlMap[speakerAgentId] || "").trim();
}

function buildThinkingSummary(
  projection: ReturnType<typeof projectMessageForDisplay>,
): string {
  const count = Number(projection.activityReasoningCharCount || 0);
  if (!Number.isFinite(count) || count <= 0) return "";
  // 分享页不输出工具明细，只保留思考完成摘要
  return `思考完毕（${count}）`;
}

async function resolveShareImageSrc(image: {
  mime?: string;
  bytesBase64?: string;
  mediaRef?: string;
}): Promise<string> {
  const mime = String(image.mime || "").trim() || "image/png";
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  if (bytesBase64) return `data:${mime};base64,${bytesBase64}`;
  const mediaRef = String(image.mediaRef || "").trim();
  if (!mediaRef) return "";
  try {
    const legacyMarker = mediaRef.startsWith("@media:") || mediaRef.startsWith("@download:");
    const result = await readTransportChatImage({
      ...(legacyMarker ? { mediaRef } : { path: mediaRef }),
      mime,
      original: true,
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

export async function loadShareMessages(
  conversationId: string,
  messageIds: string[],
): Promise<{ messages: ChatMessage[]; usedMessageIds: string[]; skippedMessageIds: string[] }> {
  const conversation = String(conversationId || "").trim();
  const ids = messageIds.map((id) => String(id || "").trim()).filter(Boolean);
  if (!conversation) {
    throw new Error("conversationId is required");
  }
  if (ids.length === 0) {
    return { messages: [], usedMessageIds: [], skippedMessageIds: [] };
  }

  const loaded = await Promise.all(
    ids.map(async (messageId) => {
      try {
        const message = await invokeTauri<ChatMessage>("conversation.messageById", {
          input: {
            conversationId: conversation,
            messageId,
          },
        });
        return { messageId, message };
      } catch (error) {
        console.warn("[分享导出] 按消息 ID 读取失败，已跳过", {
          conversationId: conversation,
          messageId,
          error: String(error),
        });
        return { messageId, message: null as ChatMessage | null };
      }
    }),
  );

  const messages: ChatMessage[] = [];
  const usedMessageIds: string[] = [];
  const skippedMessageIds: string[] = [];
  for (const item of loaded) {
    if (!item.message) {
      skippedMessageIds.push(item.messageId);
      continue;
    }
    usedMessageIds.push(item.messageId);
    messages.push(item.message);
  }
  return { messages, usedMessageIds, skippedMessageIds };
}

export async function projectShareMessages(
  messages: ChatMessage[],
  options: {
    userAlias: string;
    userAvatarUrl: string;
    personaNameMap: Record<string, string>;
    personaAvatarUrlMap: Record<string, string>;
  },
): Promise<ShareRenderModel[]> {
  return await Promise.all(
    messages.map(async (message, index) => {
      const projection = projectMessageForDisplay(message);
      const speakerAgentId = String(projection.speakerAgentId || "").trim();
      const own = isOwnMessage(message, speakerAgentId);
      const text = stripToolcallMarkers(
        (projection.text || "").split(TOOL_TEXT_BREAK_PLACEHOLDER).join("\n\n"),
      );
      const images = await Promise.all(
        (projection.images || []).map(async (image, imageIndex) => {
          const src = await resolveShareImageSrc(image);
          if (!src) return null;
          return {
            src,
            alt: `image-${index + 1}-${imageIndex + 1}`,
          };
        }),
      );
      return {
        id: String(message.id || `share-${index}`).trim() || `share-${index}`,
        tone: own ? "user" : "assistant",
        displayName: resolveDisplayName(
          message,
          speakerAgentId,
          options.userAlias,
          options.personaNameMap,
          projection,
        ),
        avatarUrl: resolveAvatarUrl(
          message,
          speakerAgentId,
          options.userAvatarUrl,
          options.personaAvatarUrlMap,
          projection,
        ),
        createdAtText: formatShareTime(message.createdAt),
        text,
        thinkingSummary: buildThinkingSummary(projection),
        images: images.filter((item): item is { src: string; alt: string } => !!item?.src),
        attachmentNames: (projection.attachmentFiles || [])
          .map((item) => String(item?.fileName || "").trim())
          .filter(Boolean),
        audioCount: Array.isArray(projection.audios) ? projection.audios.length : 0,
      } satisfies ShareRenderModel;
    }),
  );
}

async function waitForImages(root: HTMLElement): Promise<void> {
  const images = Array.from(root.querySelectorAll("img"));
  if (images.length === 0) return;
  await Promise.all(
    images.map(
      (image) => new Promise<void>((resolve) => {
        if (image.complete && image.naturalWidth > 0) {
          resolve();
          return;
        }
        const done = () => resolve();
        image.addEventListener("load", done, { once: true });
        image.addEventListener("error", done, { once: true });
      }),
    ),
  );
}

async function resolveQrDataUrl(): Promise<string> {
  return await resolveAssetDataUrl(String(projectQrUrl || "").trim());
}

async function resolveBrandIconDataUrl(): Promise<string> {
  return await resolveAssetDataUrl(String(appIconUrl || "").trim());
}

async function resolveAssetDataUrl(raw: string): Promise<string> {
  if (!raw) return "";
  if (raw.startsWith("data:")) return raw;
  try {
    const response = await fetch(raw);
    if (!response.ok) return raw;
    const blob = await response.blob();
    return await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result || ""));
      reader.onerror = () => reject(new Error("qr read failed"));
      reader.readAsDataURL(blob);
    });
  } catch {
    return raw;
  }
}

function collectStyleTags(): string {
  return Array.from(document.querySelectorAll("style"))
    .map((style) => style.outerHTML)
    .join("\n");
}

function collectStylesheetLinks(): string {
  return Array.from(document.querySelectorAll('link[rel="stylesheet"]'))
    .map((link) => {
      const href = String((link as HTMLLinkElement).href || "").trim();
      if (!href) return "";
      return `<link rel="stylesheet" href="${href.replace(/"/g, "&quot;")}">`;
    })
    .filter(Boolean)
    .join("\n");
}

function snapshotThemeAttributes(): string {
  const theme = String(document.documentElement.getAttribute("data-theme") || "light").trim() || "light";
  return `data-theme="${theme.replace(/"/g, "&quot;")}"`;
}

function buildStandaloneHtml(documentHtml: string, title: string): string {
  const themeAttr = snapshotThemeAttributes();
  const styles = collectStyleTags();
  const links = collectStylesheetLinks();
  return `<!doctype html>
<html lang="zh-CN" ${themeAttr}>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(title)}</title>
    ${links}
    ${styles}
    <style>
      html, body {
        margin: 0;
        padding: 0;
        background: var(--color-base-200, #f3f4f6);
        width: 100%;
        min-height: 100%;
        height: auto !important;
        overflow-x: hidden !important;
        overflow-y: auto !important;
        position: static !important;
      }
      body {
        display: block !important;
      }
    </style>
  </head>
  <body>
    ${documentHtml}
  </body>
</html>`;
}

function escapeHtml(value: string): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * modern-screenshot 会把元素的计算宽度内联成像素值。用户气泡是 fit-content，
 * 冻结下来的宽度等于页面测量值、没有余量；导出时文本在 SVG 排版上下文里重排，
 * 字形步进与页面存在亚像素差异（随 DPI 变化），一旦多出零点几像素就会重新折行。
 * 这里在渲染前给用户气泡补一点余量。
 */
const SHARE_USER_BUBBLE_WIDTH_MARGIN_PX = 2;

function applyShareUserBubbleWidthMargin(host: HTMLElement): void {
  const surfaces = Array.from(
    host.querySelectorAll<HTMLElement>(".ecall-chat-bubble-tone-user .ecall-chat-bubble-surface"),
  );
  let applied = 0;
  for (const surface of surfaces) {
    const measured = Number.parseFloat(getComputedStyle(surface).width);
    if (!Number.isFinite(measured) || measured <= 0) continue;
    surface.style.minWidth = `${measured + SHARE_USER_BUBBLE_WIDTH_MARGIN_PX}px`;
    applied += 1;
  }
  if (applied > 0) {
    console.info("[分享导出] 用户气泡已补宽度余量", {
      task: "applyShareUserBubbleWidthMargin",
      count: applied,
      marginPx: SHARE_USER_BUBBLE_WIDTH_MARGIN_PX,
    });
  }
}

async function renderMountedShareToPng(host: HTMLElement): Promise<string> {
  await waitForImages(host);
  applyShareUserBubbleWidthMargin(host);
  const page = host.querySelector("[data-share-document='1']") as HTMLElement | null;
  const target = page || host;
  const width = Math.max(SHARE_EXPORT_WIDTH, Math.ceil(target.scrollWidth || SHARE_EXPORT_WIDTH));
  const height = Math.max(200, Math.ceil(target.scrollHeight || 200));
  console.info("[分享导出] 开始生成 PNG", {
    renderer: "modern-screenshot",
    width,
    height,
    scrollWidth: target.scrollWidth,
    scrollHeight: target.scrollHeight,
  });
  const backgroundColor = getComputedStyle(target).backgroundColor
    || getComputedStyle(document.body).backgroundColor
    || "#f3f4f6";
  const dataUrl = await domToPng(target, {
    backgroundColor,
    width,
    height,
    scale: Math.min(Math.max(window.devicePixelRatio || 1, 1), 2),
    timeout: 30000,
    fetch: {
      requestInit: {
        cache: "force-cache",
      },
    },
    style: {
      width: `${width}px`,
      minHeight: `${height}px`,
      transform: "none",
    },
  });
  console.info("[分享导出] PNG 生成完成", {
    renderer: "modern-screenshot",
    width,
    height,
    dataUrlLength: dataUrl.length,
  });
  return dataUrl;
}

async function mountShareDocument(options: {
  title: string;
  subtitle: string;
  exportAtText: string;
  projectUrl: string;
  qrDataUrl: string;
  brandIconUrl: string;
  isDark: boolean;
  entries: ShareRenderModel[];
}): Promise<{ host: HTMLElement; app: ReturnType<typeof createApp>; html: string }> {
  const host = document.createElement("div");
  host.setAttribute(
    "style",
    [
      "position:absolute",
      "left:0",
      "top:0",
      `width:${SHARE_EXPORT_WIDTH}px`,
      "pointer-events:none",
      "z-index:-1",
      "transform:translateY(-200vh)",
    ].join(";"),
  );
  document.body.appendChild(host);

  // ShareDocument / AppMarkdownRenderer 依赖 vue-i18n；离屏挂载时提供最小实例
  const localI18n = createI18n({
    legacy: false,
    locale: String(appI18n.global.locale.value || "zh-CN"),
    messages: appI18n.global.messages.value as never,
  });

  const app = createApp({
    render: () => h(ShareDocument, {
      title: options.title,
      subtitle: options.subtitle,
      exportAtText: options.exportAtText,
      projectUrl: options.projectUrl,
      qrDataUrl: options.qrDataUrl,
      brandIconUrl: options.brandIconUrl,
      isDark: options.isDark,
      entries: options.entries,
      width: SHARE_EXPORT_WIDTH,
    }),
  });
  app.use(localI18n);
  app.mount(host);

  await nextTick();
  await waitForImages(host);
  // 再等一帧，让 markdown / 头像布局稳定
  await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  await new Promise((resolve) => setTimeout(resolve, 50));

  const page = host.querySelector("[data-share-document='1']") as HTMLElement | null;
  const documentHtml = page ? page.outerHTML : host.innerHTML;
  const html = buildStandaloneHtml(documentHtml, options.title);
  return { host, app, html };
}

export async function generateShareFromMessageIds(
  input: GenerateShareFromMessageIdsInput,
): Promise<GenerateShareFromMessageIdsResult> {
  const startedAt = performance.now();
  const conversationId = String(input.conversationId || "").trim();
  const messageIds = (input.messageIds || []).map((id) => String(id || "").trim()).filter(Boolean);
  const formats = new Set((input.formats || []).map((item) => String(item || "").trim()));
  if (!conversationId) throw new Error("conversationId is required");
  if (messageIds.length === 0) throw new Error("messageIds is required");

  const { messages, usedMessageIds, skippedMessageIds } = await loadShareMessages(
    conversationId,
    messageIds,
  );
  if (messages.length === 0) {
    throw new Error(`没有可用消息可导出: skipped=${skippedMessageIds.join(",")}`);
  }

  const entries = await projectShareMessages(messages, {
    userAlias: input.userAlias,
    userAvatarUrl: input.userAvatarUrl,
    personaNameMap: input.personaNameMap,
    personaAvatarUrlMap: input.personaAvatarUrlMap,
  });

  const title = String(input.title || t("chat.shareDocumentTitle")).trim() || "P-ai 对话分享";
  const subtitle = String(
    input.subtitle || t("chat.shareDocumentSubtitle", { count: entries.length }),
  ).trim();
  const exportAtText = formatShareTime(new Date().toISOString());
  const qrDataUrl = await resolveQrDataUrl();
  const brandIconUrl = await resolveBrandIconDataUrl();
  const isDark = isDarkTheme();

  const mounted = await mountShareDocument({
    title,
    subtitle,
    exportAtText,
    projectUrl: SHARE_PROJECT_URL,
    qrDataUrl,
    brandIconUrl,
    isDark,
    entries,
  });

  try {
    const result: GenerateShareFromMessageIdsResult = {
      usedMessageIds,
      skippedMessageIds,
    };
    if (formats.has("html")) {
      result.html = mounted.html;
    }
    if (formats.has("png")) {
      result.pngDataUrl = await renderMountedShareToPng(mounted.host);
    }
    console.info("[分享导出] generateShareFromMessageIds 完成", {
      trigger: input.trigger || "unknown",
      conversationId,
      usedCount: usedMessageIds.length,
      skippedCount: skippedMessageIds.length,
      formats: Array.from(formats),
      durationMs: Math.round(performance.now() - startedAt),
    });
    return result;
  } finally {
    mounted.app.unmount();
    mounted.host.remove();
  }
}

export function buildShareExportFileName(kind: "html" | "png"): string {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return kind === "html"
    ? `p-ai-share-${stamp}.html`
    : `p-ai-share-${stamp}.png`;
}
