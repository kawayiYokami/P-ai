<template>
  <div
    ref="chatLayoutRoot"
    class="h-full min-h-0"
    :class="showSideConversationList && !detachedChatWindow ? 'flex flex-row overflow-hidden' : 'flex flex-col relative'"
  >
    <ChatConversationSidebar
      v-if="showSideConversationList && !detachedChatWindow"
      :items="conversationItems || unarchivedConversationItems"
      :active-conversation-id="activeConversationId"
      :user-alias="userAlias"
      :user-avatar-url="userAvatarUrl"
      :persona-name-map="personaNameMap"
      :persona-avatar-url-map="personaAvatarUrlMap"
      @select="handleConversationListSelect"
      @rename="handleConversationRename"
      @toggle-pin-conversation="handleConversationPinToggle"
    />

    <div class="flex min-h-0 min-w-0 flex-1 overflow-hidden">
      <div class="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <div
          v-if="mediaDragActive && !chatting && !frozen && !conversationBusy"
          class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
        >
          <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
            {{ t("chat.dropImageOrPdf") }}
          </div>
        </div>

        <div
          ref="scrollContainer"
          class="ecall-chat-scroll-container relative flex flex-1 min-h-0 flex-col overflow-x-hidden overflow-y-auto px-0 py-3 scrollbar-gutter-stable"
          :class="chatting || frozen || conversationBusy ? 'pointer-events-auto' : ''"
          :data-chat-interaction-locked="chatting || frozen || conversationBusy ? 'true' : undefined"
          @scroll="onConversationScroll"
        >
          <div
            v-if="loadingOlderHistory"
            class="pointer-events-none sticky top-0 z-10 flex justify-center pb-2"
          >
            <div class="badge badge-ghost gap-2 border-base-300 bg-base-100/90 px-3 py-3 shadow-sm">
              <span class="loading loading-spinner loading-xs"></span>
              <span>{{ t("chat.loadingOlderMessages") }}</span>
            </div>
          </div>

          <div v-if="hasActiveOrPendingTodo" class="pointer-events-none sticky top-0 z-20 flex justify-center pt-1">
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
                <span class="min-w-0 flex-1 truncate text-left">{{ activeConversationTodoDisplay }}</span>
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
          <div class="ecall-chat-history-flow flex min-w-0 flex-col">
            <div
              class="relative min-w-0 w-full"
              :style="{ height: `${totalVirtualSize}px` }"
            >
              <div
                v-for="entry in virtualEntries"
                :key="entry.item.id"
                :data-index="entry.row.index"
                :data-render-item-id="entry.item.id"
                :ref="(el) => measureVirtualRow(entry.item.id, el)"
                class="absolute left-0 top-0 w-full ecall-chat-virtual-item"
                :style="{ transform: `translateY(${entry.row.start}px)` }"
              >
                <div
                  v-if="entry.item.kind === 'compaction'"
                  class="mt-4 flex items-center gap-3 text-[11px] text-base-content/45"
                >
                  <div class="h-px flex-1 bg-base-300/80"></div>
                  <button
                    type="button"
                    class="btn btn-ghost btn-xs shrink-0 gap-1.5 px-2 text-base-content/60 hover:text-base-content"
                    :title="t('chat.viewSummary')"
                    @click="openConversationSummary(entry.item.block, $event)"
                  >
                    <History class="h-3.5 w-3.5" />
                    <span>{{ t("chat.viewSummary") }}</span>
                  </button>
                  <div class="h-px flex-1 bg-base-300/80"></div>
                </div>

                <div
                  v-else-if="entry.item.kind === 'message'"
                  v-memo="messageMemoKey(entry.item.block, entry.item.renderId, entry.item.blockIndex)"
                >
                  <div
                    class="ecall-elastic-item-shell"
                    :style="entry.item.id === latestOwnElasticItemId ? { minHeight: `${latestOwnElasticMinHeight}px` } : undefined"
                  >
                    <ChatMessageItem
                      :block="entry.item.block"
                      :selection-key="entry.item.renderId"
                      :selection-mode-enabled="messageSelectionModeEnabled"
                      :selected="selectedMessageRenderIdSet.has(entry.item.renderId)"
                      :chatting="chatting"
                      :busy="conversationBusy"
                      :frozen="frozen"
                      :user-alias="userAlias"
                      :user-avatar-url="userAvatarUrl"
                      :persona-name-map="personaNameMap"
                      :persona-avatar-url-map="personaAvatarUrlMap"
                      :stream-tool-calls="visibleStreamToolCalls"
                      :markdown-is-dark="markdownIsDark"
                      :playing-audio-id="playingAudioId"
                      :active-turn-user="false"
                      :can-regenerate="canRegenerateBlock(entry.item.block, entry.item.blockIndex)"
                      :can-confirm-plan="canConfirmPlan(entry.item.block)"
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
                  </div>
                </div>

                <div
                  v-else
                  class="ecall-turn-group"
                >
                  <div
                    class="ecall-turn-stack"
                    :style="entry.item.id === latestOwnElasticItemId ? { minHeight: `${latestOwnElasticMinHeight}px` } : undefined"
                  >
                    <template v-for="groupItem in entry.item.items" :key="groupItem.renderId">
                      <ChatMessageItem
                        v-memo="messageMemoKey(groupItem.block, groupItem.renderId, groupItem.blockIndex)"
                        :block="groupItem.block"
                        :selection-key="groupItem.renderId"
                        :selection-mode-enabled="messageSelectionModeEnabled"
                        :selected="selectedMessageRenderIdSet.has(groupItem.renderId)"
                        :chatting="chatting"
                        :busy="conversationBusy"
                        :frozen="frozen"
                        :user-alias="userAlias"
                        :user-avatar-url="userAvatarUrl"
                        :persona-name-map="personaNameMap"
                        :persona-avatar-url-map="personaAvatarUrlMap"
                        :stream-tool-calls="visibleStreamToolCalls"
                        :markdown-is-dark="markdownIsDark"
                        :playing-audio-id="playingAudioId"
                        :active-turn-user="false"
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
              </div>
            </div>

            <div ref="toolbarContainer" class="ecall-chat-toolbar-shell px-2 pt-1 pb-2">
              <ChatWorkspaceToolbar
                :chatting="chatting"
                :frozen="frozen"
                :conversation-busy="conversationBusy"
                :workspace-button-label="t('chat.allowedWorkspaceButton')"
                :workspace-button-name="currentWorkspaceName"
                :workspace-button-disabled="activeConversationSummary?.kind === 'remote_im_contact'"
                :hide-menu-button="activeConversationSummary?.kind === 'remote_im_contact'"
                :hide-workspace-button="activeConversationSummary?.kind === 'remote_im_contact'"
                :persona-presence-chips="personaPresenceChips"
                :mentionable-agent-ids="mentionableAgentIds"
                :selected-mention-agent-ids="selectedMentionAgentIds"
                :supervision-active="supervisionActive"
                :supervision-label="t('chat.supervision.button')"
                :supervision-active-label="t('chat.supervision.buttonActive')"
                :supervision-title="supervisionButtonTitle"
                :supervision-disabled="activeConversationSummary?.kind === 'remote_im_contact'"
                :review-button-label="toolReviewButtonLabel"
                :review-button-count="toolReviewButtonCount"
                :review-panel-open="toolReviewPanelOpen"
                :review-button-enabled="toolReviewButtonEnabled"
                :show-detach-button="!detachedChatWindow && !activeConversationSummary?.isMainConversation"
                :detach-disabled="!activeConversationId || activeConversationSummary?.isMainConversation || chatting || frozen || conversationBusy"
                @lock-workspace="$emit('lockWorkspace')"
                @open-branch-selection="openBranchSelectionMenu"
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
                @detach-conversation="handleDetachConversationRequest"
                @toggle-tool-review="toggleToolReviewPanel"
              />
            </div>
          </div>
        </div>
        <div
          v-if="conversationSummaryCard.visible"
          class="pointer-events-none absolute inset-x-0 top-4 z-30 flex justify-center px-3"
        >
          <div class="pointer-events-auto w-full max-w-xl rounded-box border border-base-300 bg-base-100 shadow-xl">
            <div class="flex items-center justify-between gap-3 border-b border-base-300 px-4 py-3">
              <div class="text-sm font-semibold">{{ t("chat.viewSummary") }}</div>
              <button
                type="button"
                class="btn btn-ghost btn-xs"
                @click="closeConversationSummaryCard"
              >
                {{ t("common.close") }}
              </button>
            </div>
            <div class="max-h-[min(52vh,28rem)] overflow-y-auto px-4 py-3">
              <div class="whitespace-pre-wrap wrap-break-word text-sm leading-6 text-base-content/85">
                {{ conversationSummaryCard.text }}
              </div>
            </div>
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
            @click="handleJumpToBottom"
          >
            <ChevronsDown class="h-4 w-4" />
          </button>
        </div>

        <div ref="composerContainer" class="relative shrink-0 border-t border-base-300 bg-base-100 p-2">
          <div
            v-if="chatStatusBanner"
            class="pointer-events-none absolute inset-x-0 top-0 z-10 -translate-y-full"
          >
            <div
              class="relative flex w-full items-center justify-center rounded-none px-4 py-1.5 text-center text-[12px] backdrop-blur-md"
              :class="chatStatusBanner.tone === 'error'
                ? 'bg-error/12 text-error'
                : chatStatusBanner.text === t('chat.statusCompactingContext')
                  ? 'bg-info/12 text-info'
                  : 'bg-base-200/75 text-base-content'"
            >
              <span
                class="relative z-1"
                :class="chatStatusBanner.tone === 'error'
                  ? ''
                  : 'text-base-content/80 ecall-shimmer-text ecall-reasoning-shimmer'"
                :data-shimmer-text="chatStatusBanner.tone === 'error' ? '' : chatStatusBanner.text"
              >{{ chatStatusBanner.text }}</span>
            </div>
          </div>
          <ChatApprovalPanel
            v-if="activeConversationTerminalApprovals.length > 0"
            :approvals="activeConversationTerminalApprovals"
            :resolving="terminalApprovalResolving"
            @approve="$emit('approveTerminalApproval', $event)"
            @deny="$emit('denyTerminalApproval', $event)"
          />
          <ChatComposerPanel
            v-else
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
            :frontend-round-phase="frontendRoundPhase"
            :chat-usage-percent="chatUsagePercent"
            :force-archive-tip="forceArchiveTip"
            :chatting="chatting"
            :busy="conversationBusy"
            :frozen="frozen"
            :show-side-conversation-list="detachedChatWindow ? false : showSideConversationList"
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
            @selection-action-branch="emitSelectionAction('branch')"
            @selection-action-forward="emitSelectionAction('forward', $event)"
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
        ref="toolReviewSidebarRef"
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
        :reports="toolReviewReports"
        :current-report-id="toolReviewCurrentReportId"
        :markdown-is-dark="markdownIsDark"
        :current-workspace-name="currentWorkspaceName"
        :current-workspace-root-path="currentWorkspaceRootPath"
        :workspaces="workspaces"
        :current-department-id="currentDepartmentId"
        :department-options="createConversationDepartmentOptions"
        @select-batch="setToolReviewCurrentBatchKey"
        @load-item-detail="loadToolReviewItemDetail"
        @review-item="runToolReviewForCall"
        @review-batch="runToolReviewForBatch"
        @submit-batch="submitToolReviewBatch"
        @submit-batch-selection="handleSubmitBatchSelection"
        @pick-commit-review="handlePickCommitReview"
        @review-code="handleToolReviewCode"
        @retry-report="handleRetryToolReviewReport"
        @delete-report="handleDeleteToolReviewReport"
        @copy-report="copyToolReviewReport"
        @attach-report="$emit('attachToolReviewReport', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, toRef, watch, type ComponentPublicInstance } from "vue";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme } from "../../shell/composables/use-app-theme";
