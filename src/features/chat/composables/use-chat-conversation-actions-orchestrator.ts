import { importTransportConversationShare, invokeTauri, openTransportConversationDraft, updateTransportConversationDraft } from "../../../services/tauri-api";
import type { ShellWorkspace, ShellWorkMode } from "../../../types/app";

export function useChatConversationActionsOrchestrator(bindings: Record<string, any>) {
  function normalizeSelectedMessageIds(messageIds: unknown): string[] {
    return Array.isArray(messageIds)
      ? messageIds
          .map((item) => String(item || "").trim())
          .filter((item, index, array) => !!item && array.indexOf(item) === index)
      : [];
  }

  async function createUnarchivedConversation(input?: { title?: string; agentId?: string; copyCurrent?: boolean; importPath?: string; shellWorkspaces?: ShellWorkspace[]; shellWorkMode?: ShellWorkMode; shellAutonomousMode?: boolean }) {
    const agentId =
      String(input?.agentId || "").trim()
      || String(bindings.defaultCreateConversationAgentId.value || "").trim();
    if (!agentId) return "";
    try {
      const copySourceConversationId = input?.copyCurrent
        ? String(bindings.currentChatConversationId.value || "").trim()
        : "";
      const importPath = String(input?.importPath || "").trim();
      const request = {
        agentId: agentId || null,
        title: String(input?.title || "").trim() || null,
        shellWorkspaces: input?.shellWorkspaces || null,
        shellWorkMode: input?.shellWorkMode || null,
        shellAutonomousMode: Boolean(input?.shellAutonomousMode),
      };
      const result = importPath
        ? await importTransportConversationShare<{
          conversationId: string;
          unarchivedConversations?: any[];
        }>({ path: importPath, ...request })
        : await invokeTauri<{
          conversationId: string;
          unarchivedConversations?: any[];
        }>("conversation.create", {
          input: {
            ...request,
            copySourceConversationId: copySourceConversationId || null,
          },
        });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return "";
      const createdItem = Array.isArray(result.unarchivedConversations)
        ? result.unarchivedConversations[0]
        : null;
      if (createdItem?.conversationId) {
        // 后端现在只返回新会话单条，插入式更新列表，不再整表覆盖。
        bindings.applyConversationOverviewItemUpdated?.({ conversation: createdItem });
      } else {
        await bindings.refreshUnarchivedConversationOverview();
      }
      await bindings.switchUnarchivedConversation(conversationId);
      if (importPath) {
        bindings.setStatus(bindings.tr("status.conversationShareImported"));
      }
      return conversationId;
    } catch (error) {
      bindings.setStatus(bindings.tr("status.conversationCreateFailed", { err: bindings.formatRequestFailed(error) }));
      return "";
    }
  }

  // 新建会话 = 打开（或创建）会话草稿：设置直接落在草稿字段上，
  // 发出第一句话时后端自动转正并创建下一个备用草稿。
  // workspace 传入时，后端在返回前把工作区写入草稿。
  async function openDraftConversation(workspace?: {
    shellWorkspaces?: ShellWorkspace[];
    shellWorkMode?: ShellWorkMode;
    shellAutonomousMode?: boolean;
  }) {
    try {
      const result = await openTransportConversationDraft<{ conversationId: string; created: boolean }>({
        shellWorkspaces: workspace?.shellWorkspaces || null,
        shellWorkMode: workspace?.shellWorkMode || null,
        shellAutonomousMode: workspace?.shellAutonomousMode || null,
      });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return "";
      await bindings.switchUnarchivedConversation(conversationId);
      // 草稿可能已经打开（conversationId 未变化）：watcher 不触发，需主动刷新工作区 UI
      if (typeof bindings.refreshChatWorkspaceState === "function") {
        await bindings.refreshChatWorkspaceState();
      }
      return conversationId;
    } catch (error) {
      bindings.setStatus(bindings.tr("status.conversationCreateFailed", { err: bindings.formatRequestFailed(error) }));
      return "";
    }
  }

  // 在草稿历史区切换人格/模型/标题：直接改写草稿会话字段
  async function updateDraftConversation(patch: { agentId?: string; preferredApiConfigId?: string | null; title?: string | null }) {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!conversationId) return false;
    try {
      const input: Record<string, unknown> = { conversationId };
      if (patch.agentId !== undefined) input.agentId = patch.agentId;
      if (patch.preferredApiConfigId !== undefined) {
        // undefined 不修改；null 清空回人格默认；字符串为指定模型
        input.preferredApiConfigId = patch.preferredApiConfigId;
      }
      if (patch.title !== undefined) {
        // undefined 不修改；null/空字符串清空标题；字符串为自定义标题
        input.title = patch.title;
      }
      await updateTransportConversationDraft(input);
      return true;
    } catch (error) {
      bindings.setStatusError("status.requestFailed", error);
      return false;
    }
  }

  async function createSideChatConversation(parentConversationId?: string, withContext = true) {    const parentId = String(parentConversationId || bindings.currentChatConversationId.value || "").trim();
    if (!parentId) return "";
    try {
      const result = await invokeTauri<{
        conversationId: string;
        parentConversationId: string;
        conversationKind: string;
        title: string;
      }>("conversation.createSide", {
        input: { parentConversationId: parentId, withContext },
      });
      return String(result?.conversationId || "").trim();
    } catch (error) {
      bindings.setStatusError("status.requestFailed", error);
      return "";
    }
  }

  async function branchConversationFromSelection(payload: { count: number; messageIds: string[] }) {
    const sourceConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const selectedMessageIds = normalizeSelectedMessageIds(payload?.messageIds);
    if (
      !sourceConversationId
      || selectedMessageIds.length === 0
      || bindings.branchingConversation.value
      || bindings.forwardingConversationSelection.value
    ) return;
    bindings.branchingConversation.value = true;
    try {
      const result = await invokeTauri<{
        conversationId: string;
        title: string;
        warning?: string | null;
      }>("conversation.branchFromSelection", {
        input: {
          sourceConversationId,
          selectedMessageIds,
        },
      });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return;
      // 分支创建已由后端单项事件插入，这里仅做差量兜底，不再全量拉取。
      if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
        await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("branch_from_selection");
      }
      const warning = String(result?.warning || "").trim();
      await bindings.switchUnarchivedConversation(conversationId);
      if (warning) {
        bindings.setStatus(bindings.tr("status.conversationBranchCreatedWithWarning", { warning }));
      } else {
        bindings.setStatus(bindings.tr("status.conversationBranchCreated", { title: String(result?.title || "").trim() || conversationId }));
      }
    } catch (error) {
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.branchingConversation.value = false;
    }
  }

  async function branchConversationFromCurrent() {
    const sourceConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!sourceConversationId || bindings.branchingConversation.value) return;
    bindings.branchingConversation.value = true;
    try {
      const result = await invokeTauri<{
        conversationId: string;
        title: string;
        warning?: string | null;
      }>("conversation.branchFromCurrent", {
        input: {
          sourceConversationId,
        },
      });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return;
      // 分支创建已由后端单项事件插入，这里仅做差量兜底，不再全量拉取。
      if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
        await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("branch_from_current");
      }
      const warning = String(result?.warning || "").trim();
      await bindings.switchUnarchivedConversation(conversationId);
      if (warning) {
        bindings.setStatus(bindings.tr("status.conversationBranchCreatedWithWarning", { warning }));
      } else {
        bindings.setStatus(bindings.tr("status.conversationBranchCreated", { title: String(result?.title || "").trim() || conversationId }));
      }
    } catch (error) {
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.branchingConversation.value = false;
    }
  }

  async function createConversationBranchFromMessage(payload: { turnId: string; targetUserMessageId: string }) {
    const sourceConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const turnMessageId = String(payload?.targetUserMessageId || payload?.turnId || "").trim();
    if (
      !sourceConversationId
      || !turnMessageId
      || bindings.branchingConversation.value
      || bindings.forwardingConversationSelection.value
    ) return;
    bindings.branchingConversation.value = true;
    try {
      const result = await invokeTauri<{
        conversationId: string;
        title: string;
        warning?: string | null;
      }>("conversation.branchFromMessage", {
        input: {
          sourceConversationId,
          turnMessageId,
        },
      });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return;
      // 分支创建已由后端单项事件插入，这里仅做差量兜底，不再全量拉取。
      if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
        await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("branch_from_message");
      }
      await bindings.switchUnarchivedConversation(conversationId);
      bindings.setStatus(bindings.tr("status.conversationBranchCreated", { title: String(result?.title || "").trim() || conversationId }));
    } catch (error) {
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.branchingConversation.value = false;
    }
  }

  async function forwardConversationFromSelection(payload: {
    count: number;
    messageIds: string[];
    target: {
      kind: "local_unarchived" | "remote_im_contact";
      conversationId: string;
      remoteContactId?: string;
    };
  }) {
    const sourceConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const targetKind = payload?.target?.kind === "remote_im_contact" ? "remote_im_contact" : "local_unarchived";
    const targetConversationId = String(payload?.target?.conversationId || "").trim();
    const remoteContactId = String(payload?.target?.remoteContactId || "").trim();
    const selectedMessageIds = normalizeSelectedMessageIds(payload?.messageIds);
    if (
      !sourceConversationId
      || !targetConversationId
      || (targetKind === "remote_im_contact" && !remoteContactId)
      || selectedMessageIds.length === 0
      || bindings.trimming.value
      || bindings.branchingConversation.value
      || bindings.forwardingConversationSelection.value
    ) return;
    bindings.forwardingConversationSelection.value = true;
    try {
      if (targetKind === "remote_im_contact") {
        const result = await invokeTauri<{
          targetConversationId: string;
          remoteContactId: string;
          forwardedCount: number;
        }>("conversation.forwardRemoteContact", {
          input: {
            sourceConversationId,
            targetConversationId,
            remoteContactId,
            selectedMessageIds,
          },
        });
        const effectiveTargetConversationId = String(result?.targetConversationId || targetConversationId).trim();
        if (!effectiveTargetConversationId) return;
        if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
          await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("forward_selection_to_remote_contact");
        } else {
          await bindings.refreshUnarchivedConversationOverview();
        }
        await bindings.refreshRemoteImConversationOverview();
        await bindings.switchRemoteImContactConversation(
          String(result?.remoteContactId || remoteContactId).trim(),
        );
        bindings.setStatus(bindings.tr("status.conversationSelectionForwardedToRemoteContact", {
          count: Number(result?.forwardedCount || selectedMessageIds.length),
        }));
      } else {
        const result = await invokeTauri<{
          targetConversationId: string;
          forwardedCount: number;
        }>("conversation.forwardSelection", {
          input: {
            sourceConversationId,
            targetConversationId,
            selectedMessageIds,
          },
        });
        const effectiveTargetConversationId = String(result?.targetConversationId || targetConversationId).trim();
        if (!effectiveTargetConversationId) return;
        if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
          await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("forward_selection_to_local_conversation");
        } else {
          await bindings.refreshUnarchivedConversationOverview();
        }
        await bindings.switchUnarchivedConversation(effectiveTargetConversationId);
        bindings.setStatus(bindings.tr("status.conversationSelectionForwarded", {
          count: Number(result?.forwardedCount || selectedMessageIds.length),
        }));
      }
    } catch (error) {
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.forwardingConversationSelection.value = false;
    }
  }

  async function userAsyncDelegateFromSelection(payload: {
    count: number;
    messageIds: string[];
    agentId: string;
    presetId: string;
    why: string;
    goal: string;
    todo: string;
  }) {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    const targetAgentId = String(payload?.agentId || "").trim();
    const selectedMessageIds = normalizeSelectedMessageIds(payload?.messageIds);
    const goal = String(payload?.goal || "").trim();
    const todo = String(payload?.todo || "").trim();
    if (!conversationId || !targetAgentId || !goal) return false;
    const sourceAgentId = String(bindings.currentForegroundAgentId.value || "").trim();
    if (sourceAgentId && sourceAgentId === targetAgentId) {
      bindings.setStatus(bindings.tr("status.asyncDelegateSelfSyncOnly"));
      return false;
    }
    try {
      const result = await invokeTauri<{
        delegateId: string;
        conversationId: string;
        targetAgentId: string;
        targetAgentName: string;
        selectedMessageCount: number;
      }>("delegate.submit", {
        input: {
          conversationId,
          targetAgentId,
          presetId: String(payload?.presetId || "review").trim() || "review",
          why: String(payload?.why || "").trim(),
          goal,
          todo,
          selectedMessageIds,
        },
      });
      const targetName = String(result?.targetAgentName || result?.targetAgentId || "").trim() || "子代理";
      const selectedCount = Number(result?.selectedMessageCount || selectedMessageIds.length);
      bindings.setStatus(selectedCount > 0
        ? bindings.tr("status.asyncDelegateStartedWithMessages", { name: targetName, count: selectedCount })
        : bindings.tr("status.asyncDelegateStarted", { name: targetName }));
      return true;
    } catch (error) {
      bindings.setStatus(bindings.tr("status.asyncDelegateFailed", { err: bindings.formatRequestFailed(error) }));
      return false;
    }
  }

  async function renameCurrentConversation(payload: { conversationId: string; title: string }) {
    const conversationId = String(payload?.conversationId || "").trim();
    const title = String(payload?.title || "").trim();
    if (!conversationId) return;
    try {
      const result = await invokeTauri<{ conversationId: string; title: string }>("conversation.rename", {
        input: {
          conversationId,
          title,
        },
      });
      const nextTitle = String(result?.title || "").trim();
      bindings.unarchivedConversations.value = bindings.unarchivedConversations.value.map((item: any) =>
        String(item.conversationId || "").trim() === conversationId
          ? {
            ...item,
            title: nextTitle,
          }
          : item
      );
      bindings.setStatus(bindings.t("status.conversationRenamed"));
    } catch (error) {
      bindings.setStatusError("status.renameConversationFailed", error);
    }
  }

  async function toggleConversationPin(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    try {
      const result = await invokeTauri<{ conversationId: string; isPinned: boolean; pinIndex?: number | null }>("conversation.pin", {
        input: {
          conversationId: cid,
        },
      });
      bindings.applyConversationPinUpdated({
        conversationId: String(result?.conversationId || cid).trim(),
        isPinned: !!result?.isPinned,
        pinIndex: Number.isFinite(Number(result?.pinIndex)) ? Number(result?.pinIndex) : undefined,
      });
    } catch (error) {
      bindings.setStatusError("status.requestFailed", error);
    }
  }

  return {
    createUnarchivedConversation,
    openDraftConversation,
    updateDraftConversation,
    createSideChatConversation,
    branchConversationFromSelection,
    branchConversationFromCurrent,
    createConversationBranchFromMessage,
    forwardConversationFromSelection,
    userAsyncDelegateFromSelection,
    renameCurrentConversation,
    toggleConversationPin,
  };
}
