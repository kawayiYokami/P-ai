<template>
  <div
    ref="chatLayoutRoot"
    class="h-full min-h-0"
    :class="showSideConversationList ? 'flex flex-row overflow-hidden' : 'flex flex-col relative'"
  >
    <ChatConversationSidebar
      v-if="showSideConversationList"
      :items="unarchivedConversationItems"
      :active-conversation-id="activeConversationId"
      :user-alias="userAlias"
      :user-avatar-url="userAvatarUrl"
      :persona-name-map="personaNameMap"
      :persona-avatar-url-map="personaAvatarUrlMap"
      @select="handleConversationListSelect"
      @rename="handleConversationRename"
    />

    <div class="flex min-h-0 min-w-0 flex-1 overflow-hidden">
      <div class="relative flex min-h-0 min-w-0 flex-1 flex-col">
      <div
        v-if="mediaDragActive && !chatting && !frozen"
        class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
      >
        <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
          {{ t("chat.dropImageOrPdf") }}
        </div>
      </div>

      <div
        ref="scrollContainer"
        class="ecall-chat-scroll-container relative flex flex-1 min-h-0 flex-col overflow-x-hidden overflow-y-auto p-3 scrollbar-gutter-stable"
        :data-chat-interaction-locked="chatting || frozen ? 'true' : undefined"
        @scroll="onScroll"
      >
        <div v-if="hasActiveOrPendingTodo" class="sticky top-0 z-20 flex justify-center pt-1">
          <div
            class="dropdown dropdown-bottom pointer-events-auto"
            :aria-label="t('config.task.fields.todo')"
            @click.stop
            @mousedown.stop
          >
            <label
              tabindex="0"
              class="btn btn-sm max-w-[min(88vw,30rem)] flex-nowrap justify-start gap-2 overflow-hidden rounded-full border-base-300 bg-base-300 text-base-content hover:border-base-300 hover:bg-base-200 normal-case"
            >
              <ListTodo class="h-4 w-4 shrink-0 opacity-70" />
              <span class="min-w-0 flex-1 truncate text-left">{{ activeConversationTodo }}</span>
              <span
                v-if="normalizedConversationTodos.length > 1"
                class="badge badge-ghost badge-sm shrink-0"
              >+{{ normalizedConversationTodos.length - 1 }}</span>
            </label>
            <div
              v-if="normalizedConversationTodos.length > 1"
              tabindex="0"
              class="dropdown-content card card-compact mt-2 w-max max-w-[min(88vw,30rem)] border border-base-300 bg-base-100 shadow-xl"
            >
              <div class="card-body p-3">
                <ul class="flex flex-col gap-3">
                  <li
                    v-for="(item, index) in normalizedConversationTodos"
                    :key="`${item.status}-${index}-${item.content}`"
                    class="flex items-start gap-3"
                    :title="item.content"
                  >
                    <span
                      class="inline-flex h-7 min-w-7 shrink-0 items-center justify-center rounded-full text-sm font-semibold"
                      :class="todoIndexClass(item.status)"
                    >{{ index + 1 }}</span>
                    <span
                      class="min-w-0 wrap-break-word pt-0.5 text-sm leading-6"
                      :class="item.status === 'completed'
                        ? 'text-base-content/55 line-through'
                        : item.status === 'in_progress'
                          ? 'text-base-content font-semibold'
                          : 'text-base-content'"
                    >{{ item.content }}</span>
                  </li>
                </ul>
              </div>
            </div>
          </div>
        </div>

        <template v-for="item in chatRenderItems" :key="item.id">
          <div
            v-if="item.kind === 'compaction'"
            class="mt-4 flex items-center gap-3 text-[11px] text-base-content/45"
          >
            <div class="h-px flex-1 bg-base-300/80"></div>
            <button
              type="button"
              class="btn btn-ghost btn-xs shrink-0 gap-1.5 px-2 text-base-content/60 hover:text-base-content"
              :title="t('chat.viewSummary')"
              @click="openConversationSummary"
            >
              <History class="h-3.5 w-3.5" />
              <span>{{ t("chat.viewSummary") }}</span>
            </button>
            <div class="h-px flex-1 bg-base-300/80"></div>
          </div>
          <template v-else-if="item.kind === 'message'">
            <ChatMessageItem
              v-memo="messageMemoKey(item.block, item.renderId, item.blockIndex)"
              :block="item.block"
              :selection-key="item.renderId"
              :selection-mode-enabled="messageSelectionModeEnabled"
              :selected="selectedMessageRenderIdSet.has(item.renderId)"
              :chatting="chatting"
              :frozen="frozen"
              :user-alias="userAlias"
              :user-avatar-url="userAvatarUrl"
              :persona-name-map="personaNameMap"
              :persona-avatar-url-map="personaAvatarUrlMap"
              :stream-tool-calls="visibleStreamToolCalls"
              :markdown-is-dark="markdownIsDark"
              :playing-audio-id="playingAudioId"
              :active-turn-user="item.renderId === activeTurnUserId"
              :can-regenerate="canRegenerateBlock(item.block, item.blockIndex)"
              :can-confirm-plan="canConfirmPlan(item.block)"
              @recall-turn="$emit('recallTurn', $event)"
              @regenerate-turn="$emit('regenerateTurn', $event)"
              @confirm-plan="$emit('confirmPlan', $event)"
              @enter-selection-mode="enterMessageSelectionMode"
              @toggle-message-selected="toggleMessageSelected"
              @copy-message="copyMessage"
              @open-image-preview="openImagePreview"
              @toggle-audio-playback="toggleAudioPlayback($event.id, $event.audio)"
              @assistant-link-click="handleAssistantLinkClick"
            />
          </template>
          <div
            v-else
            class="ecall-turn-group"
            :data-active-turn-group="item.groupId === activeTurnGroupId ? 'true' : undefined"
          >
            <div
              class="ecall-turn-stack"
              :style="item.groupId === activeTurnGroupId ? { minHeight: `${activeTurnGroupMinHeight}px` } : undefined"
            >
              <template v-for="groupItem in item.items" :key="groupItem.renderId">
                <ChatMessageItem
                  v-memo="messageMemoKey(groupItem.block, groupItem.renderId, groupItem.blockIndex)"
                  :block="groupItem.block"
                  :selection-key="groupItem.renderId"
                  :selection-mode-enabled="messageSelectionModeEnabled"
                  :selected="selectedMessageRenderIdSet.has(groupItem.renderId)"
                  :chatting="chatting"
                  :frozen="frozen"
                  :user-alias="userAlias"
                  :user-avatar-url="userAvatarUrl"
                  :persona-name-map="personaNameMap"
                  :persona-avatar-url-map="personaAvatarUrlMap"
                  :stream-tool-calls="visibleStreamToolCalls"
                  :markdown-is-dark="markdownIsDark"
                  :playing-audio-id="playingAudioId"
                  :active-turn-user="groupItem.renderId === activeTurnUserId"
                  :can-regenerate="canRegenerateBlock(groupItem.block, groupItem.blockIndex)"
                  :can-confirm-plan="canConfirmPlan(groupItem.block)"
                  @recall-turn="$emit('recallTurn', $event)"
                  @regenerate-turn="$emit('regenerateTurn', $event)"
                  @confirm-plan="$emit('confirmPlan', $event)"
                  @enter-selection-mode="enterMessageSelectionMode"
                  @toggle-message-selected="toggleMessageSelected"
                  @copy-message="copyMessage"
                  @open-image-preview="openImagePreview"
                  @toggle-audio-playback="toggleAudioPlayback($event.id, $event.audio)"
                  @assistant-link-click="handleAssistantLinkClick"
                />
              </template>
            </div>
          </div>
        </template>

        <div ref="toolbarContainer" class="pt-1 pb-2">
          <ChatWorkspaceToolbar
            :chatting="chatting"
            :frozen="frozen"
            :workspace-button-label="t('chat.allowedWorkspaceButton')"
            :workspace-button-name="currentWorkspaceName"
            :persona-presence-chips="personaPresenceChips"
            :mentionable-agent-ids="mentionableAgentIds"
            :selected-mention-agent-ids="selectedMentionAgentIds"
            :supervision-active="supervisionActive"
            :supervision-label="t('chat.supervision.button')"
            :supervision-active-label="t('chat.supervision.buttonActive')"
            :supervision-title="supervisionButtonTitle"
            :review-button-label="toolReviewButtonLabel"
            :review-panel-open="toolReviewPanelOpen"
            :review-button-enabled="toolReviewButtonEnabled"
            @lock-workspace="$emit('lockWorkspace')"
            @mention-persona="agentId => {
              const normalizedAgentId = String(agentId || '').trim();
              if (!normalizedAgentId) return;
              if (selectedMentionAgentIds.includes(normalizedAgentId)) {
                emit('removeMention', normalizedAgentId);
                return;
              }
              const match = mentionOptions.find((item) => String(item.agentId || '').trim() === normalizedAgentId);
              if (match) emit('addMention', match);
            }"
            @open-supervision-task="$emit('openSupervisionTask')"
            @toggle-tool-review="toggleToolReviewPanel"
          />
        </div>
      </div>

      <div
        v-show="showJumpToBottom"
        class="pointer-events-none absolute bottom-3 right-5 z-30 flex justify-end"
        :style="jumpToBottomStyle"
      >
        <button
          class="btn btn-sm btn-circle btn-primary pointer-events-auto shadow-lg"
          :title="t('chat.jumpToBottom')"
          @click="jumpToBottom"
        >
          <ChevronsDown class="h-4 w-4" />
        </button>
      </div>

      <div
        v-if="organizingContextBannerText"
        class="shrink-0 border-t border-base-300 bg-base-100 px-3 py-2"
      >
        <div class="flex items-center gap-2 px-1 py-1 text-[12px] text-base-content/65">
          <span class="loading loading-spinner loading-xs text-base-content/50"></span>
          <span>{{ organizingContextBannerText }}</span>
        </div>
      </div>

      <div ref="composerContainer" class="shrink-0 border-t border-base-300 bg-base-100 p-2">
        <ChatComposerPanel
          ref="composerPanelRef"
          :selection-mode-enabled="messageSelectionModeEnabled"
          :selected-message-count="selectedMessageBlocks.length"
          :chat-input="chatInput"
          :instruction-presets="instructionPresets"
          :mention-options="mentionOptions"
          :selected-mentions="selectedMentions"
          :chat-input-placeholder="chatInputPlaceholder"
          :clipboard-images="clipboardImages"
          :queued-attachment-notices="queuedAttachmentNotices"
          :link-open-error-text="linkOpenErrorText"
          :chat-error-text="chatErrorText"
          :transcribing="transcribing"
          :can-record="canRecord"
          :recording="recording"
          :recording-ms="recordingMs"
          :record-hotkey="recordHotkey"
          :selected-chat-model-id="selectedChatModelId"
          :chat-model-options="chatModelOptions"
          :plan-mode-enabled="planModeEnabled"
          :chat-usage-percent="chatUsagePercent"
          :force-archive-tip="forceArchiveTip"
          :chatting="chatting"
          :frozen="frozen"
          :show-side-conversation-list="showSideConversationList"
          :active-conversation-id="activeConversationId"
          :unarchived-conversation-items="unarchivedConversationItems"
          :user-alias="userAlias"
          :user-avatar-url="userAvatarUrl"
          :persona-name-map="personaNameMap"
          :persona-avatar-url-map="personaAvatarUrlMap"
          :create-conversation-department-options="createConversationDepartmentOptions"
          :default-create-conversation-department-id="defaultCreateConversationDepartmentId"
          @update:chat-input="$emit('update:chatInput', $event)"
          @add-mention="$emit('addMention', $event)"
          @remove-mention="$emit('removeMention', $event)"
          @remove-clipboard-image="$emit('removeClipboardImage', $event)"
          @remove-queued-attachment-notice="$emit('removeQueuedAttachmentNotice', $event)"
          @start-recording="$emit('startRecording')"
          @stop-recording="$emit('stopRecording')"
          @pick-attachments="$emit('pickAttachments')"
          @update:selected-chat-model-id="$emit('update:selectedChatModelId', $event)"
          @update:plan-mode-enabled="$emit('update:planModeEnabled', $event)"
          @send-chat="$emit('sendChat')"
          @stop-chat="$emit('stopChat')"
          @exit-selection-mode="exitMessageSelectionMode"
          @selection-action-copy="copySelectedMessages"
          @selection-action-derive="emitSelectionAction('derive')"
          @selection-action-deliver="emitSelectionAction('deliver', $event)"
          @selection-action-share="emitSelectionAction('share')"
          @force-archive="$emit('forceArchive')"
          @switch-conversation="$emit('switchConversation', $event)"
          @create-conversation="$emit('createConversation', $event)"
        />
      </div>

      <ChatImagePreviewDialog
        :open="imagePreviewOpen"
        :data-url="imagePreviewDataUrl"
        :zoom="imagePreviewZoom"
        :min-zoom="IMAGE_PREVIEW_MIN_ZOOM"
        :max-zoom="IMAGE_PREVIEW_MAX_ZOOM"
        :offset-x="previewOffsetX"
        :offset-y="previewOffsetY"
        :dragging="previewDragging"
        @close="closeImagePreview"
        @zoom-in="zoomInPreview"
        @zoom-out="zoomOutPreview"
        @reset="resetPreviewZoom"
        @wheel="onPreviewWheel"
        @pointer-down="onPreviewPointerDown"
        @pointer-move="onPreviewPointerMove"
        @pointer-up="onPreviewPointerUp"
      />

      <ChatSupervisionTaskDialog
        :open="supervisionDialogOpen"
        :saving="supervisionTaskSaving"
        :error-text="supervisionTaskError"
        :active-task="activeSupervisionTask"
        :recent-history="recentSupervisionTaskHistory"
        @close="$emit('closeSupervisionTask')"
        @save="$emit('saveSupervisionTask', $event)"
      />
      </div>
        <ToolReviewSidebar
          v-if="toolReviewPanelOpen"
          class="w-104 max-w-[42vw] shrink-0 border-l border-base-300 bg-base-100 pt-2"
          :batches="toolReviewBatches"
          :current-batch-key="toolReviewCurrentBatchKey"
        :detail-map="toolReviewDetailMap"
        :detail-loading-call-id="toolReviewDetailLoadingCallId"
          :reviewing-call-id="toolReviewReviewingCallId"
          :batch-reviewing-key="toolReviewBatchReviewingKey"
          :submitting-batch-key="toolReviewSubmittingBatchKey"
          :error-text="toolReviewErrorText"
          :report-error-text="toolReviewReportErrorText"
          :markdown-is-dark="markdownIsDark"
          @select-batch="setToolReviewCurrentBatchKey"
          @load-item-detail="loadToolReviewItemDetail"
          @review-item="runToolReviewForCall"
        @review-batch="runToolReviewForBatch"
        @submit-batch="submitToolReviewBatch"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, toRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme } from "../../shell/composables/use-app-theme";
