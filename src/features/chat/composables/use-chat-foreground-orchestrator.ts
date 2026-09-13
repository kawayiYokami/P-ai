import { nextTick } from "vue";
import { i18n } from "../../../i18n";
import { chatStreamNeedsFrontendBind, invokeTauri } from "../../../services/tauri-api";
import { toErrorMessage } from "../../../utils/error";
import {
  clearLastActiveConversationId,
  isValidConversationIdInCandidates,
  readLastActiveConversationId,
} from "../utils/last-active-conversation";
import { createLatestTaskRunner, runForegroundSnapshotBindingTransaction, snapshotCanBindAssistantStream } from "./chat-foreground-coordinator";

const t = i18n.global.t;

export function useChatForegroundOrchestrator(bindings: Record<string, any>) {
  function overviewActivityAt(item: Record<string, any>): string {
    return String(item?.lastMessageAt || item?.updatedAt || "").trim();
  }

  function sortOverviewItems(items: any[]): any[] {
    return [...items].sort((a, b) => {
      if (!!a?.isSystemNotificationConversation !== !!b?.isSystemNotificationConversation) {
        return Number(!!b?.isSystemNotificationConversation) - Number(!!a?.isSystemNotificationConversation);
      }
      if (!!a?.isPinned !== !!b?.isPinned) {
        return Number(!!b?.isPinned) - Number(!!a?.isPinned);
      }
      if (a?.isPinned && b?.isPinned) {
        const aIndex = Number.isFinite(Number(a?.pinIndex)) ? Number(a.pinIndex) : Number.MAX_SAFE_INTEGER;
        const bIndex = Number.isFinite(Number(b?.pinIndex)) ? Number(b.pinIndex) : Number.MAX_SAFE_INTEGER;
        return aIndex - bIndex || String(a?.conversationId || "").localeCompare(String(b?.conversationId || ""));
      }
      return overviewActivityAt(b).localeCompare(overviewActivityAt(a))
        || String(a?.conversationId || "").localeCompare(String(b?.conversationId || ""));
    });
  }

  function applyOverviewChangedSincePayload(payload: Record<string, any> | null | undefined) {
    const changed = Array.isArray(payload?.changed) ? payload.changed : [];
    const deletedIds = new Set(
      (Array.isArray(payload?.deletedIds) ? payload.deletedIds : [])
        .map((id: unknown) => String(id || "").trim())
        .filter(Boolean),
    );
    const changedById = new Map<string, any>();
    for (const item of changed) {
      const conversationId = String(item?.conversationId || "").trim();
      if (conversationId) changedById.set(conversationId, item);
    }
    if (changedById.size > 0 || deletedIds.size > 0) {
      const nextItems = bindings.unarchivedConversations.value
        .filter((item: any) => !deletedIds.has(String(item?.conversationId || "").trim()))
        .map((item: any) => {
          const conversationId = String(item?.conversationId || "").trim();
          return changedById.get(conversationId) || item;
        });
      for (const [conversationId, item] of changedById) {
        if (!nextItems.some((existing: any) => String(existing?.conversationId || "").trim() === conversationId)) {
          nextItems.push(item);
        }
      }
      bindings.unarchivedConversations.value = sortOverviewItems(nextItems);
    }
    syncOverviewLastErrors(changed);
    for (const deletedId of deletedIds) {
      if (typeof bindings.setConversationChatErrorText === "function") {
        bindings.setConversationChatErrorText(deletedId, "");
      }
    }
    const serverTime = String(payload?.serverTime || "").trim();
    if (serverTime && bindings.lastOverviewSyncAt) {
      bindings.lastOverviewSyncAt.value = serverTime;
    }
  }

  function syncOverviewLastErrors(items: Record<string, any>[]) {
    if (typeof bindings.setConversationChatErrorText !== "function") return;
    for (const item of items) {
      const conversationId = String(item?.conversationId || "").trim();
      if (!conversationId) continue;
      bindings.setConversationChatErrorText(
        conversationId,
        String(item?.lastError || "").trim(),
      );
    }
  }

  function currentConversationId(): string {
    return String(bindings.currentChatConversationId.value || "").trim();
  }

  function readOverviewRuntimeState(conversationId?: string | null): string {
    const cid = String(conversationId || "").trim();
    if (!cid) return "";
    const item = bindings.unarchivedConversations.value.find(
      (entry: any) => String(entry?.conversationId || "").trim() === cid,
    );
    return String(item?.runtimeState || "").trim();
  }

  function describeLocalBusyProjection(conversationId?: string | null): string {
    const cid = String(conversationId || "").trim();
    if (!cid) return "无";
    const trimmingConversationId = String(bindings.trimmingConversationId.value || "").trim();
    const compactingConversationId = String(bindings.compactingConversationId.value || "").trim();
    const localFlags: string[] = [];
    if (bindings.trimming.value && (!trimmingConversationId || trimmingConversationId === cid)) {
      localFlags.push(`trimming=${trimmingConversationId || "*"}`);
    }
    if (bindings.compactingConversation.value && (!compactingConversationId || compactingConversationId === cid)) {
      localFlags.push(`compacting=${compactingConversationId || "*"}`);
    }
    return localFlags.join(", ") || "无";
  }

  async function appendSwitchRuntimeReconciliation(
    baseText: string,
    targetConversationId: string,
    previousConversationId: string,
  ): Promise<string> {
    const lines = [baseText];
    const targetId = String(targetConversationId || "").trim();
    const previousId = String(previousConversationId || "").trim();
    const currentId = currentConversationId();
    const targetOverviewRuntimeState = readOverviewRuntimeState(targetId);
    const previousOverviewRuntimeState = readOverviewRuntimeState(previousId);
    lines.push(`目标会话列表态：${targetOverviewRuntimeState || "空"}`);
    if (previousId) {
      lines.push(`原会话列表态：${previousOverviewRuntimeState || "空"}`);
    }
    lines.push(`目标会话本地忙态：${describeLocalBusyProjection(targetId)}`);
    if (previousId) {
      lines.push(`原会话本地忙态：${describeLocalBusyProjection(previousId)}`);
    }
    lines.push(`全局本地忙态：trimming=${!!bindings.trimming.value}, compacting=${!!bindings.compactingConversation.value}`);
    if (!targetId) {
      return lines.join("\n");
    }
    if (typeof bindings.requestConversationRuntimeSnapshot !== "function") {
      lines.push("后端对账：未提供 runtime snapshot 接口");
      return lines.join("\n");
    }
    try {
      const snapshot = await bindings.requestConversationRuntimeSnapshot(targetId);
      const backendConversationId = String(snapshot?.conversationId || "").trim();
      lines.push(
        `后端目标运行态：${String(snapshot?.runtimeState || "").trim() || "空"}`
        + `，processing=${!!snapshot?.isProcessing}`
        + `，pending=${Math.max(0, Number(snapshot?.pendingQueueCount || 0))}`
        + `，hasPendingQueue=${!!snapshot?.hasPendingQueue}`
        + `，visibleProgress=${!!snapshot?.streamCache?.hasVisibleProgress}`,
      );
      if (backendConversationId && backendConversationId !== targetId) {
        lines.push(`后端对账会话不一致：requested=${targetId}，actual=${backendConversationId}`);
      }
    } catch (runtimeError) {
      lines.push(`后端对账失败：${toErrorMessage(runtimeError)}`);
    }
    if (currentId && currentId !== targetId && currentId !== previousId) {
      lines.push(`失败后当前会话：${currentId}`);
      lines.push(`失败后当前列表态：${readOverviewRuntimeState(currentId) || "空"}`);
      lines.push(`失败后当前本地忙态：${describeLocalBusyProjection(currentId)}`);
    }
    return lines.join("\n");
  }

  function formatSwitchDiagnostic(
    stage: string,
    targetConversationId: string,
    previousConversationId: string,
    startedAt: number,
    detail?: unknown,
  ): string {
    const currentId = currentConversationId();
    const reason = detail === undefined || detail === null ? "" : toErrorMessage(detail);
    const elapsedMs = Math.round((bindings.perfNow() - startedAt) * 10) / 10;
    return [
      `会话切换未完成：${stage}${reason ? `。原因：${reason}` : ""}`,
      `目标会话：${targetConversationId || "空"}`,
      `当前会话：${currentId || "空"}`,
      `原会话：${previousConversationId || "空"}`,
      `耗时：${elapsedMs}ms`,
    ].join("\n");
  }

  async function showSwitchDiagnostic(
    stage: string,
    targetConversationId: string,
    previousConversationId: string,
    startedAt: number,
    detail?: unknown,
  ) {
    const baseText = formatSwitchDiagnostic(stage, targetConversationId, previousConversationId, startedAt, detail);
    const text = await appendSwitchRuntimeReconciliation(
      baseText,
      targetConversationId,
      previousConversationId,
    );
    const visibleConversationId = currentConversationId() || previousConversationId || targetConversationId;
    if (typeof bindings.setConversationChatErrorText === "function") {
      bindings.setConversationChatErrorText(visibleConversationId, text);
    } else if (typeof bindings.setStatus === "function") {
      bindings.setStatus(text);
    }
    console.warn("[会话切换] 诊断提示", {
      stage,
      targetConversationId,
      currentConversationId: currentConversationId(),
      previousConversationId,
      detail,
    });
  }

  async function requestConversationLightSnapshot(
    conversationId?: string | null,
    options?: { resumeProjection?: boolean },
  ) {
    const targetConversationId = String(conversationId || "").trim();
    return invokeTauri<any>("conversation.foregroundLightSnapshot", {
      input: {
        conversationId: targetConversationId || null,
        agentId: targetConversationId
          ? null
          : String(bindings.currentForegroundAgentId.value || "").trim() || null,
        limit: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
        resumeProjection: !!options?.resumeProjection,
      },
    });
  }

  async function requestUnarchivedConversationOverviewChangedSince(since: string) {
    return invokeTauri<Record<string, any>>("conversation.changedSince", {
      input: { since: String(since || "").trim() || null },
    });
  }

  async function refreshRemoteImConversationOverview() {
    bindings.remoteImContactConversations.value = await invokeTauri<any[]>("remoteIm.conversations.list");
  }

  async function refreshUnarchivedConversationOverview() {
    const payload = await requestUnarchivedConversationOverviewChangedSince("");
    const items = Array.isArray(payload?.changed) ? payload.changed : [];
    bindings.unarchivedConversations.value = sortOverviewItems(items);
    syncOverviewLastErrors(items);
    const serverTime = String(payload?.serverTime || "").trim();
    if (serverTime && bindings.lastOverviewSyncAt) {
      bindings.lastOverviewSyncAt.value = serverTime;
    }
  }

  async function syncUnarchivedConversationOverviewChangedSinceWatermark(reason = "unknown") {
    const since = String(bindings.lastOverviewSyncAt?.value || "").trim();
    if (!since) {
      await refreshUnarchivedConversationOverview();
      return;
    }
    try {
      const payload = await requestUnarchivedConversationOverviewChangedSince(since);
      applyOverviewChangedSincePayload(payload);
    } catch (error) {
      console.warn("[会话概览] 差量补漏失败，回退全量刷新", {
        reason,
        since,
        error,
      });
      await refreshUnarchivedConversationOverview();
    }
  }

  function pickForegroundConversationId(candidates: any[]): string {
    const storedConversationId = readLastActiveConversationId();
    if (storedConversationId) {
      if (isValidConversationIdInCandidates(storedConversationId, candidates)) {
        return storedConversationId;
      }
      clearLastActiveConversationId();
    }
    const target =
      candidates.find((item) => !!item.isSystemNotificationConversation)
      || candidates.find((item) => !!item.isActive)
      || candidates[0];
    return String(target?.conversationId || "").trim();
  }

  function clearForegroundConversation(reason: string) {
    const previousConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!previousConversationId) return;
    bindings.cacheConversationMessages(previousConversationId, bindings.allMessages.value);
    bindings.currentChatConversationId.value = "";
    bindings.currentChatPreferredApiConfigId.value = "";
    bindings.currentChatTodos.value = [];
    if (bindings.trimmingConversationId.value === previousConversationId) {
      bindings.trimmingConversationId.value = "";
      bindings.trimming.value = false;
    }
    if (bindings.compactingConversationId.value === previousConversationId) {
      bindings.compactingConversationId.value = "";
      bindings.compactingConversation.value = false;
    }
    bindings.allMessages.value = [];
    bindings.hasMoreBackendHistory.value = false;
    bindings.foregroundTailLatestReady.value = true;
    bindings.clearPendingManualScrollToBottom();
    bindings.getChatFlow().clearForegroundRuntimeState();
    void reason;
  }

  async function recoverForegroundConversationFromOverview(reason: string, preferredConversationId?: string | null) {
    if (bindings.conversationForegroundSyncing.value) return;
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (currentConversationId) return;
    const nextConversationId =
      String(preferredConversationId || "").trim()
      || pickForegroundConversationId(bindings.unarchivedConversations.value);
    if (!nextConversationId) {
      clearForegroundConversation(reason);
      return;
    }
    await switchUnarchivedConversation(nextConversationId);
  }

  function syncCurrentConversationWorkspaceLabel() {
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!currentConversationId) return;
    const nextLabel = String(bindings.chatWorkspaceName.value || "").trim() || t('chat.foregroundOrchestrator.defaultWorkspace');
    let changed = false;
    const nextItems = bindings.unarchivedConversations.value.map((item: any) => {
      if (String(item.conversationId || "").trim() !== currentConversationId) {
        return item;
      }
      if (String(item.workspaceLabel || "").trim() === nextLabel) {
        return item;
      }
      changed = true;
      return {
        ...item,
        workspaceLabel: nextLabel,
      };
    });
    if (changed) {
      bindings.unarchivedConversations.value = nextItems;
    }
  }

  async function performSwitchUnarchivedConversation(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    const storedId = readLastActiveConversationId();
    if (storedId && cid === storedId && !isValidConversationIdInCandidates(cid, bindings.unarchivedConversations.value)) {
      clearLastActiveConversationId();
      const fallbackId = pickForegroundConversationId(bindings.unarchivedConversations.value);
      if (fallbackId && fallbackId !== cid) {
        return performSwitchUnarchivedConversation(fallbackId);
      }
      await showSwitchDiagnostic("目标会话不在当前列表，已回落", cid, currentConversationId(), bindings.perfNow(), `storedId=${cid}`);
      return;
    }
    const previousConversationId = currentConversationId();
    const startedAt = bindings.perfNow();
    let stage = "准备切换";
    let snapshot: any = null;
    let snapshotApplied = false;
    try {
      stage = "标记前台会话同步中";
      bindings.conversationForegroundSyncing.value = true;
      if (previousConversationId) {
        stage = "保存原会话前台缓存";
        bindings.cacheConversationMessages(previousConversationId, bindings.allMessages.value);
        bindings.clearConversationBadge(previousConversationId);
        bindings.markConversationReadPersisted(previousConversationId);
      }
      const trace = bindings.beginForegroundPaintTrace(cid);
      snapshot = await runForegroundSnapshotBindingTransaction({
        conversationId: cid,
        isCurrent: () => !snapshotApplied || currentConversationId() === cid,
        clearRuntime: () => {
          bindings.getChatFlow().clearForegroundRuntimeState();
          bindings.clearPendingManualScrollToBottom();
          bindings.foregroundTailLatestReady.value = false;
        },
        unbind: () => Promise.resolve(bindings.getChatFlow()?.unbindActiveConversationStream?.()),
        requestSnapshot: () => requestConversationLightSnapshot(cid, { resumeProjection: true }),
        applySnapshot: (nextSnapshot) => {
          const snapshotConversationId = String(nextSnapshot?.conversationId || "").trim();
          if (!snapshotConversationId) {
            void showSwitchDiagnostic("后端快照没有返回会话 id", cid, previousConversationId, startedAt, nextSnapshot);
          } else if (snapshotConversationId !== cid) {
            void showSwitchDiagnostic(
              "后端快照返回了其他会话",
              cid,
              previousConversationId,
              startedAt,
              `snapshotConversationId=${snapshotConversationId}`,
            );
          }
          bindings.applyConversationSnapshot(nextSnapshot);
          snapshotApplied = true;
          const appliedConversationId = currentConversationId();
          if (appliedConversationId !== cid) {
            void showSwitchDiagnostic(
              "快照已应用但当前会话不是目标会话",
              cid,
              previousConversationId,
              startedAt,
              `appliedConversationId=${appliedConversationId || "空"}`,
            );
          }
        },
        bind: async () => {
          const bindActiveConversationStream = bindings.getChatFlow()?.bindActiveConversationStream;
          if (typeof bindActiveConversationStream !== "function") {
            void showSwitchDiagnostic("需要绑定流式通道但绑定函数不存在", cid, previousConversationId, startedAt);
            return;
          }
          await bindActiveConversationStream(cid, true);
        },
        alwaysBind: chatStreamNeedsFrontendBind(),
        resume: (nextSnapshot) => {
          const runtimeState = String(nextSnapshot?.runtimeState || "").trim();
          const streamCache = nextSnapshot?.streamCache as Record<string, unknown> | null | undefined;
          if (runtimeState !== "assistant_streaming" || !snapshotCanBindAssistantStream(nextSnapshot || {})) {
            return;
          }
          bindings.getChatFlow()?.resumeForegroundRuntimeRound?.({
            conversationId: cid,
            streamCache: nextSnapshot?.streamCache || null,
            statusText: t('chat.statusWaitingReply'),
            reason: "switch_conversation_snapshot_ready",
          });
        },
        onStage: (nextStage) => {
          stage = ({
            clear_runtime: "清理前台运行态",
            unbind: "取消原会话前台流绑定",
            request_snapshot: "请求前台轻量快照",
            apply_snapshot: "应用前台轻量快照",
            bind: "绑定前台流式通道",
            resume: "恢复前台运行态",
          } as const)[nextStage];
        },
        onUnbindError: (error) => {
          console.warn("[会话切换] 取消原会话前台流绑定失败", {
            previousConversationId,
            targetConversationId: cid,
            error,
          });
        },
      });
      if (!snapshot) return;
      // 收尾不占切换锁：消息已显示，角标/已读/滚动均按会话 id 独立执行，滚动自带当前会话校验
      void (async () => {
        bindings.clearConversationBadge(cid);
        bindings.markConversationReadPersisted(cid);
        await nextTick();
        const switchRuntimeState = String(snapshot?.runtimeState || "").trim();
        if (switchRuntimeState === "assistant_streaming") {
          // 切到正在流式的会话：视为「有滚到最下的意图」——立即贴底并解锁跟随，
          // 后续无限增长的流式内容由跟随机制持续顶在底部；不再等落库/超时再补滚。
          bindings.triggerConversationScrollToBottom(cid, "switch_streaming_ready", "follow");
        } else {
          bindings.triggerConversationScrollToBottom(cid, "switch_snapshot_ready");
        }
        bindings.logForegroundPaintTrace(trace, "前台轻量快照已接管最新消息", {
          conversationId: cid,
          snapshotCount: Array.isArray(snapshot?.messages) ? snapshot.messages.length : 0,
          hasMoreHistory: !!snapshot?.hasMoreHistory,
          shouldBindStream: !!snapshot?.shouldBindStream,
          fromConversationId: previousConversationId,
          syncCostMs: Math.round((bindings.perfNow() - startedAt) * 10) / 10,
        });
      })().catch((error) => {
        console.warn("[会话切换] 切换收尾失败", { conversationId: cid, error });
      });
    } catch (error) {
      const msg = toErrorMessage(error);
      const storedId = readLastActiveConversationId();
      const notFoundHint = /not found|不存在|找不到|os error 2|读取会话块元数据失败|conversation not found/i.test(msg);
      const isStaleStoredSwitch = !!storedId && cid === storedId;
      if (isStaleStoredSwitch && (notFoundHint || !isValidConversationIdInCandidates(cid, bindings.unarchivedConversations.value))) {
        clearLastActiveConversationId();
      }
      await showSwitchDiagnostic(stage, cid, previousConversationId, startedAt, error);
    } finally {
      bindings.conversationForegroundSyncing.value = false;
    }
  }

  const foregroundSwitchRunner = createLatestTaskRunner<string>(performSwitchUnarchivedConversation);

  function switchUnarchivedConversation(conversationId: string) {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return Promise.resolve();
    return foregroundSwitchRunner.run(normalizedConversationId);
  }

  async function ensureLatestForegroundTailThenScrollToBottom() {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!conversationId) return;
    if (bindings.foregroundTailLatestReady.value) {
      bindings.triggerConversationScrollToBottom(conversationId, "manual_ready", "manual");
      return;
    }
    try {
      const result = await invokeTauri<{ accepted: boolean; requestId: string }>("conversation.messagesAfterAsync", {
        input: {
          conversationId,
          afterMessageId: bindings.buildConversationMessagesAfterAnchor(conversationId),
          fallbackLimit: bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT,
        },
      });
      if (!result?.accepted) {
        bindings.triggerConversationScrollToBottom(conversationId, "manual_request_rejected", "manual");
        return;
      }
      bindings.setPendingManualScrollState(conversationId, String(result.requestId || "").trim());
      if (!String(result.requestId || "").trim()) {
        bindings.triggerConversationScrollToBottom(conversationId, "manual_request_missing_id", "manual");
      }
    } catch (error) {
      console.warn("[会话切换] 手动滚到底前请求尾部增量失败", {
        conversationId,
        error,
      });
      bindings.triggerConversationScrollToBottom(conversationId, "manual_request_failed", "manual");
    }
  }

  async function refreshChatUnarchivedConversations() {
    if (bindings.conversationForegroundSyncing.value) return;
    try {
      bindings.conversationForegroundSyncing.value = true;
      await refreshUnarchivedConversationOverview();
      await refreshRemoteImConversationOverview();
    } finally {
      bindings.conversationForegroundSyncing.value = false;
    }
    if (!String(bindings.currentChatConversationId.value || "").trim()) {
      await recoverForegroundConversationFromOverview("refresh_unarchived_conversations");
    }
  }

  async function handleCloseWindow() {
    bindings.freezeForegroundConversation("close_window");
    await bindings.closeWindow();
  }

  async function sendChatFromCurrentWindow(overrides?: { extraTextBlocks?: string[] }) {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (bindings.waitPendingConversationPreferredModelPersist) {
      const modelPersisted = await bindings.waitPendingConversationPreferredModelPersist(conversationId);
      if (!modelPersisted) return;
    }
    await bindings.getChatFlow().sendChat(overrides);
  }

  function freezeForegroundConversation(reason: string) {
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (currentConversationId) {
      bindings.cacheConversationMessages(currentConversationId, bindings.allMessages.value);
    }
    bindings.getChatFlow().freezeForegroundRoundState();
    void reason;
  }

  function hasActiveForegroundConversation(conversationId?: string | null): boolean {
    if (!bindings.isChatWindowActiveNow()) return false;
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!currentConversationId) return false;
    const targetConversationId = String(conversationId || "").trim();
    return !targetConversationId || targetConversationId === currentConversationId;
  }

  return {
    requestConversationLightSnapshot,
    refreshRemoteImConversationOverview,
    refreshUnarchivedConversationOverview,
    syncUnarchivedConversationOverviewChangedSinceWatermark,
    pickForegroundConversationId,
    clearForegroundConversation,
    recoverForegroundConversationFromOverview,
    syncCurrentConversationWorkspaceLabel,
    switchUnarchivedConversation,
    ensureLatestForegroundTailThenScrollToBottom,
    refreshChatUnarchivedConversations,
    handleCloseWindow,
    sendChatFromCurrentWindow,
    freezeForegroundConversation,
    hasActiveForegroundConversation,
  };
}
