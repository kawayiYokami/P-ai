const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const vscode = require("vscode");

const DISCOVERY_FILE = "p-ai-ide-context-bridge.json";
const IDE_CONTEXT_CLIENT_ID = `vscode-${process.pid}`;
const MAX_CONTEXT_LINES = 240;
const MAX_CONTEXT_CHARS = 40000;
const CONTEXT_TEXT_BLOCK_CONTENT_LIMIT = 2000;
const IDE_CONTEXT_DEBOUNCE_MS = 200;
const IDE_CONTEXT_HEARTBEAT_MS = 10000;

function readIdeContextConfig() {
  const config = vscode.workspace.getConfiguration("paiSidebar");
  return {
    autoSendIdeContext: Boolean(config.get("autoSendIdeContext", true)),
  };
}

function readDiscovery() {
  const discoveryPath = path.join(os.tmpdir(), DISCOVERY_FILE);
  try {
    const raw = fs.readFileSync(discoveryPath, "utf8");
    const parsed = JSON.parse(raw);
    const chatUrl = String(parsed.chatUrl || parsed.url || "").replace(/\/ide-context$/, "/chat");
    const token = String(parsed.token || "");
    if (!chatUrl) return null;
    return { ...parsed, chatUrl, token };
  } catch {
    return null;
  }
}

function readDiscoveryFileContent() {
  const discoveryPath = path.join(os.tmpdir(), DISCOVERY_FILE);
  try {
    return fs.readFileSync(discoveryPath, "utf8");
  } catch {
    return "";
  }
}

function canonicalWorkspaceRootForExtension(pathValue) {
  const raw = String(pathValue || "").trim();
  if (!raw) return "";
  // 与前端 canonicalWorkspaceRootForComparison 同构：先归一化再回溯 .pai/.worktree。
  let normalized = raw.replace(/^\\\\\?\\unc\\/i, "//").replace(/^\\\\\?\\/i, "").replace(/^\\\\.\\/i, "");
  normalized = normalized.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  if (!normalized) return raw;
  for (const marker of ["/.pai/.worktree/", "/.pai/worktree/"]) {
    const markerIndex = normalized.indexOf(marker);
    if (markerIndex !== -1) {
      // 用归一化后的切片长度反推原始前缀截断：保证 host 与会话 canonical 一致。
      return normalized.slice(0, markerIndex);
    }
  }
  for (const suffix of ["/.pai/.worktree", "/.pai/worktree"]) {
    if (normalized.endsWith(suffix)) return normalized.slice(0, normalized.length - suffix.length);
  }
  return raw;
}

function readWorkspaceRoots() {
  return (vscode.workspace.workspaceFolders || []).map((folder) => ({
    path: canonicalWorkspaceRootForExtension(folder.uri.fsPath),
    name: folder.name,
  }));
}

function nowIso() {
  return new Date().toISOString();
}

function normalizeRangeLines(startLine, endLine) {
  const start = Math.max(0, Math.min(startLine, endLine));
  const end = Math.max(start, Math.max(startLine, endLine));
  return { start, end };
}

function workspaceForFile(filePath) {
  const uri = vscode.Uri.file(filePath);
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  if (folder) {
    return { path: folder.uri.fsPath, name: folder.name };
  }
  const root = readWorkspaceRoots()[0];
  return root || { path: "", name: "" };
}

function relativePathForFile(filePath, workspacePath) {
  if (!workspacePath) return path.basename(filePath);
  const relative = path.relative(workspacePath, filePath);
  if (!relative || relative.startsWith("..")) return path.basename(filePath);
  return relative.replace(/\\/g, "/");
}

function lineSuffix(startLine, endLine) {
  if (startLine > 0 && endLine > startLine) return `:${startLine}-${endLine}`;
  if (startLine > 0) return `:${startLine}`;
  return "";
}

