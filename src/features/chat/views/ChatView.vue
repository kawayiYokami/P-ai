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
        <template v-for="item in chatRenderItems" :key="item.id">
          <div
            v-if="item.kind === 'compaction'"
            class="mt-4 flex items-center gap-3 text-[11px] text-base-content/45"
          >
            <div class="h-px flex-1 bg-base-300/80"></div>
            <span class="shrink-0 tracking-[0.2em]">上文已压缩</span>
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
            @lock-workspace="$emit('lockWorkspace')"
            @unlock-workspace="$emit('unlockWorkspace')"
          />
        </div>
      </div>

      <div
        v-show="showJumpToBottom"
        class="pointer-events-none absolute inset-x-0 z-30 flex justify-center"
        :style="jumpToBottomStyle"
      >
        <button
          class="btn btn-sm btn-circle btn-primary pointer-events-auto shadow-md"
          :title="t('chat.jumpToBottom')"
          @click="jumpToBottom"
        >
          <ArrowDown class="h-4 w-4" />
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

    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, toRef } from "vue";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme } from "../../shell/composables/use-app-theme";
import { ArrowDown } from "lucide-vue-next";
import "markstream-vue/index.css";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatConversationOverviewItem, ChatMessageBlock, ChatPersonaPresenceChip } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatConversationSidebar from "../components/ChatConversationSidebar.vue";
import ChatWorkspaceToolbar from "../components/ChatWorkspaceToolbar.vue";
import ChatImagePreviewDialog from "../components/dialogs/ChatImagePreviewDialog.vue";
import { useChatImagePreview } from "../composables/use-chat-image-preview";
import { useChatMessageActions } from "../composables/use-chat-message-actions";
import { useChatScrollLayout } from "../composables/use-chat-scroll-layout";

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
  mediaDragActive: boolean;
  chatting: boolean;
  frozen: boolean;
  messageBlocks: ChatMessageBlock[];
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  currentWorkspaceName: string;
  workspaceLocked: boolean;
  activeConversationId: string;
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
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "lockWorkspace"): void;
  (e: "unlockWorkspace"): void;
  (e: "switchConversation", conversationId: string): void;
  (e: "createConversation", input?: { title?: string; departmentId?: string }): void;
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

function isAbsoluteLocalPath(href: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(href) || href.startsWith("\\\\") || href.startsWith("/");
}

async function handleAssistantLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = anchor.getAttribute("href")?.trim() || "";
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
