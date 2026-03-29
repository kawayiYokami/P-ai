function parseIsoTime(value?: string | null): Date | null {
  const raw = String(value || "").trim();
  if (!raw) return null;
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return null;
  return parsed;
}

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

function buildLocalDatePart(date: Date): string {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

function buildLocalTimePart(date: Date): string {
  return `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
}

function buildOffsetPart(date: Date): string {
  const offsetMinutes = -date.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const absoluteMinutes = Math.abs(offsetMinutes);
  const hours = Math.floor(absoluteMinutes / 60);
  const minutes = absoluteMinutes % 60;
  return `${sign}${pad2(hours)}:${pad2(minutes)}`;
}

export function formatDateToLocalRfc3339(date: Date): string {
  return `${buildLocalDatePart(date)}T${pad2(date.getHours())}:${pad2(date.getMinutes())}:${pad2(date.getSeconds())}${buildOffsetPart(date)}`;
}

export function extractIsoLocalDatePart(value?: string | null, emptyText = ""): string {
  const parsed = parseIsoTime(value);
  if (!parsed) return emptyText;
  return buildLocalDatePart(parsed);
}

export function extractIsoLocalTimePart(value?: string | null, emptyText = ""): string {
  const parsed = parseIsoTime(value);
  if (!parsed) return emptyText;
  return buildLocalTimePart(parsed);
}

export function nowLocalDatePart(): string {
  return buildLocalDatePart(new Date());
}

export function nowLocalRfc3339ForInput(): string {
  const now = new Date();
  now.setSeconds(0, 0);
  return formatDateToLocalRfc3339(now);
}

export function composeLocalRfc3339(datePart?: string | null, timePart?: string | null, sourceValue?: string | null): string {
  const normalizedDatePart = String(datePart || "").trim();
  const normalizedTimePart = String(timePart || "").trim();
  if (!normalizedDatePart && !normalizedTimePart) return "";

  const sourceDate = parseIsoTime(sourceValue);
  const baseDatePart = normalizedDatePart || (sourceDate ? buildLocalDatePart(sourceDate) : buildLocalDatePart(new Date()));
  const baseTimePart = normalizedTimePart || (sourceDate ? buildLocalTimePart(sourceDate) : buildLocalTimePart(new Date()));

  const [year, month, day] = baseDatePart.split("-").map((part) => Number(part));
  const [hour, minute] = baseTimePart.split(":").map((part) => Number(part));
  if (
    !Number.isInteger(year) ||
    !Number.isInteger(month) ||
    !Number.isInteger(day) ||
    !Number.isInteger(hour) ||
    !Number.isInteger(minute)
  ) {
    return String(sourceValue || "").trim();
  }

  const seconds = sourceDate?.getSeconds() ?? 0;
  const nextDate = new Date(year, month - 1, day, hour, minute, seconds, 0);
  return formatDateToLocalRfc3339(nextDate);
}

export function nudgeLocalRfc3339Minutes(value: string | null | undefined, deltaMinutes: number): string {
  const baseDate = parseIsoTime(value) ?? new Date();
  if (!parseIsoTime(value)) {
    baseDate.setSeconds(0, 0);
  }
  const nextDate = new Date(baseDate.getTime() + deltaMinutes * 60_000);
  return formatDateToLocalRfc3339(nextDate);
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