import { ChevronsDown, History, ListTodo } from "lucide-vue-next";
import "markstream-vue/index.css";
import { invokeTauri } from "../../../services/tauri-api";
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMentionTarget, ChatMessageBlock, ChatPersonaPresenceChip, ChatTodoItem, PromptCommandPreset } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatConversationSidebar from "../components/ChatConversationSidebar.vue";
import ChatWorkspaceToolbar from "../components/ChatWorkspaceToolbar.vue";
import ToolReviewSidebar from "../components/ToolReviewSidebar.vue";
import ChatImagePreviewDialog from "../components/dialogs/ChatImagePreviewDialog.vue";
import ChatSupervisionTaskDialog from "../components/dialogs/ChatSupervisionTaskDialog.vue";
import { useChatImagePreview } from "../composables/use-chat-image-preview";
import { useChatMessageActions } from "../composables/use-chat-message-actions";
import { useChatScrollLayout } from "../composables/use-chat-scroll-layout";
import { useChatToolReview } from "../composables/use-chat-tool-review";
import { isAbsoluteLocalPath, normalizeLocalLinkHref } from "../utils/local-link";

type ChatRenderItem =
  | { kind: "compaction"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "message"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "group"; id: string; groupId: string; items: Array<{ renderId: string; block: ChatMessageBlock; blockIndex: number }> };

