export function toErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message || String(error);
  if (typeof error === "string") return error;
  if (error == null) return "unknown";
  if (typeof error === "object" && "message" in error && typeof (error as { message?: unknown }).message === "string") {
    return (error as { message: string }).message;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function resolveKnownI18nErrorKey(errorMessage: string): string {
  const normalized = String(errorMessage || "").trim();
  const lower = normalized.toLowerCase();
  const hasStatusCode = (code: string) => new RegExp(`(^|\\D)${code}(\\D|$)`).test(lower);

  if (normalized === "CHAT_ABORTED_BY_USER") {
    return "status.requestAbortedByUser";
  }
  if (
    hasStatusCode("429")
    || lower.includes("rate limit")
    || lower.includes("too many requests")
    || lower.includes("quota exceeded")
  ) {
    return "status.requestRateLimited";
  }
  if (
    hasStatusCode("503")
    || lower.includes("service unavailable")
    || lower.includes("server overloaded")
    || lower.includes("overloaded")
  ) {
    return "status.requestServiceUnavailable";
  }
  if (
    hasStatusCode("401")
    || lower.includes("unauthorized")
    || lower.includes("invalid api key")
    || lower.includes("incorrect api key")
    || lower.includes("authentication failed")
  ) {
    return "status.requestUnauthorized";
  }
  if (
    hasStatusCode("403")
    || lower.includes("forbidden")
    || lower.includes("permission denied")
  ) {
    return "status.requestForbidden";
  }
  if (
    lower.includes("timed out")
    || lower.includes("timeout")
    || lower.includes("etimedout")
    || lower.includes("deadline exceeded")
  ) {
    return "status.requestTimedOut";
  }
  if (
    lower.includes("failed to fetch")
    || lower.includes("network error")
    || lower.includes("connection reset")
    || lower.includes("connection refused")
    || lower.includes("connection aborted")
    || lower.includes("dns")
    || lower.includes("unreachable")
    || lower.includes("econnreset")
    || lower.includes("econnrefused")
    || lower.includes("eai_again")
  ) {
    return "status.requestNetworkError";
  }
  if (
    hasStatusCode("404")
    || lower.includes("model not found")
    || lower.includes("no such model")
    || lower.includes("does not exist")
  ) {
    return "status.requestModelUnavailable";
  }
  if (
    lower.includes("context length")
    || lower.includes("maximum context length")
    || lower.includes("context window")
    || lower.includes("prompt is too long")
    || lower.includes("token limit")
    || lower.includes("too many tokens")
  ) {
    return "status.requestContextTooLong";
  }
  if (
    lower.includes("insufficient_quota")
    || lower.includes("quota exceeded")
    || lower.includes("billing")
    || lower.includes("credit")
    || lower.includes("balance")
    || lower.includes("payment required")
  ) {
    return "status.requestInsufficientBalance";
  }
  if (
    lower.includes("maintenance")
    || lower.includes("temporarily unavailable")
    || lower.includes("service under maintenance")
    || lower.includes("try again later")
  ) {
    return "status.requestServiceMaintenance";
  }

  return "";
}

export function formatI18nError(
  translate: (key: string, params?: Record<string, unknown>) => string,
  key: string,
  error: unknown,
): string {
  const err = toErrorMessage(error);
  const knownKey = resolveKnownI18nErrorKey(err);
  if (knownKey) {
    const friendly = translate(knownKey);
    if (err === "CHAT_ABORTED_BY_USER") {
      return friendly;
    }
    return translate("status.requestFriendlyWithRaw", {
      reason: friendly,
      err,
    });
  }
  return translate(key, { err });
}