import { ChevronsDown, History, ListTodo } from "lucide-vue-next";
import "markstream-vue/index.css";
import { invokeTauri } from "../../../services/tauri-api";
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMentionTarget, ChatMessageBlock, ChatPersonaPresenceChip, ChatTodoItem, PromptCommandPreset, ShellWorkspace } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatApprovalPanel from "../components/ChatApprovalPanel.vue";
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatConversationSidebar from "../components/ChatConversationSidebar.vue";
import ChatWorkspaceToolbar from "../components/ChatWorkspaceToolbar.vue";
import ToolReviewSidebar from "../components/ToolReviewSidebar.vue";
import ChatImagePreviewDialog from "../components/dialogs/ChatImagePreviewDialog.vue";
import ChatSupervisionTaskDialog from "../components/dialogs/ChatSupervisionTaskDialog.vue";
import { useChatImagePreview } from "../composables/use-chat-image-preview";
import { useChatMessageActions } from "../composables/use-chat-message-actions";
import { useChatScrollLayout } from "../composables/use-chat-scroll-layout";
import { useChatToolReview, type ToolReviewCodeReviewScope, type ToolReviewCommitOption, type ToolReviewReportRecord } from "../composables/use-chat-tool-review";
import type { TerminalApprovalConversationItem } from "../../shell/composables/use-terminal-approval";
import { isAbsoluteLocalPath, normalizeLocalLinkHref } from "../utils/local-link";

