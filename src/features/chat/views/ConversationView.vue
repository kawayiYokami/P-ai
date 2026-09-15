<template>
  <ChatView
    class="h-full"
    composer-scope="side"
    :user-alias="userAlias"
    :persona-name="personaName"
    :user-avatar-url="userAvatarUrl"
    :assistant-avatar-url="assistantAvatarUrl"
    :persona-name-map="personaNameMap"
    :persona-avatar-url-map="personaAvatarUrlMap"
    :mention-entries="[]"
    :selected-mentions="runtime.selectedMentions.value"
    :latest-user-text="runtime.latestUserText.value"
    :latest-user-images="runtime.latestUserImages.value"
    :frontend-round-phase="runtime.flow.frontendRoundPhase.value"
    :submit-pending="runtime.submitPending.value"
    :chat-error-text="runtime.chatErrorText.value"
    :clipboard-images="runtime.clipboardImages.value"
    :queued-attachment-notices="runtime.queuedAttachmentNotices.value"
    :chat-input="runtime.chatInput.value"
    :instruction-presets="instructionPresets"
    :conversation-call-primary-api-config-id="runtime.preferredApiConfigId.value"
    :preferred-chat-model-id="runtime.preferredApiConfigId.value"
    tool-review-api-config-id=""
    :tool-review-refresh-tick="0"
    :chat-model-options="chatModelOptions"
    :plan-mode-enabled="runtime.planModeEnabled.value"
    :chat-usage-percent="messageBlocks.chatUsagePercent.value"
    :media-drag-active="false"
    :chatting="runtime.chatting.value"
    :trimming="false"
    :compacting-conversation="false"
    :conversation-busy="
      isViewLayerBusy({
        trimming: false,
        compactingConversation: false,
        activeConversationId: conversationId,
        organizingContext: runtime.runtimeState.value === 'organizing_context',
      })
    "
    :frozen="false"
    :message-blocks="messageBlocks.visibleMessageBlocks.value"
    :has-more-history="runtime.hasMoreHistory.value"
    :loading-older-history="runtime.loadingOlderHistory.value"
    :latest-own-message-align-request="0"
    :conversation-scroll-to-bottom-request="runtime.conversationScrollToBottomRequest.value"
    :scroll-to-bottom-behavior="runtime.scrollToBottomBehavior.value"
    :current-workspace-name="workspaceName"
    :current-workspace-display-name="workspaceName"
    :current-workspace-root-path="workspaceRootPath"
    :workspaces="workspaces"
    :current-workspace-autonomous-mode="false"
    :active-agent-id="agentId"
    :active-conversation-id="conversationId"
    :current-todos="runtime.currentTodos.value"
    :current-theme="currentTheme"
    :unarchived-conversation-items="conversationItems"
    :remote-im-contact-conversations="[]"
    :conversation-items="conversationItems"
    :side-conversation-list-visible="false"
    :initial-tool-review-panel-open="false"
    :conversation-list-tab="'local'"
    :chat-left-panel-mode="'local'"
    :chat-right-panel-mode="'reader'"
    :chat-monitor-panel-mode="'delegate'"
    :create-conversation-agent-options="[]"
    :default-create-conversation-agent-id="agentId"
    :ide-context-groups="[]"
    :terminal-approvals="terminalApprovals"
    :terminal-approval-resolving="terminalApprovalResolving"
    :hide-conversation-control-panel="true"
    :hide-workspace-button="true"
    :workspace-access="workspaceAccess"
    @update:chat-input="updateChatInput"
    @send-chat="runtime.send"
    @stop-chat="runtime.stop"
    @clear-chat-error="clearChatError"
    @load-older-history="runtime.loadOlderHistory"
    @load-older-compaction-segment="runtime.loadOlderCompactionSegment"
    @update:conversation-preferred-api-config-id="handleConversationPreferredApiConfigIdChange"
    @recall-turn="runtime.handleRecallTurn"
    @regenerate-turn="runtime.handleRegenerateTurn"
    @create-conversation-branch-from-turn="props.createConversationBranchFromTurn"
    @approve-terminal-approval="(requestId: string, reason?: string) => approveTerminalApproval?.(requestId, reason)"
    @deny-terminal-approval="(requestId: string, reason?: string) => denyTerminalApproval?.(requestId, reason)"
    @approve-terminal-approval-for-session="approveTerminalApprovalForSession?.($event)"
    @approve-terminal-approval-for-workspace="approveTerminalApprovalForWorkspace?.($event)"
    :goal-active="goalActive"
    :goal-title="goalTitleText"
    :goal-dialog-open="goalDialogOpen"
    :goal-saving="goalSaving"
    :goal-error="goalErrorText"
    :active-goal-task="activeGoalTask"
    :recent-goal-task-history="recentGoalTaskHistory"
    @open-goal-task="openGoalTaskDialog"
    @close-goal-task="closeGoalTaskDialog"
    @save-goal-task="saveGoalTask"
    @stop-goal-task="stopGoalTask"
  />
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, toRef } from "vue";
import type { ApiConfigItem, AppConfig, PromptCommandPreset, ShellWorkspace } from "../../../types/app";
import ChatView from "./ChatView.vue";
import { useChatMessageBlocks } from "../composables/use-chat-turns";
import { useConversationViewRuntime } from "../composables/use-conversation-view-runtime";
import { useGoalTask } from "../composables/use-goal-task";
import { useConversationPreferredModel } from "../composables/use-conversation-preferred-model";
import { isTextRequestFormat } from "../../config/quick-setup/usable-text-llm";
import { isViewLayerBusy } from "../composables/chat-view-busy";
import { getActiveChatComposerScope, clearChatComposerFocus } from "../composables/chat-composer-focus";
import { collectPastedFiles, ingestPastedImages } from "../composables/chat-paste-ingest";
import { useI18n } from "vue-i18n";
import { onTransportNotification, invokeTauri } from "../../../services/tauri-api";
import type { ConversationGoalState } from "../../../types/app";
import type { TerminalApprovalConversationItem } from "../../shell/composables/use-terminal-approval";
import type { ExclusiveChatViewSubscriptionSlot } from "../composables/exclusive-chat-view-subscription-slot";

