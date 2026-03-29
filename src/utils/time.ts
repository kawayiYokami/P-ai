function parseIsoTime(value?: string | null): Date | null {
  const raw = String(value || "").trim();
  if (!raw) return null;
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return null;
  return parsed;
}

export function formatIsoToLocalDateTime(value?: string | null, emptyText = "-"): string {
  const raw = String(value || "").trim();
  if (!raw) return emptyText;
  const parsed = parseIsoTime(raw);
  if (!parsed) return raw;
  const parts = new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).formatToParts(parsed);
  const pick = (type: string) => parts.find((part) => part.type === type)?.value || "00";
  return `${pick("year")}-${pick("month")}-${pick("day")} ${pick("hour")}:${pick("minute")}:${pick("second")}`;
}

export function formatIsoToLocalHourMinute(value?: string | null, emptyText = ""): string {
  const raw = String(value || "").trim();
  if (!raw) return emptyText;
  const parsed = parseIsoTime(raw);
  if (!parsed) return raw;
  const parts = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).formatToParts(parsed);
  const pick = (type: string) => parts.find((part) => part.type === type)?.value || "00";
  return `${pick("hour")}:${pick("minute")}`;
}
