import type { ChatMessage } from "../types/app";

export function parseAssistantStoredText(rawText: string): {
  assistantText: string;
  reasoningStandard: string;
  reasoningInline: string;
} {
  const raw = rawText || "";
  const standardMarker = "[标准思考]";
  const standardIdx = raw.indexOf(standardMarker);

  if (standardIdx < 0) {
    return {
      assistantText: raw.trim(),
      reasoningStandard: "",
      reasoningInline: "",
    };
  }

  const reasoningStandard = raw.slice(standardIdx + standardMarker.length).trim();

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
      if (p.type === "image") return "[image]";
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
    .filter((line) => line !== "[image]" && line !== "[audio]")
    .join("\n")
    .trim();
}

export function extractMessageImages(msg?: ChatMessage): Array<{ mime: string; bytesBase64: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "image")
    .map((p) => {
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      return {
        mime: anyPart.mime || "image/webp",
        bytesBase64: anyPart.bytesBase64 || anyPart.bytes_base64 || "",
      };
    })
    .filter((p) => !!p.bytesBase64);
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