const props = defineProps<{
  conversationId: string;
  subscriptionSlot?: ExclusiveChatViewSubscriptionSlot;
  apiConfigId: string;
  agentId: string;
  personaName: string;
  userAlias: string;
  userAvatarUrl: string;
  assistantAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  chatModelOptions: ApiConfigItem[];
  instructionPresets: PromptCommandPreset[];
  workspaceName: string;
  workspaceRootPath: string;
  workspaces: ShellWorkspace[];
  workspaceAccess: "approval" | "full_access";
  currentTheme: string;
  config: AppConfig;
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
  approveTerminalApproval?: (requestId: string, reason?: string) => void;
  denyTerminalApproval?: (requestId: string, reason?: string) => void;
  approveTerminalApprovalForSession?: (requestId: string) => void;
  approveTerminalApprovalForWorkspace?: (requestId: string) => void;
  requestRecallMode?: (payload: {
    turnId: string;
    targetUserMessageId: string;
    conversationId?: string;
  }) => Promise<"with_patch" | "message_only" | "cancel">;
  createConversationBranchFromTurn?: (payload: { turnId: string }) => Promise<void> | void;
}>();

const { t } = useI18n();
let unlistenGoalUpdated: (() => void) | null = null;
const conversationId = toRef(props, "conversationId");
const apiConfigId = toRef(props, "apiConfigId");
const agentId = toRef(props, "agentId");
const runtime = useConversationViewRuntime({
  conversationId,
  apiConfigId,
  agentId,
  subscriptionSlot: props.subscriptionSlot,
  requestRecallMode: props.requestRecallMode,
  t,
});
const {
  goalDialogOpen,
  goalSaving,
  goalError: goalErrorText,
  activeGoalTask,
  recentGoalTaskHistory,
  chatGoalActive: goalActive,
  chatGoalTitle: goalTitleText,
  openGoalTaskDialog,
  closeGoalTaskDialog,
  saveGoalTask,
  stopGoalTask,
  refreshActiveGoalTask,
  applyConversationGoalUpdated,
} = useGoalTask({
  t,
  currentConversationId: conversationId,
  // 追问侧无全局状态条（transientNotice 是 ChatView 内部机制），
  // goal 完成反馈由按钮高亮与对话框关闭承载，这里静默。
  setStatus: () => {},
});
const activeApiConfig = computed(() =>
  props.chatModelOptions.find((item) => item.id === runtime.preferredApiConfigId.value) || null,
);
const { updateConversationPreferredApiConfigId } = useConversationPreferredModel({
  config: props.config,
  currentChatConversationId: conversationId,
  currentChatPreferredApiConfigId: runtime.preferredApiConfigId,
  isTextRequestFormat,
  setStatus: () => {},
  setStatusError: (key: string, error: unknown) => {
    console.error("[追问会话模型] 保存失败", { key, error });
  },
});
function handleConversationPreferredApiConfigIdChange(value: string) {
  updateConversationPreferredApiConfigId(value);
}
const messageBlocks = useChatMessageBlocks({
  allMessages: runtime.allMessages,
  activeChatApiConfig: activeApiConfig,
  currentConversationId: conversationId,
  perfDebug: false,
  perfNow: () => performance.now(),
  taskTriggerLabels: { goal: t("config.task.fields.goal"), todo: t("config.task.fields.todo") },
});
const conversationItems = computed(() => conversationId.value ? [{
  conversationId: conversationId.value,
  title: "",
  kind: "local_unarchived" as const,
  messageCount: runtime.allMessages.value.length,
  agentId: agentId.value,
}] : []);

