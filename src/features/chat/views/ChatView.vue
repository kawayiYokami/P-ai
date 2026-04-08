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
        @scroll="onScroll"
      >
        <div v-if="normalizedConversationTodos.length" class="sticky top-0 z-20 flex justify-center pt-1">
          <div
            class="ecall-floating-todo pointer-events-auto"
            :aria-label="t('config.task.fields.todo')"
            tabindex="0"
            @click.stop
            @mousedown.stop
          >
            <div class="ecall-floating-todo-summary text-[12px] text-base-content">
              <ListTodo class="h-4 w-4 shrink-0 text-base-content/65" />
              <span
                class="ecall-floating-todo-text truncate"
                :data-text="activeConversationTodo"
              >{{ activeConversationTodo }}</span>
              <span
                v-if="normalizedConversationTodos.length > 1"
                class="ecall-floating-todo-count"
              >+{{ normalizedConversationTodos.length - 1 }}</span>
            </div>
            <div
              v-if="normalizedConversationTodos.length > 1"
              class="ecall-floating-todo-panel"
            >
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
              v-memo="[item.block, chatting, frozen, markdownIsDark, playingAudioId, userAlias, userAvatarUrl, personaNameMap, personaAvatarUrlMap]"
              :block="item.block"
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
              @recall-turn="$emit('recallTurn', $event)"
              @regenerate-turn="$emit('regenerateTurn', $event)"
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
                  v-memo="[groupItem.block, chatting, frozen, markdownIsDark, activeTurnUserId, playingAudioId, userAlias, userAvatarUrl, personaNameMap, personaAvatarUrlMap]"
                  :block="groupItem.block"
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
                  @recall-turn="$emit('recallTurn', $event)"
                  @regenerate-turn="$emit('regenerateTurn', $event)"
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
            :workspace-locked="workspaceLocked"
            :current-workspace-name="currentWorkspaceName"
            :persona-presence-chips="personaPresenceChips"
            :supervision-active="supervisionActive"
            :supervision-label="t('chat.supervision.button')"
            :supervision-active-label="t('chat.supervision.buttonActive')"
            :supervision-title="supervisionButtonTitle"
            @lock-workspace="$emit('lockWorkspace')"
            @unlock-workspace="$emit('unlockWorkspace')"
            @open-supervision-task="$emit('openSupervisionTask')"
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
          :chat-input="chatInput"
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
          @remove-clipboard-image="$emit('removeClipboardImage', $event)"
          @remove-queued-attachment-notice="$emit('removeQueuedAttachmentNotice', $event)"
          @start-recording="$emit('startRecording')"
          @stop-recording="$emit('stopRecording')"
          @pick-attachments="$emit('pickAttachments')"
          @send-chat="$emit('sendChat')"
          @stop-chat="$emit('stopChat')"
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
        @close="$emit('closeSupervisionTask')"
        @save="$emit('saveSupervisionTask', $event)"
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
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMessageBlock, ChatPersonaPresenceChip, ChatTodoItem } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatConversationSidebar from "../components/ChatConversationSidebar.vue";
import ChatWorkspaceToolbar from "../components/ChatWorkspaceToolbar.vue";
import ChatImagePreviewDialog from "../components/dialogs/ChatImagePreviewDialog.vue";
import ChatSupervisionTaskDialog from "../components/dialogs/ChatSupervisionTaskDialog.vue";
import { useChatImagePreview } from "../composables/use-chat-image-preview";
import { useChatMessageActions } from "../composables/use-chat-message-actions";
import { useChatScrollLayout } from "../composables/use-chat-scroll-layout";
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
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string }>;
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  queuedAttachmentNotices: Array<{ id: string; fileName: string; relativePath: string; mime: string }>;
  chatInput: string;
  chatInputPlaceholder: string;
  canRecord: boolean;
  recording: boolean;
  recordingMs: number;
  transcribing: boolean;
  recordHotkey: string;
  selectedChatModelId: string;
  chatModelOptions: ApiConfigItem[];
  chatUsagePercent: number;
  forceArchiveTip: string;
  mediaDragActive: boolean;
  chatting: boolean;
  frozen: boolean;
  messageBlocks: ChatMessageBlock[];
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  currentWorkspaceName: string;
  workspaceLocked: boolean;
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
  currentTheme: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  createConversationDepartmentOptions: Array<{ id: string; name: string; ownerName: string }>;
  defaultCreateConversationDepartmentId: string;
}>();

