import type { ChatMessageBlock } from "../../../types/app";
import { USER_PERSONA_ID, isSystemSpeakerId } from "../../shared/builtin-persona-ids";

/**
 * 蛇形时间线的锚点判定。
 *
 * 锚点语义是「当前会话人格之外的发言」，不是「用户发言」：队伍会话里其他队员的发言同样以
 * `role: "user"` 入库，如果拿 `role === "user"` 当用户，队员提问会被算成自己提问。
 */

export const TIMELINE_TOOL_BREAK = "\uE000TOOLBREAK\uE000";

export type TimelineAnchor = {
  id: string;
  userText: string;
  assistantTail: string;
  index: number;
};

/**
 * 发言人是否当前会话人格。
 * `speakerAgentId` 为空且 role 为 assistant 的旧数据算当前人格；`user-persona` 一律不算。
 */
export function isSessionPersonaBlock(block: ChatMessageBlock, sessionAgentId: string): boolean {
  const speakerId = String(block.speakerAgentId || "").trim();
  if (speakerId === USER_PERSONA_ID) return false;
  if (!speakerId) return String(block.role || "").trim() !== "user";
  const current = String(sessionAgentId || "").trim();
  return !!current && speakerId === current;
}

/** 发言人是否用户：只管「这句谁说的」，用于时间线节点的名字与头像。 */
export function isUserBlock(block: ChatMessageBlock): boolean {
  const speakerId = String(block.speakerAgentId || "").trim();
  if (speakerId) return speakerId === USER_PERSONA_ID;
  return String(block.role || "").trim() === "user";
}

/** 该块能否成为锚点：非流式、非附加文本、非远程来源、非当前人格、非系统人格，且有实际内容。 */
export function isTimelineAnchorBlock(block: ChatMessageBlock, sessionAgentId: string): boolean {
  if (block.isStreaming || block.isExtraTextBlock || block.remoteImOrigin) return false;
  if (isSessionPersonaBlock(block, sessionAgentId)) return false;
  if (isSystemSpeakerId(block.speakerAgentId)) return false;
  const text = String(block.text || "");
  if (text.trim()) return true;
  return (block.images?.length || 0) > 0 || (block.audios?.length || 0) > 0;
}

/** 取一段工具调用文本的最后一段可读内容，用作悬停预览里的回复尾段。 */
export function timelineAssistantTail(raw: string): string {
  const parts = String(raw || "")
    .split(TIMELINE_TOOL_BREAK)
    .map((part) => part.replace(/\[toolcall:[^\]\n]+\]/g, "").replace(/\s+/g, " ").trim())
    .filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1]! : "";
}

/**
 * 计算全部锚点。
 *
 * `virtualIndexById` 是「消息块 id → 虚拟列表下标」，只在实际渲染出来的块上出现；
 * 拿不到下标的锚点无法跳转，直接丢弃。
 */
export function buildTimelineAnchors(
  blocks: ChatMessageBlock[],
  virtualIndexById: Map<string, number>,
  sessionAgentId: string,
): TimelineAnchor[] {
  const anchors: TimelineAnchor[] = [];
  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i]!;
    if (!isTimelineAnchorBlock(block, sessionAgentId)) continue;
    const index = virtualIndexById.get(String(block.id || ""));
    if (index === undefined) continue;
    let assistantTail = "";
    for (let j = i + 1; j < blocks.length; j++) {
      const next = blocks[j]!;
      if (next.isStreaming) continue;
      // 遇到下一个非当前人格发言即收束尾段，取紧随其后的那一轮人格回复
      if (!isSessionPersonaBlock(next, sessionAgentId)) break;
      const tail = timelineAssistantTail(String(next.text || ""));
      if (tail) {
        assistantTail = tail;
        break;
      }
    }
    anchors.push({
      id: String(block.id || `timeline-${i}`),
      userText: String(block.text || ""),
      assistantTail,
      index,
    });
  }
  return anchors;
}
