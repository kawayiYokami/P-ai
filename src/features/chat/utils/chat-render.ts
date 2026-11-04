import type { ChatMessageBlock } from "../../../types/app";
import { textContentSignature } from "./text-signature";

// ==================== 类型 ====================

export type ChatRenderItem =
  | { kind: "compaction"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "plan_started"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "message"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number; compactWithPrevious: boolean };

// ==================== 常量 ====================

export const FILE_READER_EXTENSIONS = new Set([
  "md", "markdown", "mdx",
  "ts", "tsx", "c", "cc", "cpp", "cxx", "h", "hpp",
  "cs", "java", "kt", "kts", "go",
  "js", "jsx", "vue", "rs", "py", "rb", "php",
  "swift", "scala", "dart", "lua", "r", "m", "mm",
  "pl", "pm", "json", "jsonc", "json5", "toml",
  "yaml", "yml", "css", "scss", "sass", "less",
  "html", "htm", "xml", "svg", "sql",
  "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd",
  "dockerfile", "ini", "env",
  "gitignore", "gitattributes", "editorconfig",
  "lock", "csv", "tsv", "txt", "log", "cea",
]);

// ==================== 纯函数 ====================

export function fileExtensionFromPath(path: string): string {
  const normalizedPath = String(path || "").trim().replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").filter(Boolean).pop() || "";
  const normalizedFileName = fileName.startsWith(".") ? fileName.slice(1) : fileName;
  const lowerFileName = normalizedFileName.toLowerCase();
  if (FILE_READER_EXTENSIONS.has(lowerFileName)) return lowerFileName;
  const dotIndex = normalizedFileName.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === normalizedFileName.length - 1) return "";
  return normalizedFileName.slice(dotIndex + 1).toLowerCase();
}

export function canOpenInFileReader(path: string): boolean {
  return FILE_READER_EXTENSIONS.has(fileExtensionFromPath(path));
}

function countFenceMatches(text: string, pattern: RegExp): number {
  if (!text) return 0;
  return Array.from(text.matchAll(pattern)).length;
}

export function estimateMessageBlockHeight(block: ChatMessageBlock, isOwn: boolean): number {
  let estimate = isOwn ? 78 : 108;
  const text = String(block.text || "");
  const combinedTextLength = text.length;
  estimate += Math.min(920, Math.ceil(combinedTextLength / 28) * 9);

  const codeFenceCount = countFenceMatches(text, /```[\w-]*\s*[\r\n]/g);
  const mermaidFenceCount = countFenceMatches(text, /```(?:\s*)mermaid\b/gi);
  estimate += codeFenceCount * 180;
  estimate += mermaidFenceCount * 120;

  if (block.planCard) {
    estimate += block.planCard.action === "present" ? 144 : 84;
  }
  if (block.taskTrigger) estimate += 120;
  if (block.activityItems.length > 0 || block.activityRunning) estimate += 42;
  estimate += block.images.length * 120;
  estimate += block.audios.length * 42;
  estimate += block.attachmentFiles.length * 34;
  return Math.max(64, estimate);
}

/**
 * 已闭合条目的内容不会再变，不必哈希全文：列表级 memo 每帧对一屏所有条目做这件事，
 * 长正文时是帧预算里最大的一项。只留长度的风险是「等长替换」漏检，所以留长度 + 首/中/尾字符，
 * 仍是 O(1)，但把等长不同内容这条硬性关掉。
 */
function closedTextSignature(text?: string): string {
  const input = String(text || "");
  const length = input.length;
  if (length === 0) return "0";
  return `${length}:${input.charCodeAt(0)}:${input.charCodeAt(length >> 1)}:${input.charCodeAt(length - 1)}`;
}

function activityItemsSignature(block: ChatMessageBlock): string {
  return (block.activityItems || [])
    .map((item) => {
      if (item.kind === "reasoning" || item.kind === "content") {
        const tag = item.kind === "reasoning" ? "r" : "c";
        // 只有还在生长的条目会继续变长，它照旧全文哈希
        if (item.running) return [tag, item.id || "", textContentSignature(item.text), "1"].join(":");
        return [tag, item.id || "", closedTextSignature(item.text), "0"].join(":");
      }
      const done = item.status !== "doing";
      return [
        "t",
        item.id || "",
        item.toolCallId || "",
        item.name || "",
        item.status || "",
        done ? closedTextSignature(item.argsText) : textContentSignature(item.argsText),
        done ? closedTextSignature(item.resultText) : textContentSignature(item.resultText),
      ].join(":");
    })
    .join("|");
}

function toolCallsSignature(block: ChatMessageBlock): string {
  return (block.toolCalls || [])
    .map((item) => [
      item.name || "",
      item.status || "",
      String(item.argsText || "").length,
    ].join(":"))
    .join("|");
}

export function isCompactionBlock(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const meta = (block.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
  const kind = String(messageMeta.kind || "").trim();
  return kind === "context_compaction" || kind === "summary_context_seed";
}

export function isRightAlignedMessage(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) {
    return block.role === "user" && block.remoteImOrigin.remoteContactType !== "group";
  }
  if (block.role === "user") return true;
  const id = String(block.speakerAgentId || "").trim();
  return id === "user-persona";
}

export function isCompactUserContinuation(block: ChatMessageBlock, previousBlock: ChatMessageBlock | null): boolean {
  if (!previousBlock) return false;
  if (!isRightAlignedMessage(block) || !isRightAlignedMessage(previousBlock)) return false;
  if (block.dividerKind || previousBlock.dividerKind) return false;
  if (block.isExtraTextBlock || previousBlock.isExtraTextBlock) return false;
  return true;
}

export function blockSizeDependencies(block: ChatMessageBlock): unknown[] {
  return [
    String(block.id || ""),
    String(block.sourceMessageId || ""),
    String(block.text || ""),
    activityItemsSignature(block),
    block.activityItems.length,
    block.activityReasoningCharCount,
    block.activityRunning,
    block.activityStatus,
    block.images.length,
    block.audios.length,
    block.attachmentFiles.length,
    toolCallsSignature(block),
    block.toolCalls.length,
    block.planCard?.action || "",
    block.planCard?.path || "",
    String(block.taskTrigger ? JSON.stringify(block.taskTrigger) : ""),
  ];
}

export function buildChatRenderTimeline(
  messageBlocks: ChatMessageBlock[],
  renderIdOf: (block: ChatMessageBlock) => string,
): ChatRenderItem[] {
  const items: ChatRenderItem[] = [];
  let previousMessageBlock: ChatMessageBlock | null = null;

  messageBlocks.forEach((block, blockIndex) => {
    const renderId = renderIdOf(block);
    if (block.dividerKind === "plan_started") {
      previousMessageBlock = null;
      items.push({ kind: "plan_started", id: `plan-started-${renderId}`, renderId, block, blockIndex });
      return;
    }
    if (isCompactionBlock(block)) {
      previousMessageBlock = null;
      items.push({ kind: "compaction", id: `compaction-${renderId}`, renderId, block, blockIndex });
      return;
    }
    const compactWithPrevious = isCompactUserContinuation(block, previousMessageBlock);
    items.push({ kind: "message", id: `message-${renderId}`, renderId, block, blockIndex, compactWithPrevious });
    previousMessageBlock = block;
  });

  return items;
}
