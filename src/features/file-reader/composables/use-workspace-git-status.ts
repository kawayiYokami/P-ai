// 工作区 Git 状态共享源（模块级单例）
// 目的：文件阅读器 Git 面板与右栏卡片墙的 Git 卡共用同一份更改数据，避免两边各存一份导致显示漂移。
// 职责：
// - 独占 git_panel_status 取数与刷新冷却：两个消费方同时触发也只发一次请求
// - 独占仓库监听（git_panel_watch_start / git_panel_watch_stop）的启停，按消费方引用计数
// - 独占 gitPanel.watchChanged 订阅，过滤出属于当前仓库的事件后驱动刷新，并转发给订阅者
// 共享范围是「分支 + 更改列表 + 最近几条提交」；其中最近提交只给卡片墙用，
// 面板的完整分页历史、stashes、branches、diff 仍由 Git 面板自理，两边不读写同一份列表。
import { ref } from "vue";
import {
  appendTransportProbeLog,
  gitPanelDiscover,
  gitPanelLog,
  gitPanelStatus,
  gitPanelWatchStart,
  gitPanelWatchStop,
  onTransportNotification,
  type GitPanelLogEntry,
  type GitPanelStatusEntry,
  type GitPanelStatusOutput,
  type GitPanelWatchEventPayload,
} from "../../../services/tauri-api";

/** 排障前缀：运行日志里按这个标记过滤 git 监视链路。 */
const GIT_WATCH_PROBE_PREFIX = "[Git监视诊断]";
/** 事件到达时刻的窗口状态：判断「离开前台期间事件是否真的到了本窗口」的关键字段。 */
function gitWatchProbeContext(): Record<string, unknown> {
  const visibility = typeof document !== "undefined" ? document.visibilityState || "" : "";
  const focused = typeof document !== "undefined" && typeof document.hasFocus === "function"
    ? document.hasFocus()
    : false;
  const pathname = typeof window !== "undefined" ? window.location?.pathname || "" : "";
  return { page: pathname, visibility, focused };
}

function gitWatchProbe(tag: string, data: Record<string, unknown> = {}) {
  appendTransportProbeLog(GIT_WATCH_PROBE_PREFIX, tag, { ...gitWatchProbeContext(), ...data }, "debug");
}

/** 自动刷新冷却：1 秒内重复触发只发一次请求；force 可穿透 */
const REFRESH_CD_MS = 1000;
/** 卡片墙的提交卡只看最近几条，与 Git 面板的分页历史互不影响 */
const RECENT_COMMITS_LIMIT = 5;

// ==================== 共享状态 ====================
/** 当前仓库根：由 Git 面板选中决定，其他消费方跟随 */
const repoRoot = ref("");
const currentBranch = ref("");
const statusEntries = ref<GitPanelStatusEntry[]>([]);
/** 变更条目超过后端返回上限（1000）时为 true */
const statusTruncated = ref(false);
const stagedTotal = ref(0);
const unstagedTotal = ref(0);
const statusLoaded = ref(false);
const statusError = ref("");
/** 最近几条提交：只给卡片墙用，与 Git 面板自己那份分页历史无关 */
const recentCommits = ref<GitPanelLogEntry[]>([]);

/** 需要实时数据的消费方数量，归零时关闭仓库监听与事件订阅 */
let consumerCount = 0;
/** Git 面板挂载数量：面板在时仓库根由面板决定，其他消费方只跟随 */
let panelCount = 0;
/** 当前已向后端注册监听的仓库 */
let watchedRoot = "";
let unlistenWatch: (() => void) | null = null;
let lastStatusLoad = 0;
let loadSeq = 0;
const externalChangeHandlers = new Set<(payload: GitPanelWatchEventPayload) => void>();