const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));
function isOrganizeContextToolCall(call: { name: string; argsText: string }): boolean {
  const name = String(call.name || "").trim().toLowerCase();
  const argsText = String(call.argsText || "").trim().toLowerCase();
  if (name === "organize_context" || name === "archive") return true;
  return name === "command" && argsText.includes("organize_context");
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

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "sideConversationListVisibleChange", value: boolean): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "update:selectedChatModelId", value: string): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "forceArchive"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "lockWorkspace"): void;
  (e: "unlockWorkspace"): void;
  (e: "openSupervisionTask"): void;
  (e: "closeSupervisionTask"): void;
  (e: "saveSupervisionTask", payload: { durationHours: number; goal: string; why: string; todo: string }): void;
  (e: "switchConversation", conversationId: string): void;
  (e: "renameConversation", payload: { conversationId: string; title: string }): void;
  (e: "createConversation", input?: { title?: string; departmentId?: string }): void;
  (e: "openConversationSummary", conversationId: string): void;
  (e: "reachedBottom"): void;
}>();
const { t } = useI18n();

const linkOpenErrorText = ref("");
const composerPanelRef = ref<{ focusInput: (options?: FocusOptions) => void } | null>(null);

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
  return rawId ? `${rawId}-${blockIndex}` : `block-${blockIndex}`;
}

function isCompactionBlock(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const meta = (block.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
  const kind = String(messageMeta.kind || "").trim();
  return kind === "context_compaction" || kind === "summary_context_seed";
}

function canRegenerateBlock(block: ChatMessageBlock, blockIndex: number): boolean {
  if (block.role !== "assistant") return false;
  return blockIndex === props.messageBlocks.length - 1;
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

.ecall-turn-group {
  display: block;
  width: 100%;
}

.ecall-turn-stack {
  display: flow-root;
  width: 100%;
  min-height: 0;
}

.ecall-floating-todo {
  position: relative;
  isolation: isolate;
  overflow: visible;
}

.ecall-floating-todo-summary {
  display: inline-flex;
  max-width: min(88vw, 30rem);
  align-items: center;
  gap: 0.625rem;
  overflow: hidden;
  border: 1px solid hsl(var(--bc) / 0.15);
  border-radius: 999px;
  background: #fff;
  padding: 0.55rem 0.9rem;
  box-shadow:
    0 10px 30px rgb(0 0 0 / 0.16),
    inset 0 1px 0 rgb(255 255 255 / 0.18);
}

.ecall-floating-todo-text {
  position: relative;
  display: inline-flex;
  align-items: center;
  flex: 1 1 auto;
  min-width: 0;
  line-height: 1;
  color: currentColor;
}

.ecall-floating-todo-count {
  flex-shrink: 0;
  border-radius: 999px;
  background: hsl(var(--b2) / 0.9);
  padding: 0.15rem 0.45rem;
  font-size: 11px;
  line-height: 1;
  color: hsl(var(--bc) / 0.65);
}

.ecall-floating-todo-text::after {
  content: attr(data-text);
  position: absolute;
  inset: 0;
  pointer-events: none;
  color: transparent;
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    transparent 38%,
    rgb(255 255 255 / 0.92) 50%,
    transparent 62%,
    transparent 100%
  );
  background-size: 240px 100%;
  background-position: -240px 0;
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: ecall-floating-todo-shimmer 3.2s linear infinite;
}

.ecall-floating-todo-panel {
  position: absolute;
  top: calc(100% + 0.45rem);
  left: 50%;
  width: max-content;
  max-width: min(88vw, 30rem);
  max-height: min(60vh, 24rem);
  overflow-y: auto;
  border: 1px solid hsl(var(--bc) / 0.15);
  border-radius: 1rem;
  background: #fff;
  padding: 0.6rem;
  box-shadow:
    0 20px 50px rgb(0 0 0 / 0.2),
    inset 0 1px 0 rgb(255 255 255 / 0.2);
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, -0.35rem) scale(0.98);
  transition:
    opacity 140ms ease,
    transform 140ms ease;
}

.ecall-floating-todo:hover .ecall-floating-todo-panel,
.ecall-floating-todo:focus-within .ecall-floating-todo-panel {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, 0) scale(1);
}

.ecall-floating-todo:focus-visible {
  outline: none;
}

.ecall-floating-todo:focus-visible .ecall-floating-todo-summary {
  box-shadow:
    0 0 0 2px hsl(var(--p) / 0.25),
    0 10px 30px rgb(0 0 0 / 0.16),
    inset 0 1px 0 rgb(255 255 255 / 0.18);
}

@keyframes ecall-floating-todo-shimmer {
  from {
    background-position: -240px 0;
  }
  to {
    background-position: 240px 0;
  }
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
