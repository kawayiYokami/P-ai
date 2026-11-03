/**
 * 快速模型失败的「可读原因」提炼。
 *
 * 原始错误常常长这样：
 *   GenAI 流式收集失败：WebStream { model_iden: ModelIden { adapter_kind: OpenAIResp, ... }, cause: "HTTP error.\nStatus: 502 Bad Gateway" }
 * 直接印出来人看不懂，所以这里只判断两件事：错误类型 + 一句细节。
 * 判定顺序固定，方便单测锁住；类型到文案的映射交给 i18n。
 */

export type QuickModelErrorKind = "server" | "auth" | "timeout" | "notFound" | "unknown";

export type QuickModelErrorHint = {
  kind: QuickModelErrorKind;
  /** 从错误里认出 HTTP 状态码时带上，没认出来是 null。 */
  status: number | null;
  /** 压成单行并截断的原始信息，用于兜底展示。 */
  detail: string;
};

/** 细节行最长保留多少字符：再长就截断加省略号。 */
const DETAIL_MAX_CHARS = 80;

function normalizeRawError(raw: unknown): string {
  return String(raw ?? "")
    .replace(/\s+/g, " ")
    .trim();
}

function extractStatusCode(text: string): number | null {
  // 只认 4xx / 5xx：错误里常混着端口号、时间戳，2xx/3xx 出现基本不是状态码。
  const matched = text.match(/\b([45]\d{2})\b/);
  if (!matched) return null;
  const parsed = Number(matched[1]);
  return Number.isFinite(parsed) ? parsed : null;
}

function truncateDetail(text: string): string {
  if (text.length <= DETAIL_MAX_CHARS) return text;
  return `${text.slice(0, DETAIL_MAX_CHARS)}…`;
}

export function classifyQuickModelError(raw: unknown): QuickModelErrorHint {
  const text = normalizeRawError(raw);
  const lower = text.toLowerCase();
  const status = extractStatusCode(text);
  const detail = truncateDetail(text);

  if (status === 401 || status === 403) {
    return { kind: "auth", status, detail };
  }
  if (status !== null && status >= 500) {
    return { kind: "server", status, detail };
  }
  if (status === 404) {
    return { kind: "notFound", status, detail };
  }
  if (/timeout|timed out|超时|time out/.test(lower)) {
    return { kind: "timeout", status, detail };
  }
  if (/unauthorized|invalid api key|incorrect api key|authentication|密钥|鉴权/.test(lower)) {
    return { kind: "auth", status, detail };
  }
  if (/model not found|not found|模型不存在|no such model/.test(lower)) {
    return { kind: "notFound", status, detail };
  }
  return { kind: "unknown", status, detail };
}
