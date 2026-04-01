import type { ChatMessage } from "../types/app";

const MEDIA_REF_PREFIX = "@media:";

// Internal parsing marker for reasoning block - DO NOT translate or change this value
// This is a protocol marker used for text parsing, not a user-facing UI string
const STANDARD_REASONING_MARKER = "[标准思考]";

export function parseAssistantStoredText(rawText: string): {
  assistantText: string;
  reasoningStandard: string;
  reasoningInline: string;
} {
  const raw = rawText || "";
  const standardIdx = raw.indexOf(STANDARD_REASONING_MARKER);

  if (standardIdx < 0) {
    return {
      assistantText: raw.trim(),
      reasoningStandard: "",
      reasoningInline: "",
    };
  }

  const reasoningStandard = raw.slice(standardIdx + STANDARD_REASONING_MARKER.length).trim();

  return {
    assistantText: raw.slice(0, standardIdx).trim(),
    reasoningStandard,
    reasoningInline: "",
  };
}

export function stripHiddenExtraBlocks(text: string): string {
  return (text || "")
    .replace(/<memory_board>[\s\S]*?<\/memory_board>/g, "")
    .replace(/\[MEMORY BOARD\][\s\S]*$/g, "")
    .trim();
}

export function renderMessage(msg: ChatMessage): string {
  const merged = msg.parts
    .map((p) => {
      if (p.type === "text") return p.text;
      if (p.type === "image") {
        const mime = String((p as { mime?: string }).mime || "").trim().toLowerCase();
        return mime === "application/pdf" ? "[pdf]" : "[image]";
      }
      return "[audio]";
    })
    .join("\n");
  return stripHiddenExtraBlocks(merged);
}

export function messageText(msg: ChatMessage): string {
  const visible = msg.parts
    .filter((p) => p.type === "text")
    .map((p) => p.text)
    .join("\n");
  return stripHiddenExtraBlocks(visible);
}

export function removeBinaryPlaceholders(text: string): string {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "[image]" && line !== "[pdf]" && line !== "[audio]")
    .join("\n")
    .trim();
}

export function extractMessageImages(
  msg?: ChatMessage,
): Array<{ mime: string; bytesBase64?: string; mediaRef?: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "image")
    .map((p) => {
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      const raw = String(anyPart.bytesBase64 || anyPart.bytes_base64 || "").trim();
      return {
        mime: anyPart.mime || "image/webp",
        bytesBase64: raw && !raw.startsWith(MEDIA_REF_PREFIX) ? raw : undefined,
        mediaRef: raw.startsWith(MEDIA_REF_PREFIX) ? raw : undefined,
      };
    })
    .filter((p) => !!p.bytesBase64 || !!p.mediaRef);
}

export function extractMessageAudios(msg?: ChatMessage): Array<{ mime: string; bytesBase64: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "audio")
    .map((p) => {
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      return {
        mime: anyPart.mime || "audio/webm",
        bytesBase64: anyPart.bytesBase64 || anyPart.bytes_base64 || "",
      };
    })
    .filter((p) => !!p.bytesBase64);
}

export function extractMessageAttachmentFiles(
  msg?: ChatMessage,
): Array<{ fileName: string; relativePath: string }> {
  if (!msg) return [];
  const out: Array<{ fileName: string; relativePath: string }> = [];
  const seen = new Set<string>();
  const metaAttachments = Array.isArray((msg.providerMeta as { attachments?: unknown } | undefined)?.attachments)
    ? ((msg.providerMeta as { attachments?: Array<{ fileName?: unknown; relativePath?: unknown }> }).attachments || [])
    : [];
  for (const item of metaAttachments) {
    const fileName = String(item?.fileName || "").trim();
    const relativePath = String(item?.relativePath || "").trim().replace(/\\/g, "/");
    if (!fileName || !relativePath) continue;
    const key = `${fileName}::${relativePath}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ fileName, relativePath });
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
    const key = `${fileName}::${relativePath}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ fileName, relativePath });
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
      else if (p.type === "image") total += 280;
      else if (p.type === "audio") total += 320;
    }
  }
  return Math.ceil(total);
}