const props = defineProps<{
  userAlias: string;
  personaName: string;
  userAvatarUrl: string;
  assistantAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  personaPresenceChips: ChatPersonaPresenceChip[];
  mentionOptions: ChatMentionTarget[];
  selectedMentions: ChatMentionTarget[];
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  queuedAttachmentNotices: Array<{ id: string; fileName: string; relativePath: string; mime: string }>;
  chatInput: string;
  instructionPresets: PromptCommandPreset[];
  chatInputPlaceholder: string;
  canRecord: boolean;
  recording: boolean;
  recordingMs: number;
  transcribing: boolean;
  recordHotkey: string;
  selectedChatModelId: string;
  toolReviewRefreshTick: number;
  chatModelOptions: ApiConfigItem[];
  planModeEnabled: boolean;
  chatUsagePercent: number;
  forceArchiveTip: string;
  mediaDragActive: boolean;
  chatting: boolean;
  frozen: boolean;
  messageBlocks: ChatMessageBlock[];
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  currentWorkspaceName: string;
  activeConversationId: string;
  currentTodos: ChatTodoItem[];
  supervisionActive: boolean;
  supervisionTitle: string;
  supervisionDialogOpen: boolean;
  supervisionTaskSaving: boolean;
  supervisionTaskError: string;
  activeSupervisionTask: {
    taskId: string;
    goal: string;
    why: string;
    todo: string;
    endAtLocal: string;
    remainingHours: number;
  } | null;
  recentSupervisionTaskHistory: Array<{
    goal: string;
    why: string;
    todo: string;
    durationHours: number;
  }>;
  currentTheme: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  createConversationDepartmentOptions: Array<{ id: string; name: string; ownerName: string }>;
  defaultCreateConversationDepartmentId: string;
}>();

