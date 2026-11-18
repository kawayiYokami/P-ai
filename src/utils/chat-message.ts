import type { ChatMessage } from "../types/app";

const MEDIA_REF_PREFIX = "@media:";
const DOWNLOAD_REF_PREFIX = "@download:";
const IMAGE_ATTACHMENT_EXTENSIONS = new Set([
  "avif",
  "bmp",
  "gif",
  "heic",
  "heif",
  "jpg",
  "jpeg",
  "png",
  "svg",
  "webp",
]);

function isStoredImageRef(value: string): boolean {
  return value.startsWith(MEDIA_REF_PREFIX) || value.startsWith(DOWNLOAD_REF_PREFIX);
}

function fileExtension(value: string): string {
  const name = String(value || "").trim().split(/[\\/]/).filter(Boolean).pop() || "";
  const index = name.lastIndexOf(".");
  return index >= 0 ? name.slice(index + 1).trim().toLowerCase() : "";
}

function isImageAttachmentFile(fileName: string, relativePath: string, mime?: string): boolean {
  const normalizedMime = String(mime || "").trim().toLowerCase();
  if (normalizedMime.startsWith("image/")) return true;
  return IMAGE_ATTACHMENT_EXTENSIONS.has(fileExtension(fileName || relativePath));
}

export function stripHiddenExtraBlocks(text: string): string {
  return (text || "")
    .replace(/<memory_board>[\s\S]*?<\/memory_board>/g, "")
    .replace(/\[MEMORY BOARD\][\s\S]*$/g, "")
    .trim();
}

export function messageText(msg: ChatMessage): string {
  const visible = msg.parts
    .filter((p) => p.type === "text")
    .map((p) => p.text)
    .join("\n");
  return stripHiddenExtraBlocks(visible);
}

export function extractMessageImages(
  msg?: ChatMessage,
): Array<{ mime: string; bytesBase64?: string; mediaRef?: string; name?: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "image" || (p.type === "attachment" && p.mime.toLowerCase().startsWith("image/")))
    .map((p) => {
      if (p.type === "attachment") {
        const path = String(p.path || "").trim();
        return {
          mime: String(p.mime || "image/webp"),
          mediaRef: path || undefined,
          name: String(p.name || "").trim() || undefined,
        };
      }
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      const raw = String(anyPart.bytesBase64 || anyPart.bytes_base64 || "").trim();
      const storedRef = isStoredImageRef(raw);
      return {
        mime: anyPart.mime || "image/webp",
        bytesBase64: raw && !storedRef ? raw : undefined,
        mediaRef: storedRef ? raw : undefined,
      };
    })
    .filter((p) => !!p.bytesBase64 || !!p.mediaRef);
}

export function extractMessageAudios(
  msg?: ChatMessage,
): Array<{ mime: string; bytesBase64?: string; mediaRef?: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "audio" || (p.type === "attachment" && p.mime.toLowerCase().startsWith("audio/")))
    .map((p) => {
      if (p.type === "attachment") {
        return {
          mime: p.mime || "audio/webm",
          mediaRef: String(p.path || "").trim() || undefined,
        };
      }
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      return {
        mime: anyPart.mime || "audio/webm",
        bytesBase64: anyPart.bytesBase64 || anyPart.bytes_base64 || "",
      };
    })
    .filter((p) => !!p.bytesBase64 || !!p.mediaRef);
}

export function extractMessageAttachmentFiles(
  msg?: ChatMessage,
): Array<{ fileName: string; path: string; mime?: string }> {
  if (!msg) return [];
  const out: Array<{ fileName: string; path: string; mime?: string }> = [];
  const seen = new Set<string>();
  const hasVisibleImages = extractMessageImages(msg).length > 0;
  for (const part of msg.parts) {
    if (part.type !== "attachment") continue;
    const path = String(part.path || "").trim().replace(/\\/g, "/");
    const mime = String(part.mime || "").trim();
    const normalizedMime = mime.toLowerCase();
    if (!path || normalizedMime.startsWith("image/") || normalizedMime.startsWith("audio/")) continue;
    const fileName = String(part.name || "").trim() || path.split("/").pop() || "attachment";
    const key = `${fileName}::${path}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ fileName, path, mime: mime || undefined });
  }
  const metaAttachments = Array.isArray((msg.providerMeta as { attachments?: unknown } | undefined)?.attachments)
    ? ((msg.providerMeta as { attachments?: Array<{ fileName?: unknown; relativePath?: unknown; mime?: unknown }> }).attachments || [])
    : [];
  for (const item of metaAttachments) {
    const fileName = String(item?.fileName || "").trim();
    const relativePath = String(item?.relativePath || "").trim().replace(/\\/g, "/");
    const mime = String(item?.mime || "").trim();
    if (!fileName || !relativePath) continue;
    if (hasVisibleImages && isImageAttachmentFile(fileName, relativePath, mime)) continue;
    const key = `${fileName}::${relativePath}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ fileName, path: relativePath, mime: mime || undefined });
  }
  if (!Array.isArray(msg.extraTextBlocks)) return out;
  for (const raw of msg.extraTextBlocks) {
    const text = String(raw || "").trim();
    if (!text) continue;
    const fileMatch = text.match(/用户本次上传了一个附件：([^\n\r]+)/);
    const pathMatch = text.match(/路径：([^\n\r）)]+)(?:）|\)|$)/);
    const fileName = String(fileMatch?.[1] || "").trim();
    const relativePath = String(pathMatch?.[1] || "").trim().replace(/\\/g, "/");
    if (!fileName || !relativePath) continue;
    if (hasVisibleImages && isImageAttachmentFile(fileName, relativePath)) continue;
    const key = `${fileName}::${relativePath}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ fileName, path: relativePath });
  }
  return out;
}

export function estimateTextTokens(text: string): number {
  let zh = 0;
  let other = 0;
  for (const ch of text || "") {
    if (/\s/.test(ch)) continue;
    if (/[\u3400-\u9fff\uf900-\ufaff]/.test(ch)) zh += 1;
    else other += 1;
  }
  return zh * 0.6 + other * 0.3;
}

export function estimateConversationTokens(messages: ChatMessage[]): number {
  let total = 0;
  for (const m of messages) {
    total += 12;
    for (const p of m.parts || []) {
      if (p.type === "text") total += estimateTextTokens((p as { text?: string }).text || "");
      else if (p.type === "image" || (p.type === "attachment" && p.mime.startsWith("image/"))) total += 280;
      else if (p.type === "audio" || (p.type === "attachment" && p.mime.startsWith("audio/"))) total += 320;
    }
  }
  return Math.ceil(total);
}