function textBlockForReference(reference) {
  const lineText = reference.endLine && reference.endLine > reference.startLine
    ? `${reference.startLine}-${reference.endLine}`
    : String(reference.startLine || reference.endLine || "");
  const location = `${reference.filePath}${lineText ? `:${lineText}` : ""}`;
  const content = String(reference.content || "");
  if (content.length > CONTEXT_TEXT_BLOCK_CONTENT_LIMIT) {
    return `用户引用了文件片段：${location}`;
  }
  const lines = ["[IDE 上下文引用]", `文件: ${reference.filePath}`];
  if (reference.startLine || reference.endLine) {
    if (lineText) lines.push(`行号: ${lineText}`);
  }
  if (reference.languageId) lines.push(`语言: ${reference.languageId}`);
  if (reference.source) lines.push(`来源: ${reference.source}`);
  if (reference.capturedAt) lines.push(`采集时间: ${reference.capturedAt}`);
  lines.push("内容:");
  lines.push(content);
  return lines.join("\n");
}

function readDocumentLineRange(document, startLine, endLine) {
  if (!document || document.lineCount <= 0) return "";
  const normalized = normalizeRangeLines(startLine, endLine);
  const start = Math.min(normalized.start, document.lineCount - 1);
  const end = Math.min(normalized.end, Math.min(document.lineCount - 1, start + MAX_CONTEXT_LINES - 1));
  const endCharacter = document.lineAt(end).range.end.character;
  return document.getText(new vscode.Range(new vscode.Position(start, 0), new vscode.Position(end, endCharacter))).slice(0, MAX_CONTEXT_CHARS);
}

function createReference(document, source, startLineZeroBased, endLineZeroBased, content, capturedAt) {
  const filePath = document.uri.fsPath;
  if (!filePath || document.uri.scheme !== "file") return null;
  const workspace = workspaceForFile(filePath);
  const relativePath = relativePathForFile(filePath, workspace.path);
  const fileName = path.basename(filePath);
  const hasLineRange = source !== "active_file";
  const startLine = hasLineRange ? Math.max(1, startLineZeroBased + 1) : undefined;
  const endLine = hasLineRange ? Math.max(startLine, endLineZeroBased + 1) : undefined;
  const reference = {
    id: hasLineRange ? `${source}:${filePath}:${startLine}:${endLine}` : `${source}:${filePath}`,
    workspacePath: workspace.path,
    workspaceName: workspace.name || path.basename(workspace.path || "") || "Workspace",
    filePath,
    fileName,
    relativePath,
    startLine,
    endLine,
    displayLabel: `${fileName}${lineSuffix(startLine, endLine)}`,
    content: String(content || "").trim(),
    languageId: document.languageId || undefined,
    source,
    capturedAt,
    textBlock: "",
  };
  if (!reference.content && source !== "active_file") return null;
  reference.textBlock = textBlockForReference(reference);
  return reference;
}

function collectIdeContextReferences() {
  const capturedAt = nowIso();
  const referencesByFile = new Map();
  const visibleEditors = vscode.window.visibleTextEditors || [];
  for (const editor of visibleEditors) {
    const document = editor.document;
    if (!document || document.uri.scheme !== "file") continue;
    let reference = null;
    const selection = editor.selection;
    if (selection && !selection.isEmpty) {
      const start = selection.start.isBefore(selection.end) ? selection.start : selection.end;
      const end = selection.end.isAfter(selection.start) ? selection.end : selection.start;
      const content = document.getText(new vscode.Range(start, end)).slice(0, MAX_CONTEXT_CHARS);
      reference = createReference(document, "selection", start.line, end.line, content, capturedAt);
    }
    if (!reference) {
      reference = createReference(document, "active_file", 0, 0, "", capturedAt);
    }
    if (!reference) continue;
    const fileKey = path.resolve(reference.filePath).replace(/\\/g, "/").toLowerCase();
    const existing = referencesByFile.get(fileKey);
    if (!existing || (reference.source === "selection" && existing.source !== "selection")) {
      referencesByFile.set(fileKey, reference);
    }
  }
  return Array.from(referencesByFile.values());
}

function ideContextSignature(references) {
  return references
    .map((reference) => [
      reference.source,
      reference.filePath,
      reference.startLine || 0,
      reference.endLine || 0,
      reference.content,
    ].join("\u001f"))
    .sort()
    .join("\u001e");
}