const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));
function isOrganizeContextToolCall(call: { name: string; argsText: string; status?: "doing" | "done" }): boolean {
  const name = String(call.name || "").trim().toLowerCase();
  if (name === "organize_context" || name === "archive") return true;
  return false;
}

const visibleStreamToolCalls = computed(() =>
  props.streamToolCalls.filter((call) => !isOrganizeContextToolCall(call))
);

const normalizedConversationTodos = computed(() => {
  const todos = Array.isArray(props.currentTodos) ? props.currentTodos : [];
  return todos
    .map((item) => ({
      content: String(item?.content || "").trim(),
      status: String(item?.status || "").trim() as ChatTodoItem["status"],
    }))
    .filter((item) => item.content && (item.status === "pending" || item.status === "in_progress" || item.status === "completed"));
});

const hasActiveOrPendingTodo = computed(() => {
  return normalizedConversationTodos.value.some(item => item.status === "pending" || item.status === "in_progress");
});

const activeConversationTodoIndex = computed(() => {
  const todos = normalizedConversationTodos.value;
  const inProgressIndex = todos.findIndex((item) => item.status === "in_progress");
  if (inProgressIndex >= 0) return inProgressIndex;
  const pendingIndex = todos.findIndex((item) => item.status === "pending");
  if (pendingIndex >= 0) return pendingIndex;
  return todos.length ? 0 : -1;
});

const activeConversationTodo = computed(() => {
  const index = activeConversationTodoIndex.value;
  if (index < 0) return "";
  return String(normalizedConversationTodos.value[index]?.content || "").trim();
});

const supervisionButtonTitle = computed(() => {
  const baseTitle = props.supervisionActive
    ? t("chat.supervision.activeButtonTitle")
    : t("chat.supervision.buttonTitle");
  const detail = String(props.supervisionTitle || "").trim();
  return detail ? `${baseTitle}\n${detail}` : baseTitle;
});

