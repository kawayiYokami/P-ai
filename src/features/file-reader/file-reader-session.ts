import type { FileReaderSessionState } from "./types";
import { normalizePath } from "./utils";

// ==================== 会话状态读写 ====================

/**
 * 阅读器会话状态的读写与「已打开文件」推导。
 * 阅读器面板与卡片墙（幕墙）共用：面板在场时由面板写，面板未挂载时（幕墙在场）由卡片墙写。
 */

/** diff 标签是合成伪路径（git-diff:...），不对应磁盘文件，不进已打开文件列表也不落盘 */
function isDiffPseudoPath(path: string) {
  return String(path || "").trim().startsWith("git-diff:");
}

export function readFileReaderSessionState(storageKey: string, legacyStorageKey?: string): FileReaderSessionState {
  const key = String(storageKey || "").trim();
  if (!key || typeof window === "undefined") return {};
  try {
    const legacyKey = String(legacyStorageKey || "").trim();
    const raw = window.localStorage.getItem(key)
      || (legacyKey ? window.localStorage.getItem(legacyKey) : "")
      || "{}";
    return JSON.parse(raw) as FileReaderSessionState;
  } catch {
    return {};
  }
}

export function writeFileReaderSessionState(storageKey: string, state: FileReaderSessionState) {
  const key = String(storageKey || "").trim();
  if (!key || typeof window === "undefined") return;
  window.localStorage.setItem(key, JSON.stringify(state));
}

// ==================== 已打开文件 ====================

/** 会话状态里的已打开文件：归一化、去重、剔除伪路径，顺序即标签顺序 */
export function listSessionFilePaths(state: FileReaderSessionState | null | undefined): string[] {
  const tabs = Array.isArray(state?.tabs) ? state.tabs : [];
  const paths = tabs
    .map((path) => normalizePath(String(path || "")))
    .filter((path) => path && !isDiffPseudoPath(path));
  return Array.from(new Set(paths));
}

/** 会话状态里的当前文件；无有效记录时与面板恢复口径一致，落在第一个文件上 */
export function sessionActiveFilePath(state: FileReaderSessionState | null | undefined): string {
  const paths = listSessionFilePaths(state);
  const active = normalizePath(String(state?.activePath || ""));
  if (active && !isDiffPseudoPath(active) && paths.includes(active)) return active;
  return paths[0] || "";
}

/** 从会话状态里移除某个文件；移除的是当前文件时，接替顺序与面板关闭标签一致（左邻优先，其次右邻） */
export function removeFilePathFromSessionState(
  state: FileReaderSessionState | null | undefined,
  path: string,
): FileReaderSessionState {
  const paths = listSessionFilePaths(state);
  const target = normalizePath(path);
  const removedIndex = paths.indexOf(target);
  if (removedIndex < 0) return { ...(state || {}), tabs: paths, activePath: sessionActiveFilePath(state) };

  const remaining = paths.filter((item) => item !== target);
  const active = sessionActiveFilePath(state);
  const activePath = active === target
    ? remaining[Math.max(0, removedIndex - 1)] || remaining[0] || ""
    : active;
  return { ...(state || {}), tabs: remaining, activePath };
}