function snapshotPayloadFromReferences(references, discovery) {
  return {
    clientId: IDE_CONTEXT_CLIENT_ID,
    authToken: String(discovery?.token || ""),
    editor: "vscode",
    workspaceRoots: readWorkspaceRoots().map((root) => root.path).filter(Boolean),
    references: references.map((reference) => ({
      id: reference.id,
      filePath: reference.filePath,
      startLine: reference.startLine,
      endLine: reference.endLine,
      content: reference.content,
      languageId: reference.languageId,
      source: reference.source,
      capturedAt: reference.capturedAt,
    })),
    updatedAt: nowIso(),
  };
}

function nonce() {
  return Array.from({ length: 16 }, () => Math.floor(Math.random() * 16).toString(16)).join("");
}

function readSidebarAssets(extensionUri) {
  const candidates = [
    path.join(extensionUri.fsPath, "dist"),
    path.resolve(extensionUri.fsPath, "..", "..", "..", "..", "dist"),
  ];
  const sidebarDistPath = candidates.find((dir) => fs.existsSync(path.join(dir, "sidebar.html")));
  if (!sidebarDistPath) return { scripts: [], styles: [], localResourceRoots: [] };
  const sidebarHtmlPath = path.join(sidebarDistPath, "sidebar.html");
  try {
    const html = fs.readFileSync(sidebarHtmlPath, "utf8");
    const scripts = Array.from(html.matchAll(/<script[^>]+src="([^"]+\.js)"[^>]*>/g))
      .map((match) => match[1]);
    const styles = Array.from(html.matchAll(/<link[^>]+href="([^"]+\.css)"[^>]*>/g))
      .map((match) => match[1]);
    const distUri = vscode.Uri.file(sidebarDistPath);
    return {
      scripts: scripts.map((asset) => vscode.Uri.joinPath(distUri, asset.replace(/^\//, ""))),
      styles: styles.map((asset) => vscode.Uri.joinPath(distUri, asset.replace(/^\//, ""))),
      localResourceRoots: [distUri],
    };
  } catch {
    return { scripts: [], styles: [], localResourceRoots: [] };
  }
}

function safeDecode(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function normalizeLocalReference(rawHref) {
  let href = String(rawHref || "").trim();
  if (!href) return "";
  if ((href.startsWith("<") && href.endsWith(">")) || (href.startsWith("\"") && href.endsWith("\""))) {
    href = href.slice(1, -1).trim();
  }
  if (/^https?:\/\//i.test(href)) return "";
  if (/^file:/i.test(href)) {
    try {
      const url = new URL(href);
      const decodedPath = safeDecode(url.pathname || "");
      if (url.host && url.host !== "localhost") {
        return `\\\\${url.host}${decodedPath.replace(/\//g, "\\")}`;
      }
      return decodedPath.replace(/^\/([A-Za-z]:)/, "$1");
    } catch {
      return safeDecode(href);
    }
  }
  return safeDecode(href).replace(/%5C/gi, "\\");
}

function parseLocalFileReference(rawHref) {
  const normalized = normalizeLocalReference(rawHref);
  if (!normalized) return null;
  const match = normalized.match(/^(.*?)(?::(\d+))(?::(\d+))?$/);
  const filePath = (match ? match[1] : normalized).trim();
  if (!/^[A-Za-z]:[\\/]/.test(filePath) && !filePath.startsWith("\\\\") && !filePath.startsWith("/")) {
    return null;
  }
  const line = match && match[2] ? Math.max(Number.parseInt(match[2], 10), 1) : null;
  const column = match && match[3] ? Math.max(Number.parseInt(match[3], 10), 1) : null;
  return { filePath, line, column };
}

async function openLocalFileReference(rawHref) {
  const reference = parseLocalFileReference(rawHref);
  if (!reference) {
    void vscode.window.showWarningMessage("无法识别文件引用。");
    return;
  }
  const uri = vscode.Uri.file(reference.filePath);
  const options = { preview: true };
  if (reference.line !== null) {
    const position = new vscode.Position(reference.line - 1, (reference.column || 1) - 1);
    options.selection = new vscode.Range(position, position);
  }
  try {
    await vscode.window.showTextDocument(uri, options);
  } catch (error) {
    if (reference.line !== null) throw error;
    await vscode.commands.executeCommand("vscode.open", uri);
  }
}

async function openExternalUrl(rawUrl) {
  const url = String(rawUrl || "").trim();
  if (!/^https?:\/\//i.test(url)) {
    void vscode.window.showWarningMessage("无法识别外部链接。");
    return;
  }
  await vscode.env.openExternal(vscode.Uri.parse(url));
}

class PaiSidebarProvider {
  constructor(extensionUri) {
    this.extensionUri = extensionUri;
    this.view = null;
    this._discoveryWatcher = null;
    this._lastDiscoveryContent = "";
    this._ideContextTimer = null;
    this._ideContextSendTimer = null;
    this._lastIdeContextSignature = "";
    this._lastIdeContextPayload = null;
    this._lastIdeContextSentAt = 0;
    this._ideContextDirty = true;
    this._subscriptions = [];
  }

  resolveWebviewView(webviewView) {
    this.view = webviewView;
    const webview = webviewView.webview;
    const assets = readSidebarAssets(this.extensionUri);
    webview.options = {
      enableScripts: true,
      localResourceRoots: [
        ...assets.localResourceRoots,
      ],
    };
    webview.onDidReceiveMessage((message) => {
      if (message && message.type === "pai-refresh-discovery") {
        this.postDiscovery();
        return;
      }
      if (message && message.type === "pai-open-file") {
        openLocalFileReference(message.href).catch((error) => {
          const detail = error && error.message ? error.message : String(error);
          void vscode.window.showWarningMessage(`打开文件引用失败：${detail}`);
        });
        return;
      }
      if (message && message.type === "pai-open-url") {
        openExternalUrl(message.url).catch((error) => {
          const detail = error && error.message ? error.message : String(error);
          void vscode.window.showWarningMessage(`打开外部链接失败：${detail}`);
        });
        return;
      }
    });
    webview.html = this.html(webview, assets);
    this.postDiscovery();
    this._startDiscoveryWatcher();
    this._startIdeContextSync();
  }

  _startDiscoveryWatcher() {
    if (this._discoveryWatcher) return;
    const discoveryPath = path.join(os.tmpdir(), DISCOVERY_FILE);
    this._lastDiscoveryContent = readDiscoveryFileContent();
    let debounceTimer = null;
    try {
      this._discoveryWatcher = fs.watch(discoveryPath, () => {
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
          const content = readDiscoveryFileContent();
          if (content !== this._lastDiscoveryContent) {
            this._lastDiscoveryContent = content;
            this.postDiscovery();
            this.markIdeContextDirty();
            this.scheduleIdeContextSync();
          }
        }, 300);
      });
    } catch {
      // 文件不存在时 watcher 无法启动，等下次 postDiscovery 时重试
    }
  }

  _stopDiscoveryWatcher() {
    if (this._discoveryWatcher) {
      this._discoveryWatcher.close();
      this._discoveryWatcher = null;
    }
  }

  _startIdeContextSync() {
    if (this._ideContextTimer) return;
    this._subscriptions.push(
      vscode.window.onDidChangeActiveTextEditor(() => {
        this.markIdeContextDirty();
        this.scheduleIdeContextSync();
      }),
      vscode.window.onDidChangeVisibleTextEditors(() => {
        this.markIdeContextDirty();
        this.scheduleIdeContextSync();
      }),
      vscode.window.onDidChangeTextEditorSelection(() => {
        this.markIdeContextDirty();
        this.scheduleIdeContextSync();
      }),
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (!event.affectsConfiguration("paiSidebar")) return;
        this.markIdeContextDirty();
        this.scheduleIdeContextSync();
      }),
    );
    this.scheduleIdeContextSync();
    this._ideContextTimer = setInterval(() => this.syncIdeContext({ heartbeatOnly: true }), 5000);
  }

  _stopIdeContextSync() {
    if (this._ideContextTimer) {
      clearInterval(this._ideContextTimer);
      this._ideContextTimer = null;
    }
    if (this._ideContextSendTimer) {
      clearTimeout(this._ideContextSendTimer);
      this._ideContextSendTimer = null;
    }
    for (const subscription of this._subscriptions.splice(0)) {
      try { subscription.dispose(); } catch {}
    }
  }

  scheduleIdeContextSync() {
    const config = readIdeContextConfig();
    if (!config.autoSendIdeContext) return;
    if (this._ideContextSendTimer) clearTimeout(this._ideContextSendTimer);
    this._ideContextSendTimer = setTimeout(() => {
      this._ideContextSendTimer = null;
      this.syncIdeContext();
    }, IDE_CONTEXT_DEBOUNCE_MS);
  }

  markIdeContextDirty() {
    this._ideContextDirty = true;
  }

  syncIdeContext(options = {}) {
    const heartbeatOnly = Boolean(options.heartbeatOnly);
    const config = readIdeContextConfig();
    if (!config.autoSendIdeContext) return;
    const now = Date.now();
    const heartbeatDue = this._lastIdeContextPayload && now - this._lastIdeContextSentAt >= IDE_CONTEXT_HEARTBEAT_MS;
    if (heartbeatOnly && !heartbeatDue) return;
    const discovery = readDiscovery();
    if (heartbeatOnly && !this._ideContextDirty && heartbeatDue) {
      if (!discovery) return;
      this._lastIdeContextPayload = {
        ...this._lastIdeContextPayload,
        updatedAt: nowIso(),
      };
      this.sendIdeContextSnapshot(this._lastIdeContextPayload);
      return;
    }
    const references = collectIdeContextReferences();
    const signature = ideContextSignature(references);
    if (signature === this._lastIdeContextSignature && !heartbeatDue) {
      this._ideContextDirty = false;
      return;
    }
    if (!discovery) return;
    const payload = snapshotPayloadFromReferences(references, discovery);
    this._lastIdeContextSignature = signature;
    this._lastIdeContextPayload = payload;
    this._ideContextDirty = false;
    this.sendIdeContextSnapshot(payload);
  }

  sendIdeContextSnapshot(payload) {
    if (!this.view || !payload) return;
    void this.view.webview.postMessage({
      type: "pai-ide-context-snapshot",
      snapshot: payload,
    });
    this._lastIdeContextSentAt = Date.now();
  }

  postDiscovery(discoveryInput) {
    const discovery = discoveryInput || readDiscovery();
    if (this.view) {
      void this.view.webview.postMessage({
        type: "pai-discovery",
        discovery: discovery ? { ...discovery, workspaceRoots: readWorkspaceRoots() } : discovery,
      });
    }
  }

  html(webview, preloadedAssets) {
    const scriptNonce = nonce();
    const assets = preloadedAssets || readSidebarAssets(this.extensionUri);
    const scriptUris = assets.scripts.map((asset) => webview.asWebviewUri(asset).toString());
    const styleUris = assets.styles.map((asset) => webview.asWebviewUri(asset).toString());
    const discovery = JSON.stringify({
      ...(readDiscovery() || {}),
      workspaceRoots: readWorkspaceRoots(),
    });
    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; connect-src ws://127.0.0.1:*; img-src ${webview.cspSource} data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}' 'wasm-unsafe-eval';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  ${styleUris.map((uri) => `<link href="${uri}" rel="stylesheet">`).join("\n  ")}
  <title>PAI</title>
</head>
<body>
  <div id="app"></div>
  <script nonce="${scriptNonce}">window.__PAI_SIDEBAR_BRIDGE__ = ${discovery};</script>
  ${
    scriptUris.length > 0
      ? scriptUris.map((uri) => `<script nonce="${scriptNonce}" type="module" src="${uri}"></script>`).join("\n  ")
      : "<p>Pai assets are missing. Run the extension build first.</p>"
  }
</body>
</html>`;
  }
}

let currentProvider = null;

function activate(context) {
  const provider = new PaiSidebarProvider(context.extensionUri);
  currentProvider = provider;
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider("paiSidebar.chatView", provider, {
      webviewOptions: { retainContextWhenHidden: true },
    }),
    vscode.commands.registerCommand("paiSidebar.refresh", () => provider.postDiscovery()),
    {
      dispose: () => {
        provider._stopDiscoveryWatcher();
        provider._stopIdeContextSync();
      },
    },
  );
}

function deactivate() {
  if (currentProvider) {
    currentProvider._stopDiscoveryWatcher();
    currentProvider._stopIdeContextSync();
    // 清空旧 webview HTML，迫使新 provider 注册时 VS Code 重新调用 resolveWebviewView
    if (currentProvider.view) {
      try { currentProvider.view.webview.html = ""; } catch {}
    }
    currentProvider = null;
  }
}

module.exports = { activate, deactivate };