function todoIndexClass(status: ChatTodoItem["status"]): string {
  if (status === "completed") return "bg-success text-success-content";
  if (status === "in_progress") return "bg-primary text-primary-content";
  return "bg-base-200 text-base-content/70";
}

const organizingContextBannerText = computed(() => {
  if (props.toolStatusState !== "running") return "";
  const statusText = String(props.toolStatusText || "").trim();
  if (statusText.includes("整理上下文") || statusText.includes("自动整理")) {
    return statusText;
  }
  if (props.streamToolCalls.some((call) => isOrganizeContextToolCall(call))) {
    return "正在整理上下文...";
  }
  return "";
});

const chatRenderItems = computed<ChatRenderItem[]>(() => {
  const items: ChatRenderItem[] = [];
  let currentGroup: Extract<ChatRenderItem, { kind: "group" }> | null = null;

  const flushGroup = () => {
    if (!currentGroup) return;
    items.push(currentGroup);
    currentGroup = null;
  };

  props.messageBlocks.forEach((block, blockIndex) => {
    const renderId = blockRenderId(block, blockIndex);
    if (isCompactionBlock(block)) {
      flushGroup();
      items.push({ kind: "compaction", id: `compaction-${renderId}`, renderId, block, blockIndex });
      return;
    }
    if (isRightAlignedMessage(block)) {
      flushGroup();
      currentGroup = {
        kind: "group",
        id: `group-${renderId}`,
        groupId: renderId,
        items: [{ renderId, block, blockIndex }],
      };
      return;
    }
    if (currentGroup) {
      currentGroup.items.push({ renderId, block, blockIndex });
      return;
    }
    items.push({ kind: "message", id: `message-${renderId}`, renderId, block, blockIndex });
  });
  flushGroup();
  return items;
});
const activeTurnGroupId = computed(() => {
  for (let idx = chatRenderItems.value.length - 1; idx >= 0; idx -= 1) {
    const item = chatRenderItems.value[idx];
    if (item.kind !== "group") continue;
    return item.groupId;
  }
  return "";
});
const activeTurnUserId = computed(() => activeTurnGroupId.value);
const mentionableAgentIds = computed(() =>
  props.mentionOptions
    .map((item) => String(item?.agentId || "").trim())
    .filter((value, index, list) => !!value && list.indexOf(value) === index),
);
const selectedMentionAgentIds = computed(() =>
  (Array.isArray(props.selectedMentions) ? props.selectedMentions : [])
    .map((item) => String(item?.agentId || "").trim())
    .filter((value, index, list) => !!value && list.indexOf(value) === index),
);
const latestPendingPlanMessageId = computed(() => {
  for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
    const block = props.messageBlocks[idx];
    if (block.isExtraTextBlock) continue;
    const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
    const messageMeta = ((providerMeta.message_meta || providerMeta.messageMeta || {}) as Record<string, unknown>);
    const messageKind = String(messageMeta.kind || providerMeta.messageKind || "").trim();
    if (messageKind === "plan_complete" || block.planCard?.action === "complete") {
      return "";
    }
    if (messageKind === "plan_present" || block.planCard?.action === "present") {
      return String(block.sourceMessageId || block.id || "").trim();
    }
  }
  return "";
});

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "update:selectedInstructionPrompts", value: PromptCommandPreset[]): void;
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", agentId: string): void;
  (e: "sideConversationListVisibleChange", value: boolean): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "update:selectedChatModelId", value: string): void;
  (e: "update:planModeEnabled", value: boolean): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "forceArchive"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "confirmPlan", payload: { messageId: string }): void;
  (e: "lockWorkspace"): void;
  (e: "openSupervisionTask"): void;
  (e: "closeSupervisionTask"): void;
  (e: "saveSupervisionTask", payload: { durationHours: number; goal: string; why: string; todo: string }): void;
  (e: "switchConversation", conversationId: string): void;
  (e: "renameConversation", payload: { conversationId: string; title: string }): void;
  (e: "createConversation", input?: { title?: string; departmentId?: string }): void;
  (e: "openConversationSummary", conversationId: string): void;
  (e: "reachedBottom"): void;
  (e: "refreshToolReviewMessages"): void;
  (e: "selectionActionCopy", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
  (e: "selectionActionCopyError", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; error: string }): void;
  (e: "selectionActionDerive", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
  (e: "selectionActionDeliver", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; targetConversationId: string }): void;
  (e: "selectionActionShare", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
}>();
const { t } = useI18n();

const linkOpenErrorText = ref("");
const composerPanelRef = ref<{ focusInput: (options?: FocusOptions) => void } | null>(null);
const messageSelectionModeEnabled = ref(false);
const selectedMessageRenderIds = ref<string[]>([]);

