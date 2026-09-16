import type { PermissionCatalog, PermissionCatalogItem } from "../../../types/app";

// ========== 类型 ==========

export type PermissionLeafCategory = "builtinToolNames" | "skillNames" | "mcpToolNames";

export type PermissionTreeLeaf = {
  category: PermissionLeafCategory;
  name: string;
  displayName: string;
  description: string;
  enabled: boolean;
  disabled?: boolean;
  badge?: string;
  locked?: boolean;
};

export type PermissionGroupState = "all" | "none" | "partial";

export type PermissionTreeGroup = {
  key: string;
  label: string;
  state: PermissionGroupState;
  leaves: PermissionTreeLeaf[];
};

export type PermissionTreeSection = {
  key: PermissionLeafCategory;
  label: string;
  disabled: boolean;
  groups: PermissionTreeGroup[];
  leaves: PermissionTreeLeaf[];
};

// ========== 权限目录归一化 ==========

function normalizeCatalogItems(raw: unknown): PermissionCatalogItem[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((entry) => {
      const record = (entry ?? {}) as Record<string, unknown>;
      return {
        name: String(record.name || "").trim(),
        description: String(record.description || "").trim(),
        group: String(record.group || "").trim(),
      };
    })
    .filter((entry) => !!entry.name);
}

export function normalizePermissionCatalog(payload: unknown): PermissionCatalog {
  const record = (payload ?? {}) as Record<string, unknown>;
  return {
    builtinTools: normalizeCatalogItems(record.builtinTools),
    skills: normalizeCatalogItems(record.skills),
    mcpTools: normalizeCatalogItems(record.mcpTools),
  };
}

// ========== 内置工具前端分组表 ==========

type BuiltinToolGroupDef = { key: string; tools: readonly string[] };

const BUILTIN_TOOL_GROUP_DEFS: readonly BuiltinToolGroupDef[] = [
  { key: "files", tools: ["read", "read_file", "read_media", "write", "update", "delete", "move"] },
  { key: "execConfig", tools: ["exec", "config"] },
  { key: "desktop", tools: ["operate", "windows"] },
  { key: "web", tools: ["fetch", "websearch"] },
  { key: "delegate", tools: ["delegate"] },
  { key: "media", tools: ["image_generate", "image_edit", "meme"] },
];

const OTHER_GROUP_KEY = "other";
const MCP_GROUP_KEY_PREFIX = "server:";

function buildLeaf(
  category: PermissionLeafCategory,
  item: PermissionCatalogItem,
  displayName: string,
  isEnabled: (name: string) => boolean,
): PermissionTreeLeaf {
  return {
    category,
    name: item.name,
    displayName,
    description: item.description,
    enabled: isEnabled(item.name),
  };
}

function aggregateGroupState(leaves: PermissionTreeLeaf[]): PermissionGroupState {
  const enabledCount = leaves.filter((leaf) => leaf.enabled).length;
  if (enabledCount === 0) return "none";
  if (enabledCount === leaves.length) return "all";
  return "partial";
}

function buildGroup(key: string, label: string, leaves: PermissionTreeLeaf[]): PermissionTreeGroup {
  return { key, label, state: aggregateGroupState(leaves), leaves };
}

// ========== 内置工具：按功能域分组 ==========

export function buildBuiltinToolGroups(
  items: PermissionCatalogItem[],
  isEnabled: (name: string) => boolean,
  labelFor: (groupKey: string) => string,
): PermissionTreeGroup[] {
  const byName = new Map(items.map((item) => [item.name, item]));
  const groups: PermissionTreeGroup[] = [];
  const assigned = new Set<string>();
  for (const def of BUILTIN_TOOL_GROUP_DEFS) {
    const leaves = def.tools
      .filter((tool) => byName.has(tool))
      .map((tool) => {
        assigned.add(tool);
        return buildLeaf("builtinToolNames", byName.get(tool)!, tool, isEnabled);
      });
    if (leaves.length > 0) groups.push(buildGroup(def.key, labelFor(def.key), leaves));
  }
  const others = items.filter((item) => !assigned.has(item.name));
  if (others.length > 0) {
    groups.push(
      buildGroup(
        OTHER_GROUP_KEY,
        labelFor(OTHER_GROUP_KEY),
        others.map((item) => buildLeaf("builtinToolNames", item, item.name, isEnabled)),
      ),
    );
  }
  return groups;
}

// ========== MCP 工具：按 server 分组 ==========

export function buildMcpToolGroups(
  items: PermissionCatalogItem[],
  isEnabled: (name: string) => boolean,
  otherLabel: string,
): PermissionTreeGroup[] {
  const grouped = new Map<string, PermissionTreeLeaf[]>();
  const otherLeaves: PermissionTreeLeaf[] = [];
  for (const item of items) {
    const groupName = (item.group || "").trim();
    if (!groupName) {
      otherLeaves.push(buildLeaf("mcpToolNames", item, item.name, isEnabled));
      continue;
    }
    const bucket = grouped.get(groupName) ?? [];
    bucket.push(buildLeaf("mcpToolNames", item, item.name, isEnabled));
    grouped.set(groupName, bucket);
  }
  const groups = Array.from(grouped.entries()).map(([server, leaves]) =>
    buildGroup(`${MCP_GROUP_KEY_PREFIX}${server}`, server, leaves),
  );
  if (otherLeaves.length > 0) groups.push(buildGroup(OTHER_GROUP_KEY, otherLabel, otherLeaves));
  return groups;
}
