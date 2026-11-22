import type { ChannelLogEntry } from "./types";

type TranslateFn = (key: string, params?: Record<string, unknown>) => string;

export type ContactLogDisplayItem = {
  timestamp: string;
  level: string;
  kind: string;
  title: string;
  summary: string;
  detail?: string;
};

function logField(message: string, key: string): string {
  // 联系人日志是 key=value 串，字段之间既可能用英文逗号也可能用中文逗号，
  // 键名有英文（contact）也有中文（原因、预计离场），两种都要能切开。
  const match = message.match(
    new RegExp(`${key}=([\\s\\S]*?)(?=[,，]\\s*[A-Za-z_\\u4e00-\\u9fa5]+=|$)`),
  );
  return String(match?.[1] || "").trim();
}

function currentStateLabel(value: string): string {
  const parts = String(value || "").trim().split("->").map((item) => item.trim()).filter(Boolean);
  return parts[parts.length - 1] || "";
}

function boolLabel(value: string, t: TranslateFn): string {
  return value === "是" || value.toLowerCase() === "true" ? t("common.yes") : t("common.no");
}

function decisionLabel(value: string, t: TranslateFn): string {
  if (value === "reply" || value === "reply_async") return t("config.remoteIm.logDecisionReply");
  if (value === "send") return t("config.remoteIm.logDecisionSend");
  if (value === "send_files") return t("config.remoteIm.logDecisionSendFiles");
  if (value === "no_reply") return t("config.remoteIm.logDecisionNoReply");
  if (value === "send_async") return t("config.remoteIm.logDecisionSendAsync");
  return value || t("common.done");
}

function presenceLabel(value: string, t: TranslateFn): string {
  const state = currentStateLabel(value);
  if (state === "Present") return t("config.remoteIm.logPresencePresent");
  if (state === "Away") return t("config.remoteIm.logPresenceAway");
  return state;
}

function presenceTransitionLabel(value: string, t: TranslateFn): string {
  const parts = String(value || "")
    .split("->")
    .map((item) => item.trim())
    .filter(Boolean);
  if (parts.length < 2) return presenceLabel(value, t);
  return `${presenceLabel(parts[0], t)} → ${presenceLabel(parts[parts.length - 1], t)}`;
}

function stateSummary(message: string, t: TranslateFn): string {
  const presence = currentStateLabel(logField(message, "presence"));
  const work = currentStateLabel(logField(message, "work"));
  const activate = logField(message, "activate");
  return [
    presence,
    work,
    activate ? (boolLabel(activate, t) === t("common.yes") ? t("config.remoteIm.logActivate") : t("config.remoteIm.logInactive")) : "",
  ].filter(Boolean).join("；");
}

