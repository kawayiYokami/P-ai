/**
 * MCP 定义 JSON 的成员名改写。
 *
 * 连接器的显示名与定义里的成员名是同一份数据：定义里第一个成员叫什么，卡片就显示什么。
 * 改名时直接改写定义本身，不另存一份名字，否则保存后前端与文件各说各话、名字会被读回覆盖。
 */

/** 与卡片解析保持一致的「单 server 直接字段」签名 */
const DIRECT_FIELD_KEYS = ["command", "args", "url", "transport", "type", "env", "cwd"];

/**
 * 把定义改写成「第一个成员叫 newName」，其余成员与字段原样保留。
 * 目标是定义里的命名位置：mcpServers 的键、数组项的 name、或单 server 根上的 name。
 * 定义不是可解析的 JSON、或没有任何可命名的成员、或名字为空时返回 null（调用方保持原定义不动）。
 */
export function renameFirstMcpServerMember(definitionJson: string, newName: string): string | null {
  const name = newName.trim();
  if (!name) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(definitionJson);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;

  // 根级数组：改第一项的 name 字段
  if (Array.isArray(parsed)) {
    const first = parsed.find((item) => item && typeof item === "object");
    if (!first) return null;
    (first as Record<string, unknown>).name = name;
    return JSON.stringify(parsed, null, 2);
  }

  const root = parsed as Record<string, unknown>;

  // mcpServers：对象改第一个键，数组改第一项的 name
  const servers = root.mcpServers;
  if (servers && typeof servers === "object") {
    if (Array.isArray(servers)) {
      const first = servers.find((item) => item && typeof item === "object");
      if (!first) return null;
      (first as Record<string, unknown>).name = name;
    } else {
      const renamed = renameFirstKey(servers as Record<string, unknown>, name);
      if (!renamed) return null;
      root.mcpServers = renamed;
    }
    return JSON.stringify(parsed, null, 2);
  }

  // 单 server 直接字段：改根上的 name
  if (DIRECT_FIELD_KEYS.some((key) => key in root)) {
    root.name = name;
    return JSON.stringify(parsed, null, 2);
  }

  // 平铺命名集合：改第一个键
  const renamed = renameFirstKey(root, name);
  if (!renamed) return null;
  return JSON.stringify(renamed, null, 2);
}

/** 只替换第一个键，其余键保持原有插入顺序 */
function renameFirstKey(
  source: Record<string, unknown>,
  newName: string,
): Record<string, unknown> | null {
  const entries = Object.entries(source);
  if (entries.length === 0) return null;
  const out: Record<string, unknown> = {};
  let replaced = false;
  for (const [key, value] of entries) {
    if (!replaced) {
      out[newName] = value;
      replaced = true;
    } else {
      out[key] = value;
    }
  }
  return out;
}