const {
  scrollContainer,
  composerContainer,
  toolbarContainer,
  chatLayoutRoot,
  activeTurnGroupMinHeight,
  showJumpToBottom,
  jumpToBottomStyle,
  showSideConversationList,
  onScroll,
  jumpToBottom,
} = useChatScrollLayout({
  activeConversationId: toRef(props, "activeConversationId"),
  chatting: toRef(props, "chatting"),
  frozen: toRef(props, "frozen"),
  messageBlockCount: computed(() => props.messageBlocks.length),
  lastMessageIsOwn: computed(() => {
    const lastBlock = props.messageBlocks[props.messageBlocks.length - 1];
    return lastBlock ? isRightAlignedMessage(lastBlock) : false;
  }),
  latestOwnMessageAlignRequest: toRef(props, "latestOwnMessageAlignRequest"),
  conversationScrollToBottomRequest: toRef(props, "conversationScrollToBottomRequest"),
  onReachedBottom: () => emit("reachedBottom"),
  focusComposerInput: (options) => composerPanelRef.value?.focusInput(options),
});

watch(
  showSideConversationList,
  (value) => {
    emit("sideConversationListVisibleChange", value);
  },
  { immediate: true },
);
watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    exitMessageSelectionMode();
  },
);
const {
  imagePreviewOpen,
  imagePreviewDataUrl,
  imagePreviewZoom,
  IMAGE_PREVIEW_MIN_ZOOM,
  IMAGE_PREVIEW_MAX_ZOOM,
  previewOffsetX,
  previewOffsetY,
  previewDragging,
  zoomInPreview,
  zoomOutPreview,
  resetPreviewZoom,
  onPreviewWheel,
  openImagePreview,
  closeImagePreview,
  onPreviewPointerDown,
  onPreviewPointerMove,
  onPreviewPointerUp,
} = useChatImagePreview();
const {
  playingAudioId,
  copyMessage,
  stopAudioPlayback,
  toggleAudioPlayback,
} = useChatMessageActions();
const selectedMessageRenderIdSet = computed(() => new Set(selectedMessageRenderIds.value));
const renderedMessageItems = computed(() =>
  chatRenderItems.value.flatMap((item) => {
    if (item.kind === "message") {
      return [{ renderId: item.renderId, block: item.block }];
    }
    if (item.kind === "group") {
      return item.items.map((groupItem) => ({ renderId: groupItem.renderId, block: groupItem.block }));
    }
    return [];
  }),
);
const selectedMessageBlocks = computed(() =>
  renderedMessageItems.value.filter((item) => selectedMessageRenderIdSet.value.has(item.renderId)),
);
watch(
  () => ({
    selectionModeEnabled: messageSelectionModeEnabled.value,
    selectedRenderIdCount: selectedMessageRenderIds.value.length,
    selectedBlockCount: selectedMessageBlocks.value.length,
  }),
  ({ selectionModeEnabled, selectedRenderIdCount, selectedBlockCount }) => {
    if (!selectionModeEnabled) return;
    if (selectedRenderIdCount === 0) return;
    if (selectedBlockCount === 0) {
      exitMessageSelectionMode();
    }
  },
);
const {
  toolReviewPanelOpen,
  toolReviewBatches,
  toolReviewCurrentBatchKey,
  toolReviewButtonLabel,
  toolReviewButtonEnabled,
  toolReviewDetailMap,
  toolReviewDetailLoadingCallId,
  toolReviewReviewingCallId,
  toolReviewBatchReviewingKey,
  toolReviewSubmittingBatchKey,
  toolReviewErrorText,
  toolReviewReportErrorText,
  toggleToolReviewPanel,
  refreshToolReviewBatches,
  setToolReviewCurrentBatchKey,
  loadToolReviewItemDetail,
  runToolReviewForCall,
  runToolReviewForBatch,
  submitToolReviewBatch,
} = useChatToolReview({
  activeConversationId: toRef(props, "activeConversationId"),
  selectedChatModelId: toRef(props, "selectedChatModelId"),
  messageBlocks: computed(() => props.messageBlocks),
  refreshTick: toRef(props, "toolReviewRefreshTick"),
  t,
  onRefreshMessages: () => emit("refreshToolReviewMessages"),
});

function isOwnMessage(block: ChatMessageBlock): boolean {
  return isRightAlignedMessage(block);
}

function isRightAlignedMessage(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  if (block.role === "user") return true;
  const id = String(block.speakerAgentId || "").trim();
  return id === "user-persona";
}

function blockRenderId(block: ChatMessageBlock, blockIndex: number): string {
  const rawId = String(block.id || "").trim();
  return rawId || `block-${blockIndex}`;
}

function messageMemoKey(block: ChatMessageBlock, renderId: string, blockIndex: number) {
  const selected = selectedMessageRenderIdSet.value.has(renderId);
  const activeTurnUser = renderId === activeTurnUserId.value;
  const canRegenerate = canRegenerateBlock(block, blockIndex);
  const canConfirm = canConfirmPlan(block);
  const requiresInteractionState = canRegenerate || canConfirm;
  return [
    block,
    markdownIsDark.value,
    playingAudioId.value,
    props.userAlias,
    props.userAvatarUrl,
    props.personaNameMap,
    props.personaAvatarUrlMap,
    messageSelectionModeEnabled.value,
    selected,
    activeTurnUser,
    canRegenerate,
    canConfirm,
    requiresInteractionState ? props.chatting : false,
    requiresInteractionState ? props.frozen : false,
  ];
}