export function buildContactLogDisplayItem(log: ChannelLogEntry, t: TranslateFn): ContactLogDisplayItem | null {
  const message = String(log.message || "").trim();
  if (message.startsWith("[联系人消息] 收到:")) {
    const senderRaw = logField(message, "sender");
    const senderName = senderRaw || t("config.remoteIm.otherParty");
    const preview = logField(message, "preview") || t("config.remoteIm.receivedMessage");
    const imageCount = Number(logField(message, "image_count") || 0);
    const audioCount = Number(logField(message, "audio_count") || 0);
    const attachmentCount = Number(logField(message, "attachment_count") || 0);
    const extras = [
      imageCount > 0 ? t("config.remoteIm.imageCount", { count: imageCount }) : "",
      audioCount > 0 ? t("config.remoteIm.audioCount", { count: audioCount }) : "",
      attachmentCount > 0 ? t("config.remoteIm.attachmentCount", { count: attachmentCount }) : "",
    ].filter(Boolean);
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindMessage"),
      title: "",
      summary: `[${senderName}]${preview}`,
      detail: extras.length > 0 ? extras.join("，") : undefined,
    };
  }
  if (message.startsWith("[联系人消息] 去重跳过:")) {
    return { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindDedup"), title: t("config.remoteIm.logDedupTitle"), summary: logField(message, "preview") || t("config.remoteIm.logDedupSummary") };
  }
  if (message.startsWith("[联系人消息] 过滤跳过:")) {
    const prefix = logField(message, "prefix");
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindFilter"),
      title: t("config.remoteIm.logFilterTitle"),
      summary: prefix
        ? t("config.remoteIm.logFilterPrefix", { prefix })
        : t("config.remoteIm.logFilterSummary"),
    };
  }
  if (message.startsWith("[联系人消息] 入队:")) {
    return log.level === "warn" || log.level === "error"
      ? { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindSystem"), title: t("config.remoteIm.logEnqueueFailed"), summary: logField(message, "reason") || t("config.remoteIm.logEnqueueFailedSummary") }
      : null;
  }
  if (message.startsWith("[在场切换]")) {
    const expireAt = logField(message, "预计离场");
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindStatus"),
      title: presenceTransitionLabel(logField(message, "presence"), t),
      summary: logField(message, "原因") || t("config.remoteIm.logPresenceChanged"),
      detail: expireAt ? t("config.remoteIm.logPresenceExpireAt", { time: expireAt }) : undefined,
    };
  }
  if (message.startsWith("[巡检]")) {
    const head = message.replace(/^\[巡检\]\s*/, "");
    const separator = head.search(/[:：]/);
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindStatus"),
      title: (separator === -1 ? head : head.slice(0, separator)).trim(),
      summary:
        logField(message, "处理") ||
        logField(message, "路径") ||
        logField(message, "原因") ||
        logField(message, "内容"),
      detail: undefined,
    };
  }
  if (message.startsWith("[闭嘴]")) {
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindStatus"),
      title: t("config.remoteIm.logMutedTitle"),
      summary: logField(message, "原因"),
    };
  }
  if (message.startsWith("[外发]")) {
    const head = message.replace(/^\[外发\]\s*/, "");
    const separator = head.search(/[:：]/);
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindSend"),
      title: (separator === -1 ? head : head.slice(0, separator)).trim(),
      summary: logField(message, "preview") || logField(message, "原因") || logField(message, "错误"),
      detail: undefined,
    };
  }
  if (message.startsWith("[应答判断]")) {
    const head = message.replace(/^\[应答判断\]\s*/, "");
    const separator = head.search(/[:：]/);
    const model = logField(message, "模型");
    return {
      timestamp: log.timestamp,
      level: log.level,
      kind: t("config.remoteIm.logKindStatus"),
      title: (separator === -1 ? head : head.slice(0, separator)).trim(),
      summary: logField(message, "原因") || logField(message, "错误"),
      detail: [
        logField(message, "应回")
          ? t("config.remoteIm.logAnswerShouldReply", { value: logField(message, "应回") })
          : "",
        model ? t("config.remoteIm.logAnswerModel", { model }) : "",
      ]
        .filter(Boolean)
        .join("；") || undefined,
    };
  }
  if (message.startsWith("[联系人状态] 历史落地:")) {
    return log.level === "warn" || log.level === "error"
      ? { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindSystem"), title: t("config.remoteIm.logHistoryWriteFailed"), summary: logField(message, "reason") || t("config.remoteIm.logHistoryWriteFailedSummary") }
      : null;
  }
  if (message.startsWith("[联系人状态] 轮次结束:")) {
    const summary = stateSummary(message, t);
    return { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindStatus"), title: summary, summary, detail: t("config.remoteIm.logTurnDetail", { decision: decisionLabel(logField(message, "decision"), t), followUp: boolLabel(logField(message, "follow_up"), t) }) };
  }
  if (message.startsWith("[联系人状态] 轮次收尾失败:")) {
    const summary = stateSummary(message, t);
    return { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindStatus"), title: summary, summary, detail: logField(message, "error") || undefined };
  }
  return log.level === "warn" || log.level === "error"
    ? { timestamp: log.timestamp, level: log.level, kind: t("config.remoteIm.logKindSystem"), title: t("config.remoteIm.logAbnormalTitle"), summary: t("config.remoteIm.logAbnormalSummary") }
    : null;
}
