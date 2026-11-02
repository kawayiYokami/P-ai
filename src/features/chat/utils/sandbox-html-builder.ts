// ==================== 交互式沙箱 HTML 构建 ====================
// 为 LLM 生成的 HTML 注入 CSP、主题变量与 Bridge 通信脚本，
// 产出可直接交给 iframe srcdoc 的完整文档。

/** 可交互沙箱的代码块语言标记，如 html:interactive / html:preview / html:widget */
export const INTERACTIVE_HTML_LANG_PATTERN = /^html:(?:interactive|preview|widget)$/i;

export function isInteractiveHtmlLang(lang: string): boolean {
  return INTERACTIVE_HTML_LANG_PATTERN.test(String(lang || "").trim());
}

/** 普通 html 代码块可通过「交互预览」升级为沙箱运行 */
export function isPlainHtmlLang(lang: string): boolean {
  return /^html?$/i.test(String(lang || "").trim());
}

export const SANDBOX_MESSAGE_TYPE_RESIZE = "pai-sandbox:resize";
export const SANDBOX_MESSAGE_TYPE_ERROR = "pai-sandbox:error";
export const SANDBOX_MESSAGE_TYPE_READY = "pai-sandbox:ready";

export const SANDBOX_MIN_HEIGHT = 120;
export const SANDBOX_MAX_HEIGHT = 640;

// ==================== 网址模式：只允许本机回环 ====================
// 沙箱卡片除了渲染内联 html，还能直接加载本机起的前端（开发服务器预览）。
// 这条路径放宽了 sandbox（见 InteractiveHtmlSandbox 的注释），所以入口必须收死：
// 只认 http + loopback，外网地址、file://、javascript: 一律不加载。

/** 网址模式允许的主机名；端口与路径不限 */
const SANDBOX_URL_ALLOWED_HOSTS = new Set(["127.0.0.1", "localhost"]);

/**
 * 归一化网址模式的输入。
 *
 * 没写协议时按 http 补全（`localhost:1420` 这种粘法很常见），
 * 但只接受 `http://127.0.0.1[:port]` 与 `http://localhost[:port]`。
 * 返回可直接交给 iframe src 的地址；不合法返回 null。
 */
export function normalizeSandboxUrl(raw: string): string | null {
  const value = String(raw || "").trim();
  if (!value) return null;
  // 只有显式写了 `://` 才当作自带协议，避免把 `localhost:1420` 里的 `localhost:` 误判成协议名
  const candidate = value.includes("://") ? value : `http://${value}`;
  let url: URL;
  try {
    url = new URL(candidate);
  } catch {
    return null;
  }
  if (url.protocol !== "http:") return null;
  if (!SANDBOX_URL_ALLOWED_HOSTS.has(url.hostname.toLowerCase())) return null;
  return url.toString();
}

/** 需要从宿主提取并注入沙箱的 DaisyUI 语义色变量 */
const THEME_VARIABLE_NAMES = [
  "--color-base-100",
  "--color-base-200",
  "--color-base-300",
  "--color-base-content",
  "--color-primary",
  "--color-primary-content",
  "--color-secondary",
  "--color-secondary-content",
  "--color-accent",
  "--color-accent-content",
  "--color-neutral",
  "--color-neutral-content",
  "--color-info",
  "--color-info-content",
  "--color-success",
  "--color-success-content",
  "--color-warning",
  "--color-warning-content",
  "--color-error",
  "--color-error-content",
];

/** 从当前文档根元素提取主题变量快照 */
export function collectThemeVariables(root?: HTMLElement | null): Record<string, string> {
  const target = root || (typeof document !== "undefined" ? document.documentElement : null);
  if (!target || typeof getComputedStyle !== "function") return {};
  const style = getComputedStyle(target);
  const tokens: Record<string, string> = {};
  for (const name of THEME_VARIABLE_NAMES) {
    const value = style.getPropertyValue(name).trim();
    if (value) tokens[name] = value;
  }
  return tokens;
}

