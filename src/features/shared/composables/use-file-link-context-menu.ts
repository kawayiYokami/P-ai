import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  isAbsoluteLocalPath,
  isAssistantSpacePath,
  normalizeLocalLinkHref,
  parseLocalFileReference,
} from "../../chat/utils/local-link";

export type FileLinkTarget = {
  kind: "file";
  path: string;
  line?: number;
  column?: number;
  label: string;
  /** 链接原文（Markdown 里写的那个 href），复制动作原样输出。 */
  raw: string;
};

export type ExternalLinkTarget = {
  kind: "url";
  url: string;
  label: string;
};

/** 右键菜单的目标：本机文件链接或网络链接。 */
export type LinkMenuTarget = FileLinkTarget | ExternalLinkTarget;

export type FileLinkMenuState = LinkMenuTarget & {
  x: number;
  y: number;
};

type FileLinkContextMenuOptions = {
  /** 相对路径的解析基准（当前工作区根），与左键打开口径保持一致。 */
  workspaceRoot?: () => string;
};

type FileLinkAnchorLike = Pick<HTMLAnchorElement, "getAttribute" | "textContent">;

/** 带协议的链接（`mailto:`、`https:` 等）；Windows 盘符 `E:/` 不算协议。 */
function isSchemeHref(href: string): boolean {
  if (/^[A-Za-z]:[\\/]/.test(href)) return false;
  return /^[A-Za-z][A-Za-z0-9+.-]*:/.test(href);
}

/**
 * 从锚点解析文件链接：只认 Markdown 渲染写给本地链接的 data-href，
 * 外链（写的是 href）与站内锚点都不进本菜单。
 */
export function resolveFileLinkReference(anchor: FileLinkAnchorLike | null, workspaceRoot: string) {
  if (!anchor) return null;
  const rawHref = (anchor.getAttribute("data-href") || "").trim();
  if (!rawHref) return null;
  const href = normalizeLocalLinkHref(rawHref);
  // 站内锚点（`#x`）、带协议的链接（`mailto:` 等）都不是本机文件路径，
  // 与左键打开口径保持一致；盘符路径不适用这条。
  if (!href || href.startsWith("#") || isSchemeHref(href)) return null;
  const reference = parseLocalFileReference(href);
  let path = reference?.path || href;
  if (!isAbsoluteLocalPath(path) && !isAssistantSpacePath(path)) {
    const root = String(workspaceRoot || "").trim().replace(/\\/g, "/").replace(/\/$/, "");
    if (root) path = `${root}/${path.replace(/^\.\//, "")}`;
  }
  return {
    path,
    line: reference?.line,
    column: reference?.column,
    label: (anchor.textContent || path).trim() || path,
    raw: rawHref,
  };
}

/**
 * 从锚点解析网络链接：Markdown 渲染给外链写的是 href，本地链接写的是 data-href。
 */
export function resolveExternalLinkUrl(anchor: FileLinkAnchorLike | null): string {
  if (!anchor) return "";
  const rawHref = String(anchor.getAttribute("href") || "").trim();
  return /^https?:\/\//i.test(rawHref) ? rawHref : "";
}

/**
 * 菜单目标判定：先按本机文件链接解析，再退到网络链接；都不命中交给外层容器的右键菜单。
 */
export function resolveLinkMenuTarget(
  anchor: FileLinkAnchorLike | null,
  workspaceRoot: string,
): LinkMenuTarget | null {
  const file = resolveFileLinkReference(anchor, workspaceRoot);
  if (file) return { kind: "file", ...file };
  const url = resolveExternalLinkUrl(anchor);
  if (!url) return null;
  const label = (anchor?.textContent || url).trim() || url;
  return { kind: "url", url, label };
}

/**
 * 给所有 Markdown 文件链接挂右键菜单。
 * 用捕获阶段监听：文件链接的菜单必须先于外层容器（如消息级）右键菜单拿到事件。
 */
export function useFileLinkContextMenu(options: FileLinkContextMenuOptions = {}) {
  const state = ref<FileLinkMenuState | null>(null);

  function handleContextMenu(event: MouseEvent) {
    const target = event.target as Element | null;
    const anchor = target?.closest?.("a") as HTMLAnchorElement | null;
    const resolved = resolveLinkMenuTarget(
      anchor,
      String(options.workspaceRoot?.() || ""),
    );
    // 非文件/网络链接：收起已有菜单，但不拦截事件，交给外层容器的右键菜单。
    if (!resolved) {
      state.value = null;
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const width = 208;
    const height = 208;
    const margin = 8;
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
    const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
    state.value = {
      ...resolved,
      x: Math.max(margin, Math.min(event.clientX, viewportWidth - width - margin)),
      y: Math.max(margin, Math.min(event.clientY, viewportHeight - height - margin)),
    };
  }

  onMounted(() => window.addEventListener("contextmenu", handleContextMenu, true));
  onBeforeUnmount(() => window.removeEventListener("contextmenu", handleContextMenu, true));

  function close() {
    state.value = null;
  }

  return { state, close };
}