function isCompactionBlock(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const meta = (block.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
  const kind = String(messageMeta.kind || "").trim();
  return kind === "context_compaction" || kind === "summary_context_seed";
}

function canRegenerateBlock(block: ChatMessageBlock, blockIndex: number): boolean {
  if (block.role !== "assistant" || block.isExtraTextBlock) return false;
  for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
    const candidate = props.messageBlocks[idx];
    if (candidate.role !== "assistant" || candidate.isExtraTextBlock) continue;
    return idx === blockIndex;
  }
  return false;
}

function canConfirmPlan(block: ChatMessageBlock): boolean {
  if (block.role !== "assistant" || block.isExtraTextBlock) return false;
  if (block.planCard?.action !== "present") return false;
  const targetId = String(block.sourceMessageId || block.id || "").trim();
  if (targetId !== latestPendingPlanMessageId.value) return false;
  const blockIndex = props.messageBlocks.findIndex((item) => String(item.id || "").trim() === String(block.id || "").trim());
  if (blockIndex < 0) return false;
  return !props.messageBlocks.slice(blockIndex + 1).some((item) => !item.isExtraTextBlock && item.role === "user");
}

function handleConversationListSelect(conversationId: string) {
  const normalizedConversationId = String(conversationId || "").trim();
  if (!normalizedConversationId) return;
  const isCurrent = normalizedConversationId === String(props.activeConversationId || "").trim();
  if (isCurrent) return;
  emit("switchConversation", normalizedConversationId);
}

function handleConversationRename(payload: { conversationId: string; title: string }) {
  const conversationId = String(payload?.conversationId || "").trim();
  const title = String(payload?.title || "").trim();
  if (!conversationId || !title) return;
  if (conversationId !== String(props.activeConversationId || "").trim()) return;
  emit("renameConversation", {
    conversationId,
    title,
  });
}

function openConversationSummary() {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return;
  emit("openConversationSummary", conversationId);
}

function enterMessageSelectionMode(selectionKey: string) {
  const normalizedSelectionKey = String(selectionKey || "").trim();
  if (!normalizedSelectionKey) return;
  messageSelectionModeEnabled.value = true;
  if (!selectedMessageRenderIds.value.includes(normalizedSelectionKey)) {
    selectedMessageRenderIds.value = [...selectedMessageRenderIds.value, normalizedSelectionKey];
  }
}

function toggleMessageSelected(selectionKey: string) {
  const normalizedSelectionKey = String(selectionKey || "").trim();
  if (!normalizedSelectionKey) return;
  if (!messageSelectionModeEnabled.value) {
    enterMessageSelectionMode(normalizedSelectionKey);
    return;
  }
  if (selectedMessageRenderIds.value.includes(normalizedSelectionKey)) {
    selectedMessageRenderIds.value = selectedMessageRenderIds.value.filter((item) => item !== normalizedSelectionKey);
    return;
  }
  selectedMessageRenderIds.value = [...selectedMessageRenderIds.value, normalizedSelectionKey];
}

function exitMessageSelectionMode() {
  messageSelectionModeEnabled.value = false;
  selectedMessageRenderIds.value = [];
}

function selectionPayload() {
  const blocks = selectedMessageBlocks.value.map((item) => item.block);
  return {
    count: blocks.length,
    messageIds: blocks
      .map((block) => String(block.sourceMessageId || block.id || "").trim())
      .filter((item) => !!item),
    blocks,
  };
}

function selectionDisplayName(block: ChatMessageBlock): string {
  if (block.remoteImOrigin) {
    return block.remoteImOrigin.senderName || block.remoteImOrigin.remoteContactName || "IM";
  }
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  if (speakerAgentId && props.personaNameMap[speakerAgentId]) {
    return props.personaNameMap[speakerAgentId];
  }
  if (!speakerAgentId || speakerAgentId === "user-persona" || block.role === "user") {
    return props.userAlias || t("archives.roleUser");
  }
  return speakerAgentId || block.role;
}

function selectionBlockSummary(block: ChatMessageBlock): string {
  const parts: string[] = [];
  const text = String(block.text || "").trim();
  if (text) {
    parts.push(text);
  }
  if (block.images.length > 0) {
    parts.push(t("chat.imageCount", { count: block.images.length }));
  }
  if (block.audios.length > 0) {
    parts.push(t("chat.audioCount", { count: block.audios.length }));
  }
  if (block.attachmentFiles.length > 0) {
    parts.push(
      t("chat.attachmentList", {
        names: block.attachmentFiles.map((item) => item.fileName).join("、"),
      }),
    );
  }
  return parts.join("\n").trim();
}