type ChatRenderItem =
  | { kind: "compaction"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "message"; id: string; renderId: string; block: ChatMessageBlock; blockIndex: number }
  | { kind: "group"; id: string; groupId: string; items: Array<{ renderId: string; block: ChatMessageBlock; blockIndex: number }> };

const MAX_GROUP_ITEM_COUNT = 2;
const FILE_READER_EXTENSIONS = new Set([
  "md",
  "markdown",
  "mdx",
  "ts",
  "tsx",
  "c",
  "cc",
  "cpp",
  "cxx",
  "h",
  "hpp",
  "cs",
  "java",
  "kt",
  "kts",
  "go",
  "js",
  "jsx",
  "vue",
  "rs",
  "py",
  "rb",
  "php",
  "swift",
  "scala",
  "dart",
  "lua",
  "r",
  "m",
  "mm",
  "pl",
  "pm",
  "json",
  "jsonc",
  "json5",
  "toml",
  "yaml",
  "yml",
  "css",
  "scss",
  "sass",
  "less",
  "html",
  "htm",
  "xml",
  "svg",
  "sql",
  "sh",
  "bash",
  "zsh",
  "fish",
  "ps1",
  "bat",
  "cmd",
  "dockerfile",
  "ini",
  "env",
  "gitignore",
  "gitattributes",
  "editorconfig",
  "lock",
  "csv",
  "tsv",
  "txt",
  "log",
]);

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
  frontendRoundPhase: "idle" | "queued" | "waiting" | "streaming";
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
  forcingArchive: boolean;
  compactingConversation: boolean;
  conversationBusy: boolean;
  frozen: boolean;
  messageBlocks: ChatMessageBlock[];
  hasMoreHistory: boolean;
  loadingOlderHistory: boolean;
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  currentWorkspaceName: string;
  currentWorkspaceRootPath: string;
  workspaces: ShellWorkspace[];
  currentDepartmentId: string;
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
  conversationItems?: ChatConversationOverviewItem[];
  createConversationDepartmentOptions: Array<{ id: string; name: string; ownerName: string; providerName?: string; modelName?: string }>;
  defaultCreateConversationDepartmentId: string;
  detachedChatWindow?: boolean;
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
}>();

const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));
const activeConversationSummary = computed(() => {
  const activeConversationId = String(props.activeConversationId || "").trim();
  if (!activeConversationId) return null;
  const matched = (props.conversationItems || props.unarchivedConversationItems).find(
    (item) => String(item.conversationId || "").trim() === activeConversationId,
  ) || null;
  return matched;
});
const ephemeralBlockRenderIdMap = new WeakMap<ChatMessageBlock, string>();
let ephemeralBlockRenderIdSeq = 0;

function isOrganizeContextToolCall(call: { name: string; argsText: string; status?: "doing" | "done" }): boolean {
  const name = String(call.name || "").trim().toLowerCase();
  if (name === "organize_context" || name === "archive") return true;
  return false;
}

function fileExtensionFromPath(path: string): string {
  const normalizedPath = String(path || "").trim().replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").filter(Boolean).pop() || "";
  const normalizedFileName = fileName.startsWith(".") ? fileName.slice(1) : fileName;
  const lowerFileName = normalizedFileName.toLowerCase();
  if (FILE_READER_EXTENSIONS.has(lowerFileName)) return lowerFileName;
  const dotIndex = normalizedFileName.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === normalizedFileName.length - 1) return "";
  return normalizedFileName.slice(dotIndex + 1).toLowerCase();
}

function canOpenInFileReader(path: string): boolean {
  return FILE_READER_EXTENSIONS.has(fileExtensionFromPath(path));
}

async function handlePickCommitReview(page = 1) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return;
  toolReviewSidebarRef.value?.setCommitOptions([], true, 0, page, 30);
  try {
    const result = await listToolReviewCommitOptions(conversationId, page, 30);
    toolReviewSidebarRef.value?.setCommitOptions(result.commits, false, result.total, result.page, result.pageSize);
  } catch (error) {
    const detail = error instanceof Error ? String(error.message || "").trim() : String(error);
    toolReviewErrorText.value = t("chat.toolReview.loadFailed", { err: detail || "Unknown error" });
    toolReviewSidebarRef.value?.setCommitOptions([], false, 0, page, 30);
  }
}

async function handleDeleteToolReviewReport(report: ToolReviewReportRecord) {
  const conversationId = String(props.activeConversationId || "").trim();
  const reportId = String(report.id || "").trim();
  if (!conversationId || !reportId) return;
  try {
    await deleteToolReviewReport({
      conversationId,
      reportId,
    });
  } catch (error) {
    const detail = error instanceof Error ? String(error.message || "").trim() : String(error);
    console.error("[工具审查][前端] 删除审查报告失败", {
      conversationId,
      reportId,
      error,
    });
    toolReviewErrorText.value = t("chat.toolReview.loadFailed", { err: detail || "删除审查报告失败" });
  }
}

async function handleSubmitBatchSelection(input: { batchKeys: string[]; departmentId: string }) {
  const batchKeys = Array.isArray(input.batchKeys) ? input.batchKeys : [];
  const departmentId = String(input.departmentId || props.currentDepartmentId || "").trim();
  const reversedBatches = [...toolReviewBatches.value].reverse();
  for (const batchKey of batchKeys) {
    const normalizedBatchKey = String(batchKey || "").trim();
    if (!normalizedBatchKey) continue;
    const selectionIndex = reversedBatches.findIndex((batch) => String(batch.batchKey || "").trim() === normalizedBatchKey);
    if (selectionIndex < 0) {
      toolReviewErrorText.value = `未找到对应批次，无法发起审查任务：${normalizedBatchKey}`;
      continue;
    }
    console.info("[工具审查][前端] 批次选择提交", {
      conversationId: String(props.activeConversationId || "").trim(),
      batchKey: normalizedBatchKey,
      batchNumber: selectionIndex + 1,
      departmentId,
    });
    await submitToolReviewBatch(selectionIndex + 1, departmentId);
  }
}