const SANDBOX_CSP = [
  "default-src 'none'",
  // 脚本：仅内联 + 白名单公共 CDN
  "script-src 'unsafe-inline' https://cdn.jsdelivr.net https://unpkg.com https://cdn.tailwindcss.com https://cdnjs.cloudflare.com",
  // 样式：内联 + Google Fonts + 公共 CDN
  "style-src 'unsafe-inline' https://cdn.jsdelivr.net https://unpkg.com https://cdn.tailwindcss.com https://cdnjs.cloudflare.com https://fonts.googleapis.com",
  "font-src data: https://fonts.gstatic.com https://cdn.jsdelivr.net https://cdnjs.cloudflare.com",
  // 图片仅允许内联数据，防止沙箱内容借网络图片外发数据
  "img-src data: blob:",
  "connect-src 'none'",
  "frame-src 'none'",
  "worker-src blob:",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

/** 注入沙箱内部的 Bridge 脚本：高度上报、错误上报、主题接收 */
function buildBridgeScript(): string {
  return `<script data-pai-bridge="true">
(function () {
  if (window.__paiSandboxBridgeInstalled) return;
  window.__paiSandboxBridgeInstalled = true;
  var MESSAGE_PREFIX = "pai-sandbox:";
  var lastHeight = -1;
  function reportHeight() {
    var doc = document.documentElement;
    var body = document.body;
    // 只测 body 内容高度；documentElement 尺寸受 iframe 高度影响，会回环
    var height = body ? Math.max(body.scrollHeight, body.offsetHeight) : 0;
    if (!isFinite(height) || height <= 0) return;
    var rounded = Math.ceil(height);
    if (rounded === lastHeight) return;
    lastHeight = rounded;
    try {
      window.parent.postMessage({ type: "pai-sandbox:resize", height: rounded }, "*");
    } catch (e) { /* noop */ }
  }
  function reportError(message) {
    try {
      window.parent.postMessage({ type: "pai-sandbox:error", message: String(message || "未知错误") }, "*");
    } catch (e) { /* noop */ }
  }
  window.addEventListener("message", function (event) {
    var data = event && event.data;
    if (!data || typeof data !== "object") return;
    if (data.type === "pai-sandbox:theme" && data.tokens && typeof data.tokens === "object") {
      var rootStyle = document.documentElement.style;
      for (var key in data.tokens) {
        if (Object.prototype.hasOwnProperty.call(data.tokens, key) && key.indexOf("--") === 0) {
          rootStyle.setProperty(key, String(data.tokens[key]));
        }
      }
      if (typeof data.isDark === "boolean") {
        document.documentElement.setAttribute("data-pai-theme", data.isDark ? "dark" : "light");
        if (window.__paiThemeChanged) {
          try { window.__paiThemeChanged(!!data.isDark); } catch (e) { /* noop */ }
        }
      }
      scheduleReport();
      return;
    }
    if (data.type === "pai-sandbox:ping") {
      scheduleReport();
    }
  });
  window.addEventListener("error", function (event) {
    var detail = event && event.message ? event.message : "脚本运行错误";
    if (event && event.filename) detail += " (" + event.filename + ":" + event.lineno + ")";
    reportError(detail);
  }, true);
  window.addEventListener("unhandledrejection", function (event) {
    var reason = event && event.reason;
    reportError("未处理的 Promise 拒绝: " + (reason && reason.message ? reason.message : reason));
  });
  var scheduled = false;
  function scheduleReport() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(function () {
      scheduled = false;
      reportHeight();
    });
  }
  if (typeof ResizeObserver === "function") {
    var observer = new ResizeObserver(scheduleReport);
    var observe = function () {
      // 只观察 body；documentElement 尺寸随 iframe 高度变化，观察它会形成回环
      if (document.body) observer.observe(document.body);
    };
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", observe);
    } else {
      observe();
    }
  }
  window.addEventListener("resize", scheduleReport);
  window.addEventListener("load", function () {
    reportHeight();
    setTimeout(reportHeight, 60);
    setTimeout(reportHeight, 240);
  });
  document.addEventListener("DOMContentLoaded", reportHeight);
  try {
    window.parent.postMessage({ type: "pai-sandbox:ready" }, "*");
  } catch (e) { /* noop */ }
  scheduleReport();
})();
<\/script>`;
}

function buildThemeStyle(tokens: Record<string, string>, isDark: boolean): string {
  const lines = Object.entries(tokens)
    .map(([name, value]) => `  ${name}: ${value};`)
    .join("\n");
  return `<style data-pai-theme="true">
:root {
  color-scheme: ${isDark ? "dark" : "light"};
${lines}
}
html, body {
  margin: 0;
  padding: 0;
  background: transparent;
  color: var(--color-base-content, inherit);
  font-family: inherit;
}
/* 沙箱嵌在聊天气泡内，外层已有留白，内容默认贴边、由组件内部自行控制间距 */
</style>`;
}

function injectIntoHead(html: string, payload: string): string {
  const headMatch = html.match(/<head[^>]*>/i);
  if (headMatch) {
    const index = (headMatch.index || 0) + headMatch[0].length;
    return `${html.slice(0, index)}\n${payload}${html.slice(index)}`;
  }
  const htmlMatch = html.match(/<html[^>]*>/i);
  if (htmlMatch) {
    const index = (htmlMatch.index || 0) + htmlMatch[0].length;
    return `${html.slice(0, index)}\n<head>\n${payload}\n</head>${html.slice(index)}`;
  }
  return `${payload}\n${html}`;
}

function injectCsp(html: string): string {
  const cspMeta = `<meta http-equiv="Content-Security-Policy" content="${SANDBOX_CSP}">`;
  const existing = html.match(/<meta[^>]+http-equiv=["']?Content-Security-Policy["']?[^>]*>/i);
  if (existing) {
    return html.replace(existing[0], cspMeta);
  }
  return injectIntoHead(html, cspMeta);
}

function injectBridge(html: string, bridgeScript: string): string {
  const bodyClose = html.match(/<\/body\s*>/i);
  if (bodyClose) {
    const index = bodyClose.index || 0;
    return `${html.slice(0, index)}\n${bridgeScript}\n${html.slice(index)}`;
  }
  return `${html}\n${bridgeScript}`;
}

/**
 * 构建沙箱 srcdoc：补齐文档骨架 → 注入 CSP → 注入主题变量 → 注入 Bridge 脚本。
 */
export function buildSandboxSrcdoc(rawCode: string, tokens: Record<string, string>, isDark: boolean): string {
  const code = String(rawCode || "").trim();
  if (!code) return "";
  const hasDocument = /<html[\s>]/i.test(code) || /<!doctype/i.test(code);
  let html = hasDocument
    ? code
    : `<!DOCTYPE html>\n<html>\n<head><meta charset="utf-8"></head>\n<body>\n${code}\n</body>\n</html>`;
  html = injectCsp(html);
  html = injectIntoHead(html, buildThemeStyle(tokens, isDark));
  html = injectBridge(html, buildBridgeScript());
  return html;
}
