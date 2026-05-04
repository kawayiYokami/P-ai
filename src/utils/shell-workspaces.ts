import type { ShellWorkspaceAccess, ShellWorkspaceLevel } from "../types/app";

const LEGACY_SYSTEM_NAMES = [
  "system workspace",
  "系统工作目录",
  "系統工作目錄",
  "助理私人目录",
  "助理私人目錄",
  "assistant private workspace",
];

const LEGACY_MAIN_NAMES = [
  "shell directory",
  "终端目录",
  "終端目錄",
  "main workspace",
  "主要工作目录",
  "主要工作目錄",
];

const LEGACY_SECONDARY_NAMES = [
  "secondary workspace",
  "次要工作目录",
  "次要工作目錄",
];

export const WORKSPACE_ACCESS_ORDER: ShellWorkspaceAccess[] = ["approval", "full_access", "read_only"];

function normalizeNameToken(value: string): string {
  return String(value || "").trim().toLowerCase();
}

export function workspaceLevelRank(level: string): number {
  if (level === "system") return 0;
  if (level === "main") return 1;
  return 2;
}

export function normalizeWorkspaceLevel(level: string): ShellWorkspaceLevel {
  if (level === "system") return "system";
  if (level === "main") return "main";
  return "secondary";
}

export function defaultWorkspaceNameFromPath(path: string): string {
  const raw = String(path || "").trim();
  if (!raw) return "";
  const normalized = raw.replace(/\\/g, "/").replace(/\/+$/, "");
  const part = normalized.split("/").pop() || "";
  return part.trim();
}

export function isLegacyGenericWorkspaceName(level: ShellWorkspaceLevel, name: string): boolean {
  const normalized = normalizeNameToken(name);
  if (!normalized) return true;
  if (level === "system") {
    return LEGACY_SYSTEM_NAMES.includes(normalized);
  }
  if (level === "main") {
    return LEGACY_MAIN_NAMES.includes(normalized);
  }
  if (LEGACY_SECONDARY_NAMES.includes(normalized)) {
    return true;
  }
  return LEGACY_SECONDARY_NAMES.some((prefix) => normalized.startsWith(`${prefix} `));
}

export function inferWorkspaceName(level: ShellWorkspaceLevel, path: string, index: number): string {
  const fromPath = defaultWorkspaceNameFromPath(path);
  if (fromPath) return fromPath;
  if (level === "system") return "assistant-private";
  if (level === "main") return "shell";
  return `secondary-${index + 1}`;
}