function handleToolReviewCode(input: { scope: ToolReviewCodeReviewScope; target?: string; departmentId: string }) {
  const conversationId = String(props.activeConversationId || "").trim();
  const scope = input.scope;
  const normalizedTarget = String(input.target || "").trim();
  const departmentId = String(input.departmentId || props.currentDepartmentId || "").trim();
  if (!conversationId) {
    toolReviewErrorText.value = "当前没有活跃会话，无法发起审查任务。";
    return;
  }
  if (!departmentId) {
    toolReviewErrorText.value = "当前没有可用部门，无法发起审查任务。";
    return;
  }
  console.info("[工具审查][前端] 发起代码审查任务", {
    conversationId,
    scope,
    target: normalizedTarget,
    departmentId,
  });
  void submitToolReviewCode({
    conversationId,
    scope,
    target: normalizedTarget || undefined,
    departmentId,
    apiConfigId: String(props.selectedChatModelId || "").trim() || undefined,
  });
}

async function handleRetryToolReviewReport(report: ToolReviewReportRecord) {
  const scope = String(report.scope || "").trim() as ToolReviewCodeReviewScope | "batch";
  const target = String(report.target || "").trim();
  const conversationId = String(props.activeConversationId || "").trim();
  const reportId = String(report.id || "").trim();
  if (!conversationId || !reportId) {
    toolReviewErrorText.value = "当前没有活跃会话，无法重新生成审查任务。";
    return;
  }
  try {
    await deleteToolReviewReport({
      conversationId,
      reportId,
    });
  } catch (error) {
    const detail = error instanceof Error ? String(error.message || "").trim() : String(error);
    console.error("[工具审查][前端] 删除旧审查报告失败，取消重新生成", {
      conversationId,
      reportId,
      scope,
      target,
      error,
    });
    toolReviewErrorText.value = t("chat.toolReview.loadFailed", { err: detail || "删除旧审查报告失败" });
    return;
  }
  if (scope === "batch") {
    const matched = /^第\s*(\d+)\s*批$/.exec(target);
    if (!matched) {
      toolReviewErrorText.value = `无法识别批次审查目标：${target || "空"}`;
      return;
    }
    const batchNumber = Number(matched[1]);
    if (!Number.isFinite(batchNumber) || batchNumber <= 0) {
      toolReviewErrorText.value = `无效批次序号：${target}`;
      return;
    }
    console.info("[工具审查][前端] 重新生成批次审查", {
      conversationId,
      reportId,
      batchNumber,
    });
    void submitToolReviewBatch(batchNumber, String(props.currentDepartmentId || "").trim());
    return;
  }
  if (scope === "commit" || scope === "main" || scope === "uncommitted" || scope === "custom") {
    console.info("[工具审查][前端] 重新生成代码审查", {
      conversationId,
      reportId,
      scope,
      target,
    });
    void submitToolReviewCode({
      conversationId,
      scope,
      target: target || undefined,
      departmentId: String(props.currentDepartmentId || "").trim(),
      apiConfigId: String(props.selectedChatModelId || "").trim() || undefined,
    });
    return;
  }
  toolReviewErrorText.value = `不支持重新生成该审查范围：${scope || "空"}`;
}

