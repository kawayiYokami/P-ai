export function normalizeLocalLinkHref(href: string): string {
  const trimmed = String(href || "").trim();
  if (!trimmed) return "";

  const decoded = trimmed.replace(/%5C/gi, "\\");
  if (/^[A-Za-z]:[\\/]/.test(decoded)) {
    return `${decoded.slice(0, 2)}${decoded.slice(2).replace(/\\/g, "/")}`;
  }
  return decoded;
}

export function isAbsoluteLocalPath(href: string): boolean {
  const normalized = normalizeLocalLinkHref(href);
  return /^[A-Za-z]:[\\/]/.test(normalized) || normalized.startsWith("\\\\") || normalized.startsWith("/");
}
