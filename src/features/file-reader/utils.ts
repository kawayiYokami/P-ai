import { CODE_LANGUAGE_BY_EXTENSION, CUSTOM_SHIKI_LANGUAGE_IDS, SHIKI_LANGUAGE_KEYS } from "./constants";
import type { FileReaderDirectoryEntry, FileReaderFileKind, FileTab } from "./types";

const IMAGE_FILE_EXTENSIONS = new Set([
  "avif", "bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp",
]);

const AUDIO_FILE_EXTENSIONS = new Set([
  "aac", "flac", "m4a", "mp3", "oga", "ogg", "opus", "wav", "weba",
]);

const VIDEO_FILE_EXTENSIONS = new Set([
  "mp4", "webm",
]);

const UNSUPPORTED_MEDIA_EXTENSIONS = new Set([
  "3g2", "3gp", "aif", "aiff", "amr", "ape", "asf", "avi", "caf", "flv",
  "heic", "heif", "m2ts", "mid", "midi", "mkv", "mov", "mpeg", "mpg", "mts",
  // 注意：不要把 "ts" 放进黑名单——TypeScript 源码也是 .ts；
  // MPEG-TS 请用更具体的 mts/m2ts。
  "psd", "raw", "rm", "rmvb", "tif", "tiff", "vob", "wma", "wmv",
]);

const UNSUPPORTED_BINARY_EXTENSIONS = new Set([
  "7z", "a", "accdb", "apk", "app", "arrow", "bin", "br", "bz2", "class",
  "com", "dat", "db", "db3", "dbf", "deb", "dll", "dmg", "doc", "docx",
  "duckdb", "dylib", "eot", "exe", "feather", "gz", "img", "ipa", "iso",
  "jar", "lib", "mdb", "msi", "o", "obj", "odp", "ods", "odt", "orc", "otf",
  "pak", "parquet", "pdf", "pfx", "pkg", "ppt", "pptx", "pyo", "pyc", "rar",
  "rpm", "rtf", "scr", "so", "sqlite", "sqlite3", "sys", "tar", "tgz", "ttf",
  "war", "wasm", "woff", "woff2", "xls", "xlsx", "xz", "zip", "zst",
]);

