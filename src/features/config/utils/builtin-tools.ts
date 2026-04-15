import type { ApiToolItem } from "../../../types/app";

const BUILTIN_TOOL_DEFAULTS: ReadonlyArray<Readonly<ApiToolItem>> = [
  { id: "fetch", command: "npx", args: ["-y", "@iflow-mcp/fetch"], enabled: true, values: {} },
  { id: "websearch", command: "npx", args: ["-y", "bing-cn-mcp"], enabled: true, values: {} },
  { id: "remember", command: "builtin", args: ["remember"], enabled: true, values: {} },
  { id: "recall", command: "builtin", args: ["recall"], enabled: true, values: {} },
  { id: "operate", command: "builtin", args: ["operate"], enabled: true, values: {} },
  { id: "exec", command: "builtin", args: ["exec"], enabled: true, values: {} },
  { id: "read_file", command: "builtin", args: ["read_file"], enabled: true, values: {} },
  { id: "apply_patch", command: "builtin", args: ["apply_patch"], enabled: true, values: {} },
  { id: "command", command: "builtin", args: ["command"], enabled: true, values: {} },
  { id: "plan", command: "builtin", args: ["plan"], enabled: true, values: {} },
  { id: "task", command: "builtin", args: ["task"], enabled: true, values: {} },
  { id: "todo", command: "builtin", args: ["todo"], enabled: true, values: {} },
  { id: "delegate", command: "builtin", args: ["delegate"], enabled: true, values: {} },
  { id: "remote_im_send", command: "builtin", args: ["remote_im_send"], enabled: false, values: {} },
];

export function defaultToolBindings(): ApiToolItem[] {
  return BUILTIN_TOOL_DEFAULTS.map((tool) => ({
    id: tool.id,
    command: tool.command,
    args: [...tool.args],
    enabled: tool.enabled,
    values: { ...(tool.values as Record<string, unknown>) },
  }));
}

export function normalizeToolBindings(
  current: ApiToolItem[] | null | undefined,
): ApiToolItem[] {
  const items = Array.isArray(current) ? current : [];
  return defaultToolBindings().map((tool) => {
    const found = items.find((item) => item.id === tool.id);
    return {
      id: tool.id,
      command: found?.command || tool.command,
      args: Array.isArray(found?.args) ? [...found.args] : [...tool.args],
      enabled: typeof found?.enabled === "boolean" ? found.enabled : tool.enabled,
      values: { ...((found?.values as Record<string, unknown> | undefined) ?? (tool.values as Record<string, unknown>)) },
    };
  });
}