function handleViewApprovalDetail() {
  if (!toolReviewPanelOpen.value) {
    toggleToolReviewPanel();
  }
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

const activeConversationTodoDisplay = computed(() => {
  const todo = activeConversationTodo.value;
  if (!todo) return "";
  const personaName = String(props.personaName || "").trim();
  return personaName ? `${personaName} 打算${todo}` : `打算${todo}`;
});

const activeConversationTerminalApprovals = computed(() => {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return [];
  const approvals = Array.isArray(props.terminalApprovals) ? props.terminalApprovals : [];
  return approvals.filter((item) => String(item.conversationId || item.sessionId || "").trim() === conversationId);
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

const activeRunningToolCall = computed(() => {
  if (props.toolStatusState !== "running") return null;
  const calls = Array.isArray(props.streamToolCalls) ? props.streamToolCalls : [];
  for (let idx = calls.length - 1; idx >= 0; idx -= 1) {
    const call = calls[idx];
    if (String(call?.status || "").trim() === "done") continue;
    const name = String(call?.name || "").trim();
    if (!name) continue;
    return call;
  }
  return null;
});

const isOrganizingContextBusy = computed(() => {
  if (props.compactingConversation) return true;
  const runningTool = activeRunningToolCall.value;
  if (runningTool && isOrganizeContextToolCall(runningTool)) return true;
  const statusText = String(props.toolStatusText || "").trim();
  return props.toolStatusState === "running" && (
    statusText.includes("整理上下文") || statusText.includes("自动整理")
  );
});

const chatStatusBanner = computed<null | { text: string; tone: "default" | "error" }>(() => {
  const errorText = String(props.chatErrorText || "").trim();
  if (errorText) {
    return {
      text: errorText,
      tone: "error",
    };
  }
  if (props.forcingArchive) {
    return {
      text: t("chat.statusArchivingConversation"),
      tone: "default",
    };
  }
  if (isOrganizingContextBusy.value) {
    return {
      text: t("chat.statusCompactingContext"),
      tone: "default",
    };
  }
  return null;
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
    const renderId = blockRenderId(block);
    if (isCompactionBlock(block)) {
      flushGroup();
      items.push({ kind: "compaction", id: `compaction-${renderId}`, renderId, block, blockIndex });
      return;
    }
    if (isRightAlignedMessage(block)) {
      flushGroup();
      const groupId = blockGroupRenderId(block);
      currentGroup = {
        kind: "group",
        id: `group-${groupId}`,
        groupId,
        items: [{ renderId, block, blockIndex }],
      };
      return;
    }
    if (currentGroup) {
      if (currentGroup.items.length >= MAX_GROUP_ITEM_COUNT) {
        flushGroup();
        items.push({ kind: "message", id: `message-${renderId}`, renderId, block, blockIndex });
        return;
      }
      currentGroup.items.push({ renderId, block, blockIndex });
      return;
    }
    items.push({ kind: "message", id: `message-${renderId}`, renderId, block, blockIndex });
  });
  flushGroup();
  return items;
});
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
  (e: "detachConversation"): void;
  (e: "closeSupervisionTask"): void;
  (e: "saveSupervisionTask", payload: { durationHours: number; goal: string; why: string; todo: string }): void;
  (e: "switchConversation", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
  (e: "renameConversation", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
  (e: "createConversation", input?: { title?: string; departmentId?: string }): void;
  (e: "loadOlderHistory"): void;
  (e: "reachedBottom"): void;
  (e: "jumpToConversationBottom"): void;
  (e: "refreshToolReviewMessage", payload: { conversationId: string; messageId: string }): void;
  (e: "attachToolReviewReport", reportText: string): void;
  (e: "selectionActionCopy", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
  (e: "selectionActionCopyError", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; error: string }): void;
  (e: "selectionActionBranch", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
  (e: "selectionActionForward", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; targetConversationId: string }): void;
  (e: "selectionActionShare", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[] }): void;
  (e: "approveTerminalApproval", requestId: string): void;
  (e: "denyTerminalApproval", requestId: string): void;
}>();
const { t } = useI18n();
const toolReviewSidebarRef = ref<ComponentPublicInstance<{ setCommitOptions: (items: ToolReviewCommitOption[], loading?: boolean, total?: number, page?: number, pageSize?: number) => void }> | null>(null);

function handleDetachConversationRequest() {
  console.info("[独立聊天窗口][前端链路] ChatView 收到 detachConversation，继续派发到窗口容器", {
    activeConversationId: props.activeConversationId,
    detachedChatWindow: !!props.detachedChatWindow,
    isMainConversation: !!activeConversationSummary.value?.isMainConversation,
    conversationBusy: props.conversationBusy,
    frozen: props.frozen,
    chatting: props.chatting,
  });
  emit("detachConversation");
}

function openBranchSelectionMenu() {
  if (props.chatting || props.frozen || props.conversationBusy) return;
  messageSelectionModeEnabled.value = true;
  selectedMessageRenderIds.value = [];
  void nextTick(() => {
    composerPanelRef.value?.focusInput?.({ preventScroll: true });
  });
}

const linkOpenErrorText = ref("");
const conversationSummaryCard = ref<{ visible: boolean; text: string }>({
  visible: false,
  text: "",
});
const composerPanelRef = ref<{ focusInput: (options?: FocusOptions) => void } | null>(null);
const messageSelectionModeEnabled = ref(false);
const selectedMessageRenderIds = ref<string[]>([]);
const olderHistoryRequestPending = ref(false);
const LOAD_OLDER_HISTORY_THRESHOLD_PX = 96;
const observedVirtualItemElements = new Map<string, HTMLElement>();
const measuredVirtualItemHeights = new Map<string, number>();
let pendingMeasureFrame = 0;
let pendingPinToBottomFrame = 0;
let lastConversationScrollTop = 0;
const olderHistoryTriggerReady = ref(true);
const pendingOlderHistoryAnchor = ref<{ messageId: string; edge: "top" | "bottom"; offset: number } | null>(null);
const pendingOlderHistoryScrollRestore = ref<{ scrollTop: number; scrollHeight: number } | null>(null);

const {
  scrollContainer,
  composerContainer,
  toolbarContainer,
  chatLayoutRoot,
  latestOwnElasticMinHeight,
  showJumpToBottom,
  jumpToBottomStyle,
  showSideConversationList,
  onScroll,
} = useChatScrollLayout({
  activeConversationId: toRef(props, "activeConversationId"),
  chatting: toRef(props, "chatting"),
  busy: toRef(props, "conversationBusy"),
  frozen: toRef(props, "frozen"),
  messageBlockCount: computed(() => props.messageBlocks.length),
  conversationScrollToBottomRequest: toRef(props, "conversationScrollToBottomRequest"),
  onReachedBottom: () => emit("reachedBottom"),
  focusComposerInput: (options) => composerPanelRef.value?.focusInput(options),
});

function refreshObservedVirtualItemElements() {
  const validIds = new Set(virtualRenderItems.value.map((item) => item.id));
  for (const [itemId] of observedVirtualItemElements.entries()) {
    if (!validIds.has(itemId)) observedVirtualItemElements.delete(itemId);
  }
}

const virtualRenderItems = computed<ChatRenderItem[]>(() => [...chatRenderItems.value]);

const virtualizer = useVirtualizer(
  computed(() => ({
    count: virtualRenderItems.value.length,
    getScrollElement: () => scrollContainer.value,
    getItemKey: (index: number) => virtualRenderItems.value[index]?.id ?? `row-${index}`,
    estimateSize: (index: number) => estimateChatRenderItemHeight(virtualRenderItems.value[index]),
    measureElement: (element: Element, _entry: unknown, instance: any) => {
      const measuredHeight = (element as HTMLElement).scrollHeight;
      if (instance?.scrollDirection !== "backward") return measuredHeight;
      const indexAttr = Number((element as HTMLElement).getAttribute("data-index"));
      const cachedHeight = Number.isFinite(indexAttr) ? instance?.itemSizeCache?.get(indexAttr) : undefined;
      return typeof cachedHeight === "number" ? cachedHeight : measuredHeight;
    },
    overscan: 4,
  })),
);

function blockBelongsToMessageId(block: ChatMessageBlock, messageId: string): boolean {
  const normalizedMessageId = String(messageId || "").trim();
  if (!normalizedMessageId) return false;
  const sourceMessageId = String(block.sourceMessageId || "").trim();
  const blockId = String(block.id || "").trim();
  return sourceMessageId === normalizedMessageId || blockId === normalizedMessageId;
}

function isDraftUserMessageId(messageId: string): boolean {
  return String(messageId || "").trim().startsWith("__draft_user__:");
}

const latestOwnMessageId = computed(() => {
  for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
    const block = props.messageBlocks[idx];
    if (block.isExtraTextBlock) continue;
    if (!isOwnMessage(block)) continue;
    const messageId = String(block.sourceMessageId || block.id || "").trim();
    if (messageId) return messageId;
  }
  return "";
});

const latestOwnElasticItemId = computed(() => {
  const targetMessageId = latestOwnMessageId.value;
  if (!targetMessageId) return "";
  for (let idx = chatRenderItems.value.length - 1; idx >= 0; idx -= 1) {
    const item = chatRenderItems.value[idx];
    if (item.kind === "message") {
      if (blockBelongsToMessageId(item.block, targetMessageId)) return item.id;
      continue;
    }
    if (item.kind === "group") {
      if (item.items.some((groupItem) => blockBelongsToMessageId(groupItem.block, targetMessageId))) {
        return item.id;
      }
    }
  }
  return "";
});

const renderItemChronologicalIndexMap = computed(() => {
  const map = new Map<string, number>();
  chatRenderItems.value.forEach((item, index) => {
    map.set(item.id, index);
  });
  return map;
});
const blockChronologicalIndexMap = computed(() => {
  const map = new Map<string, number>();
  props.messageBlocks.forEach((block, index) => {
    const blockId = String(block.id || "").trim();
    if (!blockId || map.has(blockId)) return;
    map.set(blockId, index);
  });
  return map;
});