export function normalizePath(path: string) {
  return String(path || "")
    .trim()
    .replace(/^\\\\\?\\/, "")
    .replace(/^\/\/\?\//, "")
    .replace(/^\/\?\//, "")
    .replace(/^\?\//, "")
    .replace(/^\?\\/, "")
    .replace(/\\/g, "/")
    .replace(/^\/([A-Za-z]:)/, "$1");
}

export function sameNormalizedPath(left: string, right: string) {
  return normalizePath(left).toLowerCase() === normalizePath(right).toLowerCase();
}

export function splitContentLines(value: string) {
  const normalized = String(value || "").replace(/\r\n/g, "\n");
  return normalized.length > 0 ? normalized.split("\n") : [];
}

export function buildFileBlockKey(path: string, startLine: number, endLine: number) {
  return `${normalizePath(path)}::${startLine}-${endLine}`;
}

export function normalizeSelectedText(value: string) {
  return String(value || "")
    .replace(/\r\n/g, "\n")
    .replace(/\u00a0/g, " ")
    .trim()
    .slice(0, 20_000);
}

export function resolveVisibleLineRange(scroller: HTMLElement, totalLines: number): { startLine: number; endLine: number } {
  const scrollableHeight = Math.max(1, scroller.scrollHeight - scroller.clientHeight);
  const startRatio = Math.max(0, Math.min(1, scroller.scrollTop / scrollableHeight));
  const visibleRatio = Math.max(0.05, Math.min(1, scroller.clientHeight / Math.max(1, scroller.scrollHeight)));
  const startLine = Math.max(1, Math.min(totalLines, Math.floor(startRatio * totalLines) + 1));
  const visibleLineCount = Math.max(12, Math.ceil(totalLines * visibleRatio));
  const endLine = Math.max(startLine, Math.min(totalLines, startLine + visibleLineCount - 1));
  return { startLine, endLine };
}

export function resolveRawSelectedLineRange(source: string, selectedText: string): { startLine: number; endLine: number } | null {
  const normalizedSource = String(source || "").replace(/\r\n/g, "\n");
  const normalizedSelection = selectedText.replace(/\r\n/g, "\n");
  const index = normalizedSource.indexOf(normalizedSelection);
  if (index < 0) return null;
  if (normalizedSource.indexOf(normalizedSelection, index + Math.max(1, normalizedSelection.length)) >= 0) return null;
  const before = normalizedSource.slice(0, index);
  const startLine = before.split("\n").length;
  const selectedLineCount = Math.max(1, normalizedSelection.split("\n").length);
  return { startLine, endLine: startLine + selectedLineCount - 1 };
}

export function relativePathFromWorkspace(filePath: string, workspacePath: string) {
  const normalizedFilePath = normalizePath(filePath);
  const normalizedWorkspacePath = normalizePath(workspacePath).replace(/\/+$/, "");
  if (!normalizedWorkspacePath) return normalizedFilePath;
  const lowerFilePath = normalizedFilePath.toLowerCase();
  const lowerWorkspacePath = normalizedWorkspacePath.toLowerCase();
  if (lowerFilePath === lowerWorkspacePath) return titleFromPath(normalizedFilePath);
  const prefix = `${lowerWorkspacePath}/`;
  if (lowerFilePath.startsWith(prefix)) {
    return normalizedFilePath.slice(normalizedWorkspacePath.length + 1);
  }
  return normalizedFilePath;
}

export function languageIdFromTab(tab: FileTab) {
  return CODE_LANGUAGE_BY_EXTENSION[tab.extension] || tab.extension || tab.kind || "text";
}

export function formatLineSuffix(startLine?: number, endLine?: number) {
  if (!startLine) return "";
  if (endLine && endLine > startLine) return `:${startLine}-${endLine}`;
  return `:${startLine}`;
}

export function hashText(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16);
}

export function fileKindFromPath(path: string) {
  const mediaKind = mediaFileKindFromPath(path);
  if (mediaKind) return mediaKind;
  const extension = fileExtensionKeyFromPath(path);
  if (isUnsupportedFileExtension(extension)) return "unsupported";
  if (["md", "markdown", "mdx"].includes(extension)) return "markdown";
  // HTML 单列一类：右栏按沙箱渲染，源码是它的备用视图。
  if (["html", "htm"].includes(extension)) return "html";
  return "code";
}

export function mediaFileKindFromPath(path: string): Exclude<FileReaderFileKind, "markdown" | "code" | "unsupported"> | "" {
  const extension = rawExtensionFromPath(path);
  if (IMAGE_FILE_EXTENSIONS.has(extension)) return "image";
  if (AUDIO_FILE_EXTENSIONS.has(extension)) return "audio";
  if (VIDEO_FILE_EXTENSIONS.has(extension)) return "video";
  return "";
}

export function isPreviewMediaKind(kind: string) {
  return kind === "image" || kind === "audio" || kind === "video";
}

export function isTextFileKind(kind: string) {
  // html 也要按文本读：源码视图与沙箱渲染都依赖整段原文。
  return kind === "markdown" || kind === "code" || kind === "html";
}

export function isUnsupportedFileExtension(extension: string) {
  return UNSUPPORTED_MEDIA_EXTENSIONS.has(extension) || UNSUPPORTED_BINARY_EXTENSIONS.has(extension);
}

export function isUnsupportedFilePath(path: string) {
  return fileKindFromPath(path) === "unsupported";
}

export function rawExtensionFromPath(path: string) {
  const fileName = titleFromPath(path);
  const dotIndex = fileName.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return "";
  return fileName.slice(dotIndex + 1).toLowerCase();
}

export function fileExtensionKeyFromPath(path: string) {
  const fileName = titleFromPath(path);
  const lowerFileName = fileName.toLowerCase();
  if (CODE_LANGUAGE_BY_EXTENSION[lowerFileName]) return lowerFileName;
  if (SHIKI_LANGUAGE_KEYS.has(lowerFileName)) return lowerFileName;
  return rawExtensionFromPath(path);
}

export function extensionFromPath(path: string) {
  return fileExtensionKeyFromPath(path);
}

export function escapeHtml(value: string) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function stripMarkdownHtmlComments(value: string) {
  return String(value || "").replace(/<!--[\s\S]*?-->/g, "");
}

export function resolveShikiLanguage(extension: string) {
  const key = String(extension || "").trim().toLowerCase();
  const mapped = CODE_LANGUAGE_BY_EXTENSION[key] || key;
  return SHIKI_LANGUAGE_KEYS.has(mapped) || CUSTOM_SHIKI_LANGUAGE_IDS.has(mapped) ? mapped : "text";
}

export function titleFromPath(path: string) {
  const normalized = normalizePath(path);
  return normalized.split("/").filter(Boolean).pop() || normalized || "未命名文件";
}

export function directoryFromPath(path: string) {
  const normalized = normalizePath(path).replace(/\/+$/, "");
  const slashIndex = normalized.lastIndexOf("/");
  if (slashIndex < 0) return "";
  if (slashIndex === 0) return "/";
  if (slashIndex === 2 && normalized[1] === ":") return normalized.slice(0, 3);
  return normalized.slice(0, slashIndex);
}

function normalizeDirectoryPath(path: string) {
  const normalized = normalizePath(path);
  if (normalized === "/") return "/";
  const trimmed = normalized.replace(/\/+$/, "");
  if (/^[A-Za-z]:$/.test(trimmed)) return `${trimmed}/`;
  return trimmed;
}

function joinDirectoryPath(base: string, segment: string) {
  if (!base || base === "/") return `/${segment}`;
  return base.endsWith("/") ? `${base}${segment}` : `${base}/${segment}`;
}

export function directoryPathChain(rootPath: string, targetDirectoryPath: string) {
  const root = normalizeDirectoryPath(rootPath);
  const target = normalizeDirectoryPath(targetDirectoryPath);
  if (!root || !target) return [];
  const rootLower = root.toLowerCase();
  const targetLower = target.toLowerCase();
  const childPrefix = root.endsWith("/") ? rootLower : `${rootLower}/`;
  if (targetLower !== rootLower && !targetLower.startsWith(childPrefix)) return [];
  if (targetLower === rootLower) return [root];

  const suffixStart = root.length + (root.endsWith("/") ? 0 : 1);
  const suffixParts = target.slice(suffixStart).split("/").filter(Boolean);
  const chain = [root];
  let current = root;
  for (const part of suffixParts) {
    current = joinDirectoryPath(current, part);
    chain.push(current);
  }
  return chain;
}

export function normalizeDirectoryEntries(entries: FileReaderDirectoryEntry[]) {
  return entries.map((entry) => ({
    ...entry,
    path: normalizePath(entry.path),
    name: String(entry.name || titleFromPath(entry.path)),
    isDirectory: !!entry.isDirectory,
  }));
}
