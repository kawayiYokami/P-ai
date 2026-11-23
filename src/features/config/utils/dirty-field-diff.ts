import type { DirtyFieldDiff } from "../composables/use-unsaved-changes-guard";

/**
 * 「未保存的更改」弹窗里字段清单的生成工具。
 *
 * 只服务于 dev 模式下的排障：把两份快照逐层比对，列出真正不一致的叶子字段。
 * 不参与脏判定——谁算脏仍由各页面的 isDirty 决定，这里不改变任何判定结果。
 */

/** 单个值的最大展示字符数，超出截断 */
export const DIRTY_VALUE_MAX_CHARS = 80;
/** 明细条数上限，避免大配置把弹窗塞满 */
export const DIRTY_DETAIL_MAX_ITEMS = 200;

/** 清单里路径中的 id 最多显示多少个字符，超出加省略号 */
export const DIRTY_PATH_ID_MAX_CHARS = 8;

/**
 * 只用于显示的密钥字段识别。
 * 不用裸的 key / token —— contextWindowTokens、maxTokens、maxTokenCount、tokenizer
 * 这类计数字段都会被误伤，真正想看的那行变成 *** → ***，等于没显示。
 */
const SENSITIVE_FIELD_PATTERN = /apikey|secret|password|accesstoken|authtoken|refreshtoken|privatekey/i;

export function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 数字与布尔不可能是密钥内容，即使是 token 计数也不该被打码 */
function canHoldSecret(value: unknown): boolean {
  return typeof value !== "number" && typeof value !== "boolean";
}

/** 把字段值格式化成可展示的一行：密钥打码、长值截断、缺失与空值分开表达 */
export function formatDirtyValue(value: unknown, fieldName: string): string {
  if (value === undefined) return "（无）";
  const raw = typeof value === "string" ? value : JSON.stringify(value) ?? "";
  if (canHoldSecret(value) && SENSITIVE_FIELD_PATTERN.test(fieldName)) {
    return raw ? "***" : "（空）";
  }
  if (!raw) return "（空）";
  return raw.length > DIRTY_VALUE_MAX_CHARS ? `${raw.slice(0, DIRTY_VALUE_MAX_CHARS)}…` : raw;
}

/**
 * 把路径里的长 id 截短后用于显示，不改变数据本身。
 * `apiProviders[api-config-1771703416707].models[api-config-1771703416707-model-default].contextWindowTokens`
 * → `apiProviders[api-conf…].models[api-conf…].contextWindowTokens`
 */
export function shortenDirtyPathForDisplay(path: string): string {
  return String(path || "").replace(/\[([^\]]+)\]/g, (_match, id: string) => {
    // 带 / 的是技能目录这种路径而不是 id，截了就看不出改的是哪个，保持原样
    if (id.includes("/") || id.length <= DIRTY_PATH_ID_MAX_CHARS) return `[${id}]`;
    return `[${id.slice(0, DIRTY_PATH_ID_MAX_CHARS)}…]`;
  });
}

/** 解析快照 JSON；空串按空对象处理，坏 JSON 抛给调用方决定怎么办 */
export function parseDirtySnapshot(json: string): unknown {
  const text = String(json || "").trim();
  if (!text) return {};
  return JSON.parse(text) as unknown;
}

function joinPath(prefix: string, segment: string): string {
  return prefix ? `${prefix}.${segment}` : segment;
}

function walk(
  saved: unknown,
  current: unknown,
  path: string,
  out: DirtyFieldDiff[],
): void {
  if (out.length >= DIRTY_DETAIL_MAX_ITEMS) return;

  if (isPlainRecord(saved) && isPlainRecord(current)) {
    const keys = [...new Set([...Object.keys(saved), ...Object.keys(current)])].sort();
    for (const key of keys) {
      walk(saved[key], current[key], joinPath(path, key), out);
    }
    return;
  }

  if (Array.isArray(saved) && Array.isArray(current)) {
    // 带 id 的数组按 id 配对，避免中间插一条导致后面全部错位、报出一片假脏；
    // 其余（纯标量数组、无 id 的对象数组）只能按位置配对。
    const allKeyed = [...saved, ...current].every(
      (item) => isPlainRecord(item) && typeof item.id === "string",
    );
    if (allKeyed) {
      const ids = [...new Set([
        ...saved.map((item) => String((item as Record<string, unknown>).id)),
        ...current.map((item) => String((item as Record<string, unknown>).id)),
      ])];
      for (const id of ids) {
        const savedItem = saved.find((item) => isPlainRecord(item) && String(item.id) === id);
        const currentItem = current.find((item) => isPlainRecord(item) && String(item.id) === id);
        walk(savedItem, currentItem, `${path}[${id}]`, out);
      }
      return;
    }
    const length = Math.max(saved.length, current.length);
    for (let index = 0; index < length; index += 1) {
      walk(saved[index], current[index], `${path}[${index}]`, out);
    }
    return;
  }

  if (JSON.stringify(saved) === JSON.stringify(current)) return;
  const fieldName = path.split(".").pop() ?? path;
  out.push({
    path,
    before: formatDirtyValue(saved, fieldName),
    after: formatDirtyValue(current, fieldName),
  });
}

/**
 * 逐层比对两份快照，返回叶字段级差异清单。
 * `pathPrefix` 是清单里显示的最外层名字，如 `apiProviders`、`personas`。
 */
export function collectDirtyFields(
  saved: unknown,
  current: unknown,
  pathPrefix = "",
): DirtyFieldDiff[] {
  const out: DirtyFieldDiff[] = [];
  walk(saved, current, pathPrefix, out);
  return out;
}