const virtualRows = computed(() => virtualizer.value.getVirtualItems());
const virtualEntries = computed(() => {
  return virtualRows.value
    .map((row) => {
      const item = virtualRenderItems.value[row.index];
      return item ? { row, item } : null;
    })
    .filter((entry): entry is { row: typeof virtualRows.value[number]; item: ChatRenderItem } => Boolean(entry));
});
const totalVirtualSize = computed(() => virtualizer.value.getTotalSize());

function scheduleVirtualMeasure() {
  if (pendingMeasureFrame) return;
  void nextTick(() => {
    if (pendingMeasureFrame) return;
    pendingMeasureFrame = requestAnimationFrame(() => {
      pendingMeasureFrame = 0;
      refreshObservedVirtualItemElements();
      virtualizer.value.measure();
    });
  });
}

function measureVirtualRow(itemId: string, element: Element | ComponentPublicInstance | null) {
  const normalizedItemId = String(itemId || "").trim();
  if (!element) {
    if (normalizedItemId) {
      observedVirtualItemElements.delete(normalizedItemId);
      measuredVirtualItemHeights.delete(normalizedItemId);
    }
    return;
  }
  const target = element instanceof Element ? element : ((element.$el as Element | undefined) ?? null);
  if (!target) {
    if (normalizedItemId) {
      observedVirtualItemElements.delete(normalizedItemId);
      measuredVirtualItemHeights.delete(normalizedItemId);
    }
    return;
  }
  virtualizer.value.measureElement(target);
  const resolvedItemId = normalizedItemId || String(target.getAttribute("data-render-item-id") || "").trim();
  if (resolvedItemId && target instanceof HTMLElement) {
    const nextHeight = Math.round(target.getBoundingClientRect().height);
    measuredVirtualItemHeights.set(resolvedItemId, nextHeight);
    observedVirtualItemElements.set(resolvedItemId, target);
  }
}

function pinToBottomOnNextLayout(smooth = false, reason = "unknown") {
  if (pendingPinToBottomFrame) {
    cancelAnimationFrame(pendingPinToBottomFrame);
    pendingPinToBottomFrame = 0;
  }
  void nextTick(() => {
    scheduleVirtualMeasure();
    pendingPinToBottomFrame = requestAnimationFrame(() => {
      pendingPinToBottomFrame = 0;
      const scrollEl = scrollContainer.value;
      if (!scrollEl) return;
      requestAnimationFrame(() => {
        scrollEl.scrollTo({
          top: scrollEl.scrollHeight,
          behavior: smooth ? "smooth" : "auto",
        });
        onScroll();
      });
    });
  });
}

function alignLatestOwnMessageToTop(behavior: ScrollBehavior = "smooth") {
  const scrollEl = scrollContainer.value;
  const itemId = String(latestOwnElasticItemId.value || "").trim();
  if (!scrollEl || !itemId) return;
  const wrapper = observedVirtualItemElements.get(itemId);
  if (!wrapper || !wrapper.isConnected) return;
  const containerRect = scrollEl.getBoundingClientRect();
  const wrapperRect = wrapper.getBoundingClientRect();
  const scrollStyles = window.getComputedStyle(scrollEl);
  const targetTop = parseFloat(scrollStyles.paddingTop || "0");
  const nextTop = scrollEl.scrollTop + (wrapperRect.top - containerRect.top) - targetTop;
  scrollEl.scrollTo({
    top: Math.max(0, nextTop),
    behavior,
  });
  onScroll();
}

function syncViewportMetrics() {
  scheduleVirtualMeasure();
}

function findRenderedMessageElement(messageId: string): HTMLElement | null {
  const scrollEl = scrollContainer.value;
  const normalizedId = String(messageId || "").trim();
  if (!scrollEl || !normalizedId) return null;
  const escapedId = typeof CSS !== "undefined" && typeof CSS.escape === "function"
    ? CSS.escape(normalizedId)
    : normalizedId.replace(/["\\]/g, "\\$&");
  return scrollEl.querySelector(`[data-message-id="${escapedId}"]`) as HTMLElement | null;
}

function resolveMessageAnchorElement(messageElement: HTMLElement | null): HTMLElement | null {
  if (!messageElement) return null;
  return (messageElement.querySelector("[data-message-avatar-anchor='true']") as HTMLElement | null) || messageElement;
}

function captureVisibleAnchor(edge: "top" | "bottom"): { messageId: string; edge: "top" | "bottom"; offset: number } | null {
  const scrollEl = scrollContainer.value;
  if (!scrollEl) return null;
  const containerRect = scrollEl.getBoundingClientRect();
  let anchor: { messageId: string; offset: number; chronologicalIndex: number } | null = null;
  for (const entry of virtualEntries.value) {
    const itemId = String(entry.item.id || "").trim();
    if (!itemId) continue;
    const wrapper = observedVirtualItemElements.get(itemId);
    if (!wrapper || !wrapper.isConnected) continue;
    const messageElements = Array.from(wrapper.querySelectorAll("[data-message-id]")) as HTMLElement[];
    for (const element of messageElements) {
      const messageId = String(element.getAttribute("data-message-id") || "").trim();
      if (!messageId) continue;
      const anchorElement = resolveMessageAnchorElement(element);
      if (!anchorElement) continue;
      const rect = anchorElement.getBoundingClientRect();
      if (rect.bottom <= containerRect.top + 1 || rect.top >= containerRect.bottom - 1) continue;
      const chronologicalIndex = blockChronologicalIndexMap.value.get(messageId);
      if (chronologicalIndex === undefined) continue;
      const offset = edge === "bottom"
        ? containerRect.bottom - rect.bottom
        : rect.top - containerRect.top;
      if (!anchor) {
        anchor = { messageId, offset, chronologicalIndex };
        continue;
      }
      const shouldReplace = edge === "bottom"
        ? chronologicalIndex > anchor.chronologicalIndex
        : chronologicalIndex < anchor.chronologicalIndex;
      if (shouldReplace) {
        anchor = { messageId, offset, chronologicalIndex };
      }
    }
  }
  return anchor ? { messageId: anchor.messageId, edge, offset: anchor.offset } : null;
}

function onConversationScroll() {
  const scrollEl = scrollContainer.value;
  onScroll();
  maybeRequestOlderHistory();
  if (scrollEl) {
    lastConversationScrollTop = scrollEl.scrollTop;
  }
}

function handleJumpToBottom() {
  emit("jumpToConversationBottom");
}

function maybeRequestOlderHistory() {
  const scrollEl = scrollContainer.value;
  if (!scrollEl) return;
  if (!props.hasMoreHistory || props.loadingOlderHistory || olderHistoryRequestPending.value) return;
  if (!olderHistoryTriggerReady.value) return;
  if (scrollEl.scrollTop > LOAD_OLDER_HISTORY_THRESHOLD_PX) return;
  const isMovingUpward = scrollEl.scrollTop <= lastConversationScrollTop;
  if (!isMovingUpward) return;
  pendingOlderHistoryScrollRestore.value = {
    scrollTop: scrollEl.scrollTop,
    scrollHeight: scrollEl.scrollHeight,
  };
  pendingOlderHistoryAnchor.value = captureVisibleAnchor("bottom");
  olderHistoryRequestPending.value = true;
  olderHistoryTriggerReady.value = false;
  emit("loadOlderHistory");
}

watch(
  showSideConversationList,
  (value) => {
    emit("sideConversationListVisibleChange", value);
    syncViewportMetrics();
  },
  { immediate: true },
);

function handleConversationPinToggle(conversationId: string) {
  emit("togglePinConversation", String(conversationId || "").trim());
}
watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    exitMessageSelectionMode();
    pinToBottomOnNextLayout(false, "activeConversationChanged");
    olderHistoryRequestPending.value = false;
    lastConversationScrollTop = 0;
    olderHistoryTriggerReady.value = true;
    pendingOlderHistoryAnchor.value = null;
    pendingOlderHistoryScrollRestore.value = null;
  },
  { immediate: true },
);