async function copySelectedMessages() {
  const payload = selectionPayload();
  if (payload.count === 0) return;
  const text = payload.blocks
    .map((block) => {
      const body = selectionBlockSummary(block);
      return `[${selectionDisplayName(block)}]: ${body || "[空消息]"}`;
    })
    .join("\n\n");
  if (!text.trim()) return;
  try {
    await navigator.clipboard.writeText(text);
    emit("selectionActionCopy", payload);
  } catch (error) {
    emit("selectionActionCopyError", {
      ...payload,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

function emitSelectionAction(kind: "derive" | "share" | "deliver", targetConversationId = "") {
  const payload = selectionPayload();
  if (payload.count === 0) return;
  if (kind === "derive") {
    emit("selectionActionDerive", payload);
    return;
  }
  if (kind === "deliver") {
    const normalizedTargetConversationId = String(targetConversationId || "").trim();
    if (!normalizedTargetConversationId) return;
    emit("selectionActionDeliver", {
      ...payload,
      targetConversationId: normalizedTargetConversationId,
    });
    return;
  }
  emit("selectionActionShare", payload);
}

async function handleAssistantLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = normalizeLocalLinkHref(anchor.getAttribute("href")?.trim() || "");
  if (!href) return;

  if (isAbsoluteLocalPath(href)) {
    event.preventDefault();
    event.stopPropagation();
    try {
      await invokeTauri("open_local_file_directory", { path: href });
      linkOpenErrorText.value = "";
    } catch (error) {
      linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
    }
    return;
  }

  if (href.startsWith("http://") || href.startsWith("https://")) {
    event.preventDefault();
    event.stopPropagation();
    try {
      await invokeTauri("open_external_url", { url: href });
      linkOpenErrorText.value = "";
    } catch (error) {
      linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
    }
  }
}

onBeforeUnmount(() => {
  stopAudioPlayback();
});

</script>

<style scoped>
.scrollbar-gutter-stable {
  scrollbar-gutter: stable;
}

.conversation-tray-scroll-hidden {
  -ms-overflow-style: none;
  scrollbar-width: none;
}

.conversation-tray-scroll-hidden::-webkit-scrollbar {
  display: none;
}

.ecall-chat-scroll-container {
  overscroll-behavior-y: contain;
  overflow-anchor: none;
}

.ecall-chat-scroll-container[data-chat-interaction-locked="true"] :deep(.ecall-message-footer),
.ecall-chat-scroll-container[data-chat-interaction-locked="true"] :deep(.ecall-message-footer-action),
.ecall-chat-scroll-container[data-chat-interaction-locked="true"] :deep(.ecall-message-recall-action) {
  opacity: 0 !important;
  pointer-events: none !important;
}

.ecall-chat-scroll-container[data-chat-interaction-locked="true"] :deep(.ecall-plan-confirm-action) {
  pointer-events: none !important;
}

.ecall-turn-group {
  display: block;
  width: 100%;
}

.ecall-turn-stack {
  display: flow-root;
  width: 100%;
  min-height: 0;
}

.ecall-chat-avatar-col {
  width: 1.75rem;
  min-width: 1.75rem;
}

.ecall-time-loading {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  transform: scale(0.82);
  transform-origin: right center;
}

.ecall-stream-block-enter {
  animation: ecall-stream-block-fade 140ms ease-out;
}

.ecall-stream-segments {
  display: block;
}

.ecall-stream-segment {
  display: block;
  margin: 0;
}

.ecall-stream-tail {
  opacity: 1;
}

.ecall-stream-content {
  opacity: 0;
  animation: ecall-stream-fade-in 1s ease-out forwards;
}

.ecall-stream-content-done {
  opacity: 1;
}

.ecall-shimmer-text {
  position: relative;
  display: inline-block;
  color: currentColor;
}

.ecall-reasoning-shimmer::after {
  content: attr(data-shimmer-text);
  position: absolute;
  inset: 0;
  pointer-events: none;
  color: transparent;
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    transparent 44%,
    rgb(255 255 255 / 0.92) 50%,
    transparent 56%,
    transparent 100%
  );
  background-size: 280px 100%;
  background-position: 280px 0;
  will-change: background-position;
  transform: translateZ(0);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: ecall-reasoning-shimmer 2.5s linear infinite;
}

@keyframes ecall-stream-fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes ecall-stream-block-fade {
  from {
    opacity: 0;
    transform: translateY(2px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes ecall-reasoning-shimmer {
  from {
    background-position: 280px 0;
  }
  to {
    background-position: -280px 0;
  }
}

:deep(.chat-bubble) {
  min-width: 0;
  min-height: 0;
}

.ecall-assistant-bubble {
  min-width: 3rem;
  min-height: 2.25rem;
}

.ecall-assistant-bubble-wide {
  width: 100%;
  max-width: 100%;
}

:deep(.chat-input-no-focus),
:deep(.chat-input-no-focus:hover),
:deep(.chat-input-no-focus:focus),
:deep(.chat-input-no-focus:focus-visible) {
  border: none !important;
  outline: none !important;
  box-shadow: none !important;
}

</style>