function updateChatInput(value: string) {
  runtime.chatInput.value = value;
}

function clearChatError() {
  runtime.chatErrorText.value = "";
  const cid = String(conversationId.value || "").trim();
  if (!cid) return;
  void invokeTauri("conversation.clearChatError", { input: { conversationId: cid } }).catch((error) => {
    console.warn("[会话错误] 追问清除失败", { conversationId: cid, error });
  });
}

// 焦点在本视图输入框内时的图片粘贴：与主会话共用同一份入队逻辑，
// 由共享的「最后活跃输入框」状态判断归属，避免图片进错会话队列。
function handleSidePaste(event: ClipboardEvent) {
  if (getActiveChatComposerScope() !== "side") return;
  const apiConfig = activeApiConfig.value;
  if (!apiConfig) return;
  const collected = collectPastedFiles(event);
  if (collected.length === 0) return;
  event.preventDefault();
  clearChatError();
  void ingestPastedImages(collected, apiConfig, {
    setChatError: clearChatError,
    setStatusError: () => {},
    clipboardImages: runtime.clipboardImages,
    queuedAttachmentNotices: runtime.queuedAttachmentNotices,
    hasVisionFallback: false,
  });
}

onMounted(() => {
  window.addEventListener("paste", handleSidePaste);
  void refreshActiveGoalTask({ silent: true });
  unlistenGoalUpdated = onTransportNotification<{
    conversationId?: string;
    goal?: ConversationGoalState | null;
  }>("conversation.goalUpdated", (payload) => {
    applyConversationGoalUpdated(payload);
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("paste", handleSidePaste);
  unlistenGoalUpdated?.();
  // 追问视图卸载后不再有 paste 监听：主动清掉共享焦点归属，
  // 避免 scope 残留 side 导致主会话误判而丢弃图片（回退主会话接管）。
  clearChatComposerFocus("side");
});
</script>