watch(
  () => props.conversationScrollToBottomRequest,
  (nextValue, prevValue) => {
    if (!nextValue || nextValue === prevValue) return;
    pinToBottomOnNextLayout(false, "externalScrollRequest");
  },
);

watch(
  () => props.latestOwnMessageAlignRequest,
  (nextValue, prevValue) => {
    if (!nextValue || nextValue === prevValue) return;
    if (isDraftUserMessageId(latestOwnMessageId.value)) return;
    void nextTick(async () => {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      refreshObservedVirtualItemElements();
      alignLatestOwnMessageToTop("smooth");
    });
  },
);

watch(
  latestOwnMessageId,
  (nextValue, prevValue) => {
    if (!prevValue || !nextValue) return;
    if (!isDraftUserMessageId(prevValue) || isDraftUserMessageId(nextValue)) return;
    void nextTick(async () => {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      refreshObservedVirtualItemElements();
      alignLatestOwnMessageToTop("smooth");
    });
  },
);

watch(
  () => props.messageBlocks.length,
  () => {
    refreshObservedVirtualItemElements();
  },
);

watch(
  () => props.loadingOlderHistory,
  async (loading, wasLoading) => {
    if (loading) return;
    if (!wasLoading) return;
    const scrollEl = scrollContainer.value;
    if (!scrollEl) return;
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    refreshObservedVirtualItemElements();
    const scrollRestore = pendingOlderHistoryScrollRestore.value;
    if (scrollRestore) {
      const deltaHeight = scrollEl.scrollHeight - scrollRestore.scrollHeight;
      scrollEl.scrollTop = Math.max(0, scrollRestore.scrollTop + deltaHeight);
    }
    const anchor = pendingOlderHistoryAnchor.value;
    if (anchor) {
      const anchorMessageElement = findRenderedMessageElement(anchor.messageId);
      const anchorElement = resolveMessageAnchorElement(anchorMessageElement);
      if (anchorElement && anchorElement.isConnected) {
        const containerRect = scrollEl.getBoundingClientRect();
        const rect = anchorElement.getBoundingClientRect();
        if (anchor.edge === "bottom") {
          const currentOffset = containerRect.bottom - rect.bottom;
          scrollEl.scrollTop += anchor.offset - currentOffset;
        } else {
          const currentOffset = rect.top - containerRect.top;
          scrollEl.scrollTop += currentOffset - anchor.offset;
        }
      }
    }
    lastConversationScrollTop = scrollEl.scrollTop;
    olderHistoryRequestPending.value = false;
    olderHistoryTriggerReady.value = true;
    pendingOlderHistoryAnchor.value = null;
    pendingOlderHistoryScrollRestore.value = null;
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
  toolReviewButtonCount,
  toolReviewButtonLabel,
  toolReviewButtonEnabled,
  toolReviewDetailMap,
  toolReviewDetailLoadingCallId,
  toolReviewReviewingCallId,
  toolReviewBatchReviewingKey,
  toolReviewSubmittingBatchKey,
  toolReviewErrorText,
  toolReviewReportErrorText,
  toolReviewReports,
  toolReviewCurrentReportId,
  toggleToolReviewPanel,
  refreshToolReviewBatches,
  refreshToolReviewReports,
  setToolReviewCurrentBatchKey,
  loadToolReviewItemDetail,
  runToolReviewForCall,
  runToolReviewForBatch,
  submitToolReviewBatch,
  submitToolReviewCode,
  deleteToolReviewReport,
  listToolReviewCommitOptions,
} = useChatToolReview({
  activeConversationId: toRef(props, "activeConversationId"),
  selectedChatModelId: toRef(props, "selectedChatModelId"),
  messageBlocks: computed(() => props.messageBlocks),
  refreshTick: toRef(props, "toolReviewRefreshTick"),
  t,
  onRefreshMessage: (payload) => emit("refreshToolReviewMessage", payload),
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

function getEphemeralBlockRenderId(block: ChatMessageBlock): string {
  const cached = ephemeralBlockRenderIdMap.get(block);
  if (cached) return cached;
  ephemeralBlockRenderIdSeq += 1;
  const nextId = `block-ephemeral-${ephemeralBlockRenderIdSeq}`;
  ephemeralBlockRenderIdMap.set(block, nextId);
  return nextId;
}

function blockRenderId(block: ChatMessageBlock): string {
  const rawId = String(block.id || "").trim();
  if (rawId) return rawId;
  const sourceMessageId = String(block.sourceMessageId || "").trim();
  if (sourceMessageId) {
    return block.isExtraTextBlock ? `${sourceMessageId}::extra` : sourceMessageId;
  }
  const createdAt = String(block.createdAt || "").trim();
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  const role = String(block.role || "").trim();
  const textPreview = String(block.text || "").trim().slice(0, 64);
  if (createdAt || speakerAgentId || role || textPreview) {
    return [
      "block-stable",
      role || "no-role",
      speakerAgentId || "no-speaker",
      createdAt || "no-time",
      block.isExtraTextBlock ? "extra" : "base",
      textPreview || "no-text",
    ].join(":");
  }
  return getEphemeralBlockRenderId(block);
}

function blockGroupRenderId(block: ChatMessageBlock) {
  const createdAt = String(block.createdAt || "").trim();
  const textPreview = String(block.text || "").trim().slice(0, 48);
  const renderId = blockRenderId(block);
  if (createdAt || textPreview) {
    return `${renderId}:${createdAt || "no-time"}:${textPreview || "no-text"}`;
  }
  return `group-${renderId}`;
}

function messageMemoKey(block: ChatMessageBlock, renderId: string, blockIndex: number) {
  const selected = selectedMessageRenderIdSet.value.has(renderId);
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
    props.conversationBusy,
    messageSelectionModeEnabled.value,
    selected,
    canRegenerate,
    canConfirm,
    requiresInteractionState ? props.chatting : false,
    requiresInteractionState ? props.conversationBusy : false,
    requiresInteractionState ? props.frozen : false,
  ];
}

function estimateChatRenderItemHeight(item: ChatRenderItem): number {
  if (item.kind === "compaction") return 44;
  if (item.kind === "message") {
    return estimateMessageBlockHeight(item.block) + 8;
  }
  return item.items.reduce((total, groupItem) => total + estimateMessageBlockHeight(groupItem.block) + 8, 0) + 8;
}

function blockSizeDependencies(block: ChatMessageBlock): unknown[] {
  return [
    String(block.id || ""),
    String(block.sourceMessageId || ""),
    String(block.text || ""),
    String(block.reasoningInline || ""),
    String(block.reasoningStandard || ""),
    block.images.length,
    block.audios.length,
    block.attachmentFiles.length,
    block.toolCalls.length,
    Array.isArray(block.memeSegments) ? block.memeSegments.length : 0,
    block.planCard?.action || "",
    String(block.taskTrigger ? JSON.stringify(block.taskTrigger) : ""),
  ];
}

function virtualItemSizeDependencies(item: ChatRenderItem): unknown[] {
  if (item.kind === "compaction") {
    return [item.id, item.kind];
  }
  if (item.kind === "message") {
    return [item.id, ...blockSizeDependencies(item.block)];
  }
  return [
    item.id,
    ...item.items.flatMap((groupItem) => [groupItem.renderId, ...blockSizeDependencies(groupItem.block)]),
  ];
}

function estimateMessageBlockHeight(block: ChatMessageBlock): number {
  let estimate = isOwnMessage(block) ? 78 : 108;
  const text = String(block.text || "");
  const inlineReasoning = String(block.reasoningInline || "");
  const standardReasoning = String(block.reasoningStandard || "");
  const combinedTextLength = text.length + inlineReasoning.length + standardReasoning.length;
  estimate += Math.min(920, Math.ceil(combinedTextLength / 28) * 9);

  const codeFenceCount = countFenceMatches(text, /```[\w-]*\s*[\r\n]/g);
  const mermaidFenceCount = countFenceMatches(text, /```(?:\s*)mermaid\b/gi);
  estimate += codeFenceCount * 180;
  estimate += mermaidFenceCount * 120;

  if (block.planCard) estimate += 84;
  if (block.taskTrigger) estimate += 120;
  if (standardReasoning.trim()) estimate += Math.min(240, Math.ceil(standardReasoning.length / 36) * 12);
  if (inlineReasoning.trim()) estimate += Math.min(180, Math.ceil(inlineReasoning.length / 36) * 10);
  if (block.toolCalls.length > 0) estimate += block.toolCalls.length * 56 + 36;
  if (Array.isArray(block.memeSegments) && block.memeSegments.length > 0) {
    estimate += block.memeSegments.length * 42;
  }
  estimate += block.images.length * 120;
  estimate += block.audios.length * 42;
  estimate += block.attachmentFiles.length * 34;
  return Math.max(64, estimate);
}

function countFenceMatches(text: string, pattern: RegExp): number {
  if (!text) return 0;
  return Array.from(text.matchAll(pattern)).length;
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

function handleConversationListSelect(payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }) {
  const normalizedConversationId = String(payload?.conversationId || "").trim();
  if (!normalizedConversationId) return;
  const isCurrent = normalizedConversationId === String(props.activeConversationId || "").trim();
  if (isCurrent) return;
  const target = (props.conversationItems || props.unarchivedConversationItems).find(
    (item) => String(item.conversationId || "").trim() === normalizedConversationId,
  );
  emit("switchConversation", {
    conversationId: normalizedConversationId,
    kind: payload?.kind || target?.kind,
    remoteContactId: String(payload?.remoteContactId || target?.remoteContactId || "").trim() || undefined,
  });
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

function openConversationSummary(block: ChatMessageBlock, event?: MouseEvent) {
  event?.stopPropagation();
  const text = String(block?.text || "").trim();
  if (!text) return;
  conversationSummaryCard.value = {
    visible: true,
    text,
  };
}

function closeConversationSummaryCard() {
  conversationSummaryCard.value = {
    visible: false,
    text: "",
  };
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

async function copyToolReviewReport(reportText: string) {
  const text = String(reportText || "").trim();
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // 剪贴板失败不阻断报告查看。
  }
}

function emitSelectionAction(kind: "branch" | "share" | "forward", targetConversationId = "") {
  const payload = selectionPayload();
  if (payload.count === 0) return;
  if (kind === "branch") {
    emit("selectionActionBranch", payload);
    return;
  }
  if (kind === "forward") {
    const normalizedTargetConversationId = String(targetConversationId || "").trim();
    if (!normalizedTargetConversationId) return;
    emit("selectionActionForward", {
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
      if (canOpenInFileReader(href)) {
        await invokeTauri("open_file_reader_window_command", { path: href });
      } else {
        await invokeTauri("open_local_file_directory", { path: href });
      }
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
  if (pendingMeasureFrame) {
    cancelAnimationFrame(pendingMeasureFrame);
    pendingMeasureFrame = 0;
  }
  if (pendingPinToBottomFrame) {
    cancelAnimationFrame(pendingPinToBottomFrame);
    pendingPinToBottomFrame = 0;
  }
  observedVirtualItemElements.clear();
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

.ecall-chat-virtual-list {
  width: 100%;
  min-width: 0;
}

.ecall-chat-virtual-item {
  display: block;
  width: 100%;
  min-width: 0;
}

.ecall-chat-virtual-spacer {
  width: 100%;
  flex: 0 0 auto;
}

.ecall-turn-stack {
  display: flow-root;
  width: 100%;
  min-height: 0;
}

.ecall-elastic-item-shell {
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