function normalizeRepoPath(path: string): string {
  return String(path || "").replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

/** 同一仓库仅大小写或斜杠差异时视为同一个 */
export function isSameRepoPath(a: string, b: string): boolean {
  const left = normalizeRepoPath(a);
  const right = normalizeRepoPath(b);
  return !!left && !!right && left === right;
}

// ==================== 会话级仓库选择记忆 ====================
// 探测出的默认仓库是「当前工作区自身所在的仓库」，直接写进共享状态会把用户在这个
// 会话里选中的其他仓库顶掉（面板重挂、手动刷新仓库栏、切会话回来都会发生）。
// 所以把选中结果按会话记住，恢复时用同一次探测到的仓库列表校验，列表里没有就回落默认。
function repoMemoryStorage(): Storage | null {
  return (globalThis as { localStorage?: Storage }).localStorage ?? null;
}

function repoMemoryStorageKey(sessionKey: string): string {
  const key = String(sessionKey || "").trim();
  return key ? `${key}:git-panel-repo` : "";
}

/** 记住某会话选中的仓库根；sessionKey 为空时不做任何事 */
export function rememberRepoRoot(sessionKey: string, root: string): void {
  const storageKey = repoMemoryStorageKey(sessionKey);
  const value = String(root || "").trim();
  if (!storageKey || !value) return;
  const storage = repoMemoryStorage();
  if (!storage) return;
  try {
    storage.setItem(storageKey, value);
  } catch {
    // 写入失败忽略：记忆只是偏好，丢了不影响当前会话
  }
}

/**
 * 取某会话记住的仓库根，并用本次探测到的仓库列表校验。
 * - 命中列表：返回列表里的那份路径（用后端规范化结果）
 * - 列表非空但没命中：说明该仓库已不在当前工作区，清掉记忆并返回空
 * - 列表为空：保留记忆并返回空（探测可能尚未就绪，不做破坏性清理）
 */
export function readRememberedRepoRoot(sessionKey: string, availableRoots: string[]): string {
  const storageKey = repoMemoryStorageKey(sessionKey);
  if (!storageKey) return "";
  const storage = repoMemoryStorage();
  if (!storage) return "";
  let saved = "";
  try {
    saved = String(storage.getItem(storageKey) || "").trim();
  } catch {
    return "";
  }
  if (!saved) return "";
  const roots = (availableRoots || []).map((root) => String(root || "").trim()).filter(Boolean);
  const matched = roots.find((root) => isSameRepoPath(root, saved));
  if (matched) return matched;
  if (roots.length > 0) {
    try {
      storage.removeItem(storageKey);
    } catch {
      // 清理失败忽略：下次探测仍会走一次校验
    }
  }
  return "";
}

// ==================== 状态读写 ====================
function clearStatus() {
  statusEntries.value = [];
  statusTruncated.value = false;
  stagedTotal.value = 0;
  unstagedTotal.value = 0;
  currentBranch.value = "";
  statusLoaded.value = false;
  statusError.value = "";
  recentCommits.value = [];
}

/** 切换当前仓库；数据清空后若已有消费方则立即重载 */
function setRepoRoot(root: string) {
  const next = String(root || "").trim();
  if (!next) {
    if (!repoRoot.value) return;
    loadSeq += 1;
    repoRoot.value = "";
    clearStatus();
    lastStatusLoad = 0;
    syncWatcher();
    return;
  }
  if (isSameRepoPath(next, repoRoot.value)) {
    repoRoot.value = next;
    return;
  }
  loadSeq += 1;
  repoRoot.value = next;
  clearStatus();
  // 新仓库的首次加载不受冷却拦截
  lastStatusLoad = 0;
  syncWatcher();
  if (consumerCount > 0) {
    void loadStatus(true);
    void loadRecentCommits();
  }
}

/** 用一次 git_panel_status 的结果回填共享状态（stage / unstage 等写操作后可直接复用返回值） */
function applyStatus(root: string, result: GitPanelStatusOutput) {
  if (root && repoRoot.value && !isSameRepoPath(root, repoRoot.value)) return;
  statusEntries.value = result.entries || [];
  statusTruncated.value = !!result.truncated;
  stagedTotal.value = result.stagedTotal ?? 0;
  unstagedTotal.value = result.unstagedTotal ?? 0;
  currentBranch.value = result.branch || "";
  statusLoaded.value = true;
  statusError.value = "";
  // 后端可能返回规范化后的仓库根（大小写 / 分隔符差异），仅在真正不同时切换
  if (result.repoRoot && !isSameRepoPath(result.repoRoot, repoRoot.value)) {
    setRepoRoot(result.repoRoot);
  }
}

async function loadStatus(force = false) {
  const root = repoRoot.value;
  if (!root) return;
  const now = Date.now();
  if (!force && now - lastStatusLoad < REFRESH_CD_MS) {
    gitWatchProbe("状态请求被冷却拦住", { root, force, sinceLastMs: now - lastStatusLoad });
    return;
  }
  lastStatusLoad = now;
  const seq = ++loadSeq;
  const startedAt = Date.now();
  gitWatchProbe("发起状态请求", { root, force, seq });
  try {
    const result = await gitPanelStatus(root);
    if (seq !== loadSeq) {
      gitWatchProbe("状态响应因过期丢弃", { root, force, seq, currentSeq: loadSeq, costMs: Date.now() - startedAt });
      return;
    }
    applyStatus(root, result);
    gitWatchProbe("状态已回填", {
      root,
      force,
      entries: (result.entries || []).length,
      stagedTotal: result.stagedTotal ?? 0,
      unstagedTotal: result.unstagedTotal ?? 0,
      costMs: Date.now() - startedAt,
    });
  } catch (error) {
    if (seq !== loadSeq) return;
    statusError.value = error instanceof Error ? error.message : String(error);
    gitWatchProbe("状态请求失败", { root, force, costMs: Date.now() - startedAt, error: String(error) });
  }
}

/**
 * 拉最近几条提交，只服务卡片墙的提交卡。
 * Git 面板的完整分页历史由面板自理，两边互不读写同一份列表。
 */
async function loadRecentCommits() {
  const root = repoRoot.value;
  if (!root) {
    recentCommits.value = [];
    return;
  }
  try {
    const result = await gitPanelLog(root, RECENT_COMMITS_LIMIT);
    if (!isSameRepoPath(root, repoRoot.value)) return;
    recentCommits.value = result.entries || [];
  } catch (error) {
    if (!isSameRepoPath(root, repoRoot.value)) return;
    recentCommits.value = [];
    console.warn("[Git状态] 读取最近提交失败", error);
  }
}

/**
 * 按当前工作区探测默认仓库根（走后端缓存，不强制重扫）。
 * 一并返回本次探测到的仓库列表：消费方用它校验按会话记住的仓库是否还在。
 */
async function discoverRepoRoot(
  workspacePath: string,
): Promise<{ defaultRepoRoot: string; repoPaths: string[] }> {
  const workspace = String(workspacePath || "").trim();
  if (!workspace) return { defaultRepoRoot: "", repoPaths: [] };
  try {
    const result = await gitPanelDiscover(workspace, false);
    return {
      defaultRepoRoot: String(result?.defaultRepoRoot || ""),
      repoPaths: (result?.repos || []).map((repo) => String(repo?.path || "")).filter(Boolean),
    };
  } catch (error) {
    console.warn("[Git状态] 探测默认仓库失败", error);
    return { defaultRepoRoot: "", repoPaths: [] };
  }
}

// ==================== 仓库监听与事件 ====================
function subscribeWatch() {
  if (unlistenWatch) return;
  gitWatchProbe("订阅变化信号");
  unlistenWatch = onTransportNotification<GitPanelWatchEventPayload>(
    "gitPanel.watchChanged",
    handleWatchEvent,
  );
}

function unsubscribeWatch() {
  gitWatchProbe("取消订阅变化信号");
  unlistenWatch?.();
  unlistenWatch = null;
}

function handleWatchEvent(payload: GitPanelWatchEventPayload) {
  const currentRoot = repoRoot.value;
  const sameRepo = !!currentRoot && isSameRepoPath(payload?.workspacePath || "", currentRoot);
  gitWatchProbe("收到变化信号", {
    payloadRepo: payload?.workspacePath || "",
    currentRoot,
    sameRepo,
    workdirChanged: !!payload?.workdirChanged,
    headChanged: !!payload?.headChanged,
    refsChanged: !!payload?.refsChanged,
  });
  if (!sameRepo) return;
  void loadStatus(true);
  // 最近提交只在 HEAD / refs 真变时才会变；纯工作区文件改动不必多跑一次 git log
  if (payload?.headChanged || payload?.refsChanged) void loadRecentCommits();
  for (const handler of externalChangeHandlers) handler(payload);
}

/** 让后端监听跟随「是否有消费方 + 当前仓库」，两者都变了才重启 */
function syncWatcher() {
  const target = consumerCount > 0 ? repoRoot.value : "";
  if (!target && !watchedRoot) return;
  if (target && isSameRepoPath(target, watchedRoot)) {
    watchedRoot = target;
    return;
  }
  const prev = watchedRoot;
  watchedRoot = target;
  gitWatchProbe("同步仓库监听", { from: prev, to: target, consumers: consumerCount });
  if (prev) {
    gitPanelWatchStop(prev).catch((error) => console.warn("[Git状态] 停止仓库监听失败", error));
  }
  if (target) {
    gitPanelWatchStart(target).catch((error) => console.warn("[Git状态] 开启仓库监听失败", error));
    subscribeWatch();
  } else {
    unsubscribeWatch();
  }
}

/** 订阅「当前仓库有外部变化」信号；返回取消订阅函数 */
function onExternalChange(handler: (payload: GitPanelWatchEventPayload) => void): () => void {
  externalChangeHandlers.add(handler);
  return () => {
    externalChangeHandlers.delete(handler);
  };
}

/** 普通消费方（卡片墙等）声明需要实时数据 */
function acquire() {
  consumerCount += 1;
  syncWatcher();
}

function release() {
  consumerCount = Math.max(0, consumerCount - 1);
  syncWatcher();
}

/** Git 面板声明占用：面板在时仓库根由面板控制 */
function acquirePanel() {
  panelCount += 1;
  acquire();
}

function releasePanel() {
  panelCount = Math.max(0, panelCount - 1);
  release();
}

/** 是否已有 Git 面板在控制仓库根 */
function isPanelActive(): boolean {
  return panelCount > 0;
}

export function useWorkspaceGitStatus() {
  return {
    repoRoot,
    currentBranch,
    statusEntries,
    statusTruncated,
    stagedTotal,
    unstagedTotal,
    statusLoaded,
    statusError,
    recentCommits,
    setRepoRoot,
    applyStatus,
    loadStatus,
    loadRecentCommits,
    discoverRepoRoot,
    rememberRepoRoot,
    readRememberedRepoRoot,
    onExternalChange,
    acquire,
    release,
    acquirePanel,
    releasePanel,
    isPanelActive,
  };
}
