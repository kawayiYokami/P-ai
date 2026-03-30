<template>
  <div
    ref="chatLayoutRoot"
    class="h-full min-h-0"
    :class="showSideConversationList ? 'flex flex-row overflow-hidden' : 'flex flex-col relative'"
  >
    <aside
      v-if="showSideConversationList"
      class="flex h-full w-88 shrink-0 flex-col border-r border-base-300 bg-base-200"
    >
      <div class="flex-1 min-h-0 space-y-2 overflow-y-auto py-2">
        <button
          v-for="item in props.unarchivedConversationItems"
          :key="item.conversationId"
          type="button"
          class="mx-2 block w-[calc(100%-1rem)] rounded-box bg-base-200 text-left transition-colors hover:bg-base-100"
          :class="[
            item.conversationId === props.activeConversationId ? 'bg-primary/10 hover:bg-primary/10' : '',
            item.runtimeState === 'organizing_context' ? 'cursor-not-allowed opacity-60' : '',
          ]"
          :disabled="item.runtimeState === 'organizing_context'"
          :title="item.runtimeState === 'organizing_context' ? t('chat.organizingContextDisabled') : (item.workspaceLabel || t('chat.defaultWorkspace'))"
          @click="handleConversationListSelect(item.conversationId)"
        >
          <div class="flex items-center gap-3 p-3">
            <div class="shrink-0">
              <div class="avatar">
                <div class="w-10 h-10 rounded-full bg-error text-error-content">
                  <img
                    v-if="sideListLastSpeakerAvatarUrl(item)"
                    :src="sideListLastSpeakerAvatarUrl(item)"
                    :alt="sideListLastSpeakerLabel(item)"
                    class="w-10 h-10 rounded-full object-cover"
                  />
                  <span v-else class="text-sm font-bold">{{ sideListLastSpeakerInitial(item) }}</span>
                </div>
              </div>
            </div>

            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <div class="truncate text-sm font-medium">
                  {{ item.title || t("chat.untitledConversation") }}
                </div>
                <span v-if="item.isMainConversation" class="badge badge-primary badge-xs shrink-0">
                  {{ t("chat.mainConversation") }}
                </span>
                <span v-if="item.conversationId === props.activeConversationId" class="badge badge-outline badge-xs shrink-0">
                  {{ t("chat.currentConversation") }}
                </span>
              </div>

              <div class="mt-1 flex items-center gap-2 text-xs">
                <span class="font-medium">{{ item.workspaceLabel || t("chat.defaultWorkspace") }}</span>
                <span class="text-base-content/70">{{ formatConversationTime(item.updatedAt) }}</span>
                <span class="font-medium">{{ t("chat.messageCount", { count: item.messageCount }) }}</span>
              </div>
            </div>

            <span v-if="item.runtimeState" class="badge badge-ghost badge-xs shrink-0">
              {{ runtimeStateText(item.runtimeState) }}
            </span>
          </div>

          <div class="space-y-1 px-3 pb-3">
            <div
              v-for="preview in normalizedPreviewMessages(item).slice(0, 2)"
              :key="preview.messageId"
              class="flex items-start gap-2 text-xs"
            >
              <span class="shrink-0 font-medium">{{ speakerLabel(preview) }}:</span>
              <span class="truncate opacity-80">{{ previewText(preview) }}</span>
            </div>
            <div v-if="normalizedPreviewMessages(item).length === 0" class="px-2 text-xs opacity-60">
              {{ t("chat.conversationNoPreview") }}
            </div>
          </div>
        </button>
      </div>
    </aside>

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
        <!-- 历史对话 -->
        <template v-for="(block, blockIndex) in messageBlocks" :key="block.id">
          <div
            v-if="isCompactionBlock(block)"
            class="mt-4 flex items-center gap-3 text-[11px] text-base-content/45"
          >
            <div class="h-px flex-1 bg-base-300/80"></div>
            <span class="shrink-0 tracking-[0.2em]">上文已压缩</span>
            <div class="h-px flex-1 bg-base-300/80"></div>
          </div>
          <ChatMessageItem
            v-else
            v-memo="[block, chatting, frozen, markdownIsDark, latestOwnMessageBlockId, latestMessageBlockId, latestMessageMinHeight, playingAudioId, userAlias, userAvatarUrl, personaNameMap, personaAvatarUrlMap]"
            :block="block"
            :chatting="chatting"
            :frozen="frozen"
            :user-alias="userAlias"
            :user-avatar-url="userAvatarUrl"
            :persona-name-map="personaNameMap"
            :persona-avatar-url-map="personaAvatarUrlMap"
            :stream-tool-calls="streamToolCalls"
            :markdown-is-dark="markdownIsDark"
            :playing-audio-id="playingAudioId"
            :latest-own-message="isLatestOwnMessage(block)"
            :latest-message="isLatestMessage(block)"
            :latest-message-min-height="latestMessageMinHeight"
            :can-regenerate="canRegenerateBlock(block, blockIndex)"
            @recall-turn="$emit('recallTurn', $event)"
            @regenerate-turn="$emit('regenerateTurn', $event)"
            @copy-message="copyMessage"
            @open-image-preview="openImagePreview"
            @toggle-audio-playback="toggleAudioPlayback($event.id, $event.audio)"
            @assistant-link-click="handleAssistantLinkClick"
          />
        </template>

        <div ref="toolbarContainer" class="pt-1 pb-2">
          <div class="rounded-box border border-base-300 bg-base-100/70 px-2 py-1.5 flex items-center gap-2 text-[11px]">
            <button
              class="btn btn-sm bg-base-100"
              :title="workspaceLocked ? '已锁定，点击还原到默认工作空间' : '未锁定'"
              :disabled="chatting || frozen || !workspaceLocked"
              @click="$emit('unlockWorkspace')"
            >
              <Lock v-if="workspaceLocked" class="h-3.5 w-3.5" />
              <LockOpen v-else class="h-3.5 w-3.5" />
            </button>
            <button
              class="btn btn-sm bg-base-100"
              :disabled="chatting || frozen"
              @click="$emit('lockWorkspace')"
            >
              {{ currentWorkspaceName }}{{ workspaceLocked ? " (临时)" : "" }}
            </button>
            <div class="ml-auto flex items-center gap-1.5 overflow-x-auto scrollbar-thin">
              <button
                v-for="persona in props.personaPresenceChips"
                :key="persona.id"
                type="button"
                class="btn btn-ghost btn-sm btn-circle p-0 shrink-0 border relative"
                :class="persona.isFrontSpeaking ? 'border-primary/60 bg-primary/10' : 'border-base-300/70 bg-base-100/70'"
                :title="`部门：${persona.departmentName}\n人格：${persona.name}`"
                @click.prevent
              >
                <div class="avatar">
                  <div
                    class="w-7 rounded-full"
                    :class="persona.isFrontSpeaking ? 'ring-2 ring-primary ring-offset-2 ring-offset-base-100' : ''"
                  >
                    <img
                      v-if="persona.avatarUrl"
                      :src="persona.avatarUrl"
                      :alt="persona.name"
                      class="w-7 h-7 rounded-full object-cover"
                    />
                    <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-[10px]">
                      {{ avatarInitial(persona.name) }}
                    </div>
                  </div>
                </div>
                <span
                  v-if="persona.hasBackgroundTask"
                  class="absolute right-0.5 top-0.5 inline-block h-2.5 w-2.5 rounded-full bg-error ring-2 ring-base-100"
                ></span>
              </button>
            </div>
          </div>
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

      <div ref="composerContainer" class="shrink-0 border-t border-base-300 bg-base-100 p-2">
        <!-- 队列预览 -->
        <ChatQueuePreview
          :queue-events="queueEvents"
          :session-state="sessionState"
          @recall-to-input="handleRecallToInput"
          @remove-from-queue="removeFromQueue"
        />

        <div
          v-if="linkOpenErrorText"
          class="alert alert-warning mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-24 overflow-auto"
        >
          <span>{{ linkOpenErrorText }}</span>
        </div>
        <div
          v-if="chatErrorText"
          class="alert alert-error mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-28 overflow-auto"
        >
          <span>{{ chatErrorText }}</span>
        </div>
        <div v-if="clipboardImages.length > 0 || queuedAttachmentNotices.length > 0" class="mb-2 flex flex-wrap gap-1">
          <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-outline gap-1 py-3">
            <ImageIcon v-if="isImageMime(img.mime)" class="h-3.5 w-3.5" />
            <FileText v-else-if="isPdfMime(img.mime)" class="h-3.5 w-3.5" />
            <ImageIcon v-else class="h-3.5 w-3.5" />
            <span class="text-[11px]">{{ isPdfMime(img.mime) ? `PDF ${idx + 1}` : t("chat.image", { index: idx + 1 }) }}</span>
            <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="$emit('removeClipboardImage', idx)">
              <X class="h-3 w-3" />
            </button>
          </div>
          <div
            v-for="(file, idx) in queuedAttachmentNotices"
            :key="file.id"
            class="badge badge-outline gap-1 py-3"
          >
            <FileText class="h-3.5 w-3.5" />
            <span class="text-[11px]">{{ file.fileName }}</span>
            <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="$emit('removeQueuedAttachmentNotice', idx)">
              <X class="h-3 w-3" />
            </button>
          </div>
        </div>
        <div v-if="transcribing" class="mb-1 text-[11px] opacity-80 flex items-center gap-1">
          <span class="loading loading-spinner loading-sm"></span>
          <span>语音转写中...</span>
        </div>
        <div class="flex flex-col gap-2">
          <textarea
            ref="chatInputRef"
            v-model="localChatInput"
            class="w-full textarea textarea-sm resize-none overflow-y-auto chat-input-no-focus scrollbar-gutter-stable min-h-12.5"
            rows="1"
            :disabled="frozen"
            :placeholder="chatInputPlaceholder"
            @input="scheduleResizeChatInput"
            @keydown="handleChatInputKeydown"
          ></textarea>
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <button
                class="btn btn-sm btn-circle bg-base-100 shrink-0"
                :class="{ 'btn-disabled': frozen || !canCreateConversation }"
                :disabled="frozen || !canCreateConversation"
                :title="canCreateConversation ? t('chat.newConversation') : t('chat.maxConversations')"
                @click="handleCreateConversation"
              >
                <Plus class="h-3.5 w-3.5" />
              </button>
            </div>
            <div v-if="!showSideConversationList" ref="conversationListPopoverRef" class="relative flex items-center gap-2">
              <button
                class="btn btn-sm bg-base-100 shrink-0 gap-1.5 pl-3 pr-2"
                :title="t('chat.conversationList')"
                @click="toggleConversationList"
              >
                <List class="h-3.5 w-3.5" />
                <span class="text-xs">{{ t("chat.conversationListShort") }}</span>
                <span class="badge badge-ghost badge-xs">{{ props.unarchivedConversationItems.length }}</span>
              </button>
              <div v-if="conversationListOpen" class="absolute bottom-full left-0 z-40 mb-2">
                <ChatConversationListCard
                  :items="props.unarchivedConversationItems"
                  :active-conversation-id="props.activeConversationId"
                  :user-alias="props.userAlias"
                  :persona-name-map="props.personaNameMap"
                  :persona-avatar-url-map="props.personaAvatarUrlMap"
                  :user-avatar-url="props.userAvatarUrl"
                  @select-conversation="handleConversationListSelect"
                />
              </div>
            </div>
            <div class="ml-auto flex items-center gap-2">
              <div v-if="!showSideConversationList" class="h-5 w-px shrink-0 bg-base-300"></div>
              <button
                class="btn btn-sm btn-circle bg-base-100 shrink-0"
                :disabled="chatting || frozen"
                :title="t('chat.attach')"
                @click="$emit('pickAttachments')"
              >
                <Paperclip class="h-3.5 w-3.5" />
              </button>
              <button
                class="btn btn-sm btn-circle shrink-0"
                :class="recording ? 'btn-error' : 'bg-base-100'"
                :disabled="!canRecord || chatting || frozen"
                :title="recording ? t('chat.recording', { seconds: Math.max(1, Math.round(recordingMs / 1000)) }) : t('chat.holdRecord', { hotkey: recordHotkey })"
                @mousedown.prevent="$emit('startRecording')"
                @mouseup.prevent="$emit('stopRecording')"
                @mouseleave.prevent="recording && $emit('stopRecording')"
                @touchstart.prevent="$emit('startRecording')"
                @touchend.prevent="$emit('stopRecording')"
              >
                <Mic class="h-3.5 w-3.5" />
              </button>
              <button
                class="btn btn-sm btn-circle btn-primary shrink-0"
                :disabled="frozen"
                :title="chatting ? `${t('chat.stop')} / ${t('chat.stopReplying')}` : t('chat.send')"
                @click="chatting ? $emit('stopChat') : handleSendChat()"
              >
                <Square v-if="chatting" class="h-3.5 w-3.5 fill-current" />
                <Send v-else class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <dialog class="modal" :class="{ 'modal-open': imagePreviewOpen }">
        <div class="modal-box w-11/12 max-w-6xl p-2 bg-base-100">
          <div class="mb-2 flex items-center justify-end gap-1">
            <button class="btn btn-xs" :disabled="imagePreviewZoom <= IMAGE_PREVIEW_MIN_ZOOM" @click="zoomOutPreview">
              <Minus class="h-3 w-3" />
            </button>
            <button class="btn btn-xs" :disabled="imagePreviewZoom >= IMAGE_PREVIEW_MAX_ZOOM" @click="zoomInPreview">
              <Plus class="h-3 w-3" />
            </button>
            <button class="btn btn-xs" :disabled="Math.abs(imagePreviewZoom - 1) < 0.001" @click="resetPreviewZoom">
              {{ Math.round(imagePreviewZoom * 100) }}%
            </button>
          </div>
          <div
            class="max-h-[80vh] overflow-hidden flex items-center justify-center"
            :class="imagePreviewZoom > 1 ? (previewDragging ? 'cursor-grabbing' : 'cursor-grab') : ''"
            @wheel.prevent="onPreviewWheel"
            @pointermove="onPreviewPointerMove"
            @pointerup="onPreviewPointerUp"
            @pointercancel="onPreviewPointerUp"
            @pointerleave="onPreviewPointerUp"
          >
            <img
              v-if="imagePreviewDataUrl"
              :src="imagePreviewDataUrl"
              class="max-h-[80vh] max-w-full object-contain rounded select-none"
              :style="{ transform: `translate(${previewOffsetX}px, ${previewOffsetY}px) scale(${imagePreviewZoom})`, transformOrigin: 'center center' }"
              @pointerdown="onPreviewPointerDown"
            />
          </div>
        </div>
        <form method="dialog" class="modal-backdrop">
          <button @click.prevent="closeImagePreview">close</button>
        </form>
      </dialog>

      <dialog class="modal" :class="{ 'modal-open': createConversationDialogOpen }">
        <div class="modal-box max-w-md">
          <h3 class="text-base font-semibold">{{ t("chat.newConversation") }}</h3>
          <div class="mt-3 flex flex-col gap-3">
            <input
              ref="createConversationInputRef"
              v-model="createConversationTitle"
              type="text"
              class="input input-bordered w-full"
              :placeholder="t('chat.newConversationTopicPlaceholder')"
              @keydown="handleCreateConversationDialogKeydown"
            />
            <div v-if="recentConversationTopics.length > 0" class="flex flex-col gap-2">
              <div class="text-xs font-medium opacity-70">{{ t("chat.recentConversationTopics") }}</div>
              <div class="flex flex-wrap gap-2">
                <button
                  v-for="topic in recentConversationTopics"
                  :key="topic"
                  type="button"
                  class="btn btn-sm btn-ghost"
                  @click="applyRecentConversationTopic(topic)"
                >
                  {{ topic }}
                </button>
              </div>
            </div>
          </div>
          <div class="modal-action">
            <button class="btn btn-sm" @click="closeCreateConversationDialog">{{ t("common.cancel") }}</button>
            <button class="btn btn-sm btn-primary" @click="confirmCreateConversation">{{ t("common.confirm") }}</button>
          </div>
        </div>
        <form method="dialog" class="modal-backdrop">
          <button @click.prevent="closeCreateConversationDialog">close</button>
        </form>
      </dialog>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme } from "../../shell/composables/use-app-theme";
import { ArrowDown, FileText, Image as ImageIcon, List, Lock, LockOpen, Mic, Minus, Paperclip, Plus, Send, Square, X } from "lucide-vue-next";
import "markstream-vue/index.css";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatConversationOverviewItem, ChatMessageBlock, ChatPersonaPresenceChip, ConversationPreviewMessage } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatConversationListCard from "../components/ChatConversationListCard.vue";
import ChatQueuePreview from "../components/ChatQueuePreview.vue";
import { useChatQueue } from "../composables/use-chat-queue";

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
}>();

const canCreateConversation = computed(
  () => props.unarchivedConversationItems.length === 0 || !!props.unarchivedConversationItems[0]?.canCreateNew,
);
const conversationPreviewCache = computed(() => new Map(
  props.unarchivedConversationItems.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));

const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));
const latestOwnMessageBlockId = computed(() => {
  for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
    const block = props.messageBlocks[idx];
    if (isOwnMessage(block)) return String(block.id || "").trim();
  }
  return "";
});
const latestMessageBlockId = computed(() => String(props.messageBlocks[props.messageBlocks.length - 1]?.id || "").trim());

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
  (e: "createConversation", title?: string): void;
  (e: "reachedBottom"): void;
}>();
const { t } = useI18n();

// 队列管理
const { queueEvents, sessionState, removeFromQueue } = useChatQueue();

// 从队列退回到输入框
function handleRecallToInput(event: any) {
  if (event.source === "user") {
    localChatInput.value = event.messagePreview;
    removeFromQueue(event.id);
  }
}

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;
const RECENT_CONVERSATION_TOPICS_STORAGE_KEY = "easy_call.recent_conversation_topics.v1";
const RECENT_CONVERSATION_TOPICS_LIMIT = 7;
const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const recentConversationTopics = ref<string[]>([]);
const createConversationDialogOpen = ref(false);
const createConversationTitle = ref("");
let chatInputHistoryApplying = false;

function loadChatInputHistory() {
  try {
    const raw = window.localStorage.getItem(CHAT_INPUT_HISTORY_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    const normalized: string[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
      const text = String(item || "").trim();
      if (!text || seen.has(text)) continue;
      seen.add(text);
      normalized.push(text);
      if (normalized.length >= CHAT_INPUT_HISTORY_LIMIT) break;
    }
    chatInputHistory.value = normalized;
  } catch {
    chatInputHistory.value = [];
  }
}

function saveChatInputHistory() {
  try {
    window.localStorage.setItem(CHAT_INPUT_HISTORY_STORAGE_KEY, JSON.stringify(chatInputHistory.value));
  } catch {
    // ignore persistence failures
  }
}

function loadRecentConversationTopics() {
  try {
    const raw = window.localStorage.getItem(RECENT_CONVERSATION_TOPICS_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    const normalized: string[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
      const text = String(item || "").trim();
      if (!text || seen.has(text)) continue;
      seen.add(text);
      normalized.push(text);
      if (normalized.length >= RECENT_CONVERSATION_TOPICS_LIMIT) break;
    }
    recentConversationTopics.value = normalized;
  } catch {
    recentConversationTopics.value = [];
  }
}

function saveRecentConversationTopics() {
  try {
    window.localStorage.setItem(RECENT_CONVERSATION_TOPICS_STORAGE_KEY, JSON.stringify(recentConversationTopics.value));
  } catch {
    // ignore persistence failures
  }
}

function pushRecentConversationTopic(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  recentConversationTopics.value = [text, ...recentConversationTopics.value.filter((item) => item !== text)].slice(0, RECENT_CONVERSATION_TOPICS_LIMIT);
  saveRecentConversationTopics();
}

function pushChatInputHistory(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  chatInputHistory.value = [text, ...chatInputHistory.value.filter((item) => item !== text)].slice(0, CHAT_INPUT_HISTORY_LIMIT);
  saveChatInputHistory();
  chatInputHistoryCursor.value = -1;
  chatInputHistoryDraft.value = "";
}

function applyChatInputHistoryValue(value: string) {
  chatInputHistoryApplying = true;
  localChatInput.value = value;
  nextTick(() => {
    chatInputHistoryApplying = false;
    scheduleResizeChatInput();
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = value.length;
    el.setSelectionRange(cursor, cursor);
  });
}

function canNavigateHistory(el: HTMLTextAreaElement, direction: "up" | "down"): boolean {
  if (el.selectionStart !== el.selectionEnd) return false;
  if (direction === "up") return el.selectionStart === 0;
  return el.selectionStart === el.value.length;
}

function navigateChatInputHistory(direction: "up" | "down"): boolean {
  const list = chatInputHistory.value;
  if (list.length === 0) return false;
  if (direction === "up") {
    if (chatInputHistoryCursor.value === -1) {
      chatInputHistoryDraft.value = localChatInput.value;
      chatInputHistoryCursor.value = 0;
      applyChatInputHistoryValue(list[0]);
      return true;
    }
    if (chatInputHistoryCursor.value < list.length - 1) {
      chatInputHistoryCursor.value += 1;
      applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
      return true;
    }
    return false;
  }
  if (chatInputHistoryCursor.value === -1) return false;
  if (chatInputHistoryCursor.value === 0) {
    chatInputHistoryCursor.value = -1;
    const draft = chatInputHistoryDraft.value;
    chatInputHistoryDraft.value = "";
    applyChatInputHistoryValue(draft);
    return true;
  }
  chatInputHistoryCursor.value -= 1;
  applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
  return true;
}

function recordSentTextIfNeeded(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  setTimeout(() => {
    if (String(props.chatInput || "").trim()) return;
    pushChatInputHistory(text);
  }, 0);
}

function handleSendChat() {
  const plainText = String(localChatInput.value || "").trim();
  emit("sendChat");
  recordSentTextIfNeeded(plainText);
}

function handleChatInputKeydown(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
    if (props.frozen) return;
    event.preventDefault();
    handleSendChat();
    return;
  }
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
  const el = chatInputRef.value;
  if (!el) return;
  const direction = event.key === "ArrowUp" ? "up" : "down";
  if (!canNavigateHistory(el, direction)) return;
  if (navigateChatInputHistory(direction)) {
    event.preventDefault();
  }
}

const scrollContainer = ref<HTMLElement | null>(null);
const composerContainer = ref<HTMLElement | null>(null);
const toolbarContainer = ref<HTMLElement | null>(null);
const chatLayoutRoot = ref<HTMLElement | null>(null);
const conversationListPopoverRef = ref<HTMLElement | null>(null);
const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const createConversationInputRef = ref<HTMLInputElement | null>(null);
const playingAudioId = ref("");
const linkOpenErrorText = ref("");
const latestMessageMinHeight = ref(0);
const jumpToBottomOffset = ref(96);
const conversationListOpen = ref(false);
const showSideConversationList = ref(false);
let activeAudio: HTMLAudioElement | null = null;
let resizeInputRaf = 0;
let pendingAlignOnLatestContainerReady = false;
let composerResizeObserver: ResizeObserver | null = null;
let latestMessageLayoutObserver: ResizeObserver | null = null;
let chatLayoutResizeObserver: ResizeObserver | null = null;
const CHAT_SIDE_LIST_RATIO_THRESHOLD = 1.5;

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function isOwnMessage(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const id = String(block.speakerAgentId || "").trim();
  return !id || id === "user-persona";
}

function isCompactionBlock(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const text = String(block.text || "").trim();
  if (!text) return false;
  return text.startsWith("[上下文整理]") || text.startsWith("[上下文压缩]");
}

function canRegenerateBlock(block: ChatMessageBlock, blockIndex: number): boolean {
  if (block.role !== "assistant") return false;
  return blockIndex === props.messageBlocks.length - 1;
}

function isLatestOwnMessage(block: ChatMessageBlock): boolean {
  return isOwnMessage(block) && String(block.id || "").trim() === latestOwnMessageBlockId.value;
}

function isLatestMessage(block: ChatMessageBlock): boolean {
  return String(block.id || "").trim() === latestMessageBlockId.value;
}

function splitThinkText(raw: string): { visible: string; inline: string } {
  const input = raw || "";
  const openTag = "<think>";
  const closeTag = "</think>";
  const blocks: string[] = [];
  let visible = "";
  let cursor = 0;

  while (cursor < input.length) {
    const openIdx = input.indexOf(openTag, cursor);
    if (openIdx < 0) {
      visible += input.slice(cursor);
      break;
    }

    visible += input.slice(cursor, openIdx);
    const afterOpen = openIdx + openTag.length;
    const closeIdx = input.indexOf(closeTag, afterOpen);
    if (closeIdx < 0) {
      const tail = input.slice(afterOpen).trim();
      if (tail) blocks.push(tail);
      cursor = input.length;
      break;
    }

    const inner = input.slice(afterOpen, closeIdx).trim();
    if (inner) blocks.push(inner);
    cursor = closeIdx + closeTag.length;
  }

  return {
    visible: visible.trim(),
    inline: blocks.join("\n\n"),
  };
}

async function copyMessage(block: ChatMessageBlock) {
  const copyText = splitThinkText(block.text).visible || block.text || "";
  if (!copyText) return;
  try {
    await navigator.clipboard.writeText(copyText);
  } catch {
    // Ignore clipboard failures to avoid interrupting chat flow.
  }
}

function buildAudioDataUrl(audio: { mime: string; bytesBase64: string }): string {
  return `data:${audio.mime};base64,${audio.bytesBase64}`;
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function stopAudioPlayback() {
  if (activeAudio) {
    activeAudio.pause();
    activeAudio.currentTime = 0;
    activeAudio = null;
  }
  playingAudioId.value = "";
}

function toggleAudioPlayback(id: string, audio: { mime: string; bytesBase64: string }) {
  if (playingAudioId.value === id && activeAudio) {
    stopAudioPlayback();
    return;
  }
  stopAudioPlayback();
  const player = new Audio(buildAudioDataUrl(audio));
  activeAudio = player;
  playingAudioId.value = id;
  player.onended = () => {
    if (activeAudio === player) {
      activeAudio = null;
      playingAudioId.value = "";
    }
  };
  void player.play().catch(() => {
    if (activeAudio === player) {
      activeAudio = null;
      playingAudioId.value = "";
    }
  });
}

function scrollToBottom(behavior: ScrollBehavior = "smooth") {
  const el = scrollContainer.value;
  if (!el) return;
  el.scrollTo({ top: el.scrollHeight, behavior });
}

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.min(el.scrollHeight, maxHeight);
  el.style.height = `${nextHeight}px`;
  // Keep a stable scrollbar gutter to avoid width jumps while typing.
  el.style.overflowY = "auto";
}

function scheduleResizeChatInput() {
  if (resizeInputRaf) cancelAnimationFrame(resizeInputRaf);
  resizeInputRaf = requestAnimationFrame(() => {
    resizeChatInput();
    updateJumpToBottomOffset();
    resizeInputRaf = 0;
  });
}

const imagePreviewOpen = ref(false);
const imagePreviewDataUrl = ref("");
const imagePreviewZoom = ref(1);
const IMAGE_PREVIEW_MIN_ZOOM = 0.2;
const IMAGE_PREVIEW_MAX_ZOOM = 5;
const IMAGE_PREVIEW_ZOOM_STEP = 0.1;
const previewOffsetX = ref(0);
const previewOffsetY = ref(0);
const previewDragging = ref(false);
let previewPointerId: number | null = null;
let previewDragStartX = 0;
let previewDragStartY = 0;
let previewDragOriginOffsetX = 0;
let previewDragOriginOffsetY = 0;

function clampPreviewZoom(value: number): number {
  return Math.min(IMAGE_PREVIEW_MAX_ZOOM, Math.max(IMAGE_PREVIEW_MIN_ZOOM, value));
}

function zoomInPreview() {
  imagePreviewZoom.value = clampPreviewZoom(imagePreviewZoom.value + IMAGE_PREVIEW_ZOOM_STEP);
}

function zoomOutPreview() {
  imagePreviewZoom.value = clampPreviewZoom(imagePreviewZoom.value - IMAGE_PREVIEW_ZOOM_STEP);
  if (imagePreviewZoom.value <= 1) {
    previewOffsetX.value = 0;
    previewOffsetY.value = 0;
  }
}

function resetPreviewZoom() {
  imagePreviewZoom.value = 1;
  previewOffsetX.value = 0;
  previewOffsetY.value = 0;
}

function onPreviewWheel(event: WheelEvent) {
  if (event.deltaY < 0) {
    zoomInPreview();
  } else if (event.deltaY > 0) {
    zoomOutPreview();
  }
}

const showJumpToBottom = computed(() => !lastBottomState.value);
const jumpToBottomStyle = computed(() => ({
  bottom: `${jumpToBottomOffset.value}px`,
}));

function jumpToBottom() {
  scrollToBottom("smooth");
}

function openImagePreview(image: { mime: string; bytesBase64: string }) {
  const mime = String(image.mime || "").trim() || "image/webp";
  const bytes = String(image.bytesBase64 || "").trim();
  if (!bytes) return;
  imagePreviewDataUrl.value = `data:${mime};base64,${bytes}`;
  imagePreviewZoom.value = 1;
  previewOffsetX.value = 0;
  previewOffsetY.value = 0;
  previewDragging.value = false;
  previewPointerId = null;
  imagePreviewOpen.value = true;
}

function closeImagePreview() {
  imagePreviewOpen.value = false;
  imagePreviewDataUrl.value = "";
  imagePreviewZoom.value = 1;
  previewOffsetX.value = 0;
  previewOffsetY.value = 0;
  previewDragging.value = false;
  previewPointerId = null;
}

function onPreviewPointerDown(event: PointerEvent) {
  if (imagePreviewZoom.value <= 1) return;
  previewDragging.value = true;
  previewPointerId = event.pointerId;
  previewDragStartX = event.clientX;
  previewDragStartY = event.clientY;
  previewDragOriginOffsetX = previewOffsetX.value;
  previewDragOriginOffsetY = previewOffsetY.value;
  (event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId);
}

function onPreviewPointerMove(event: PointerEvent) {
  if (!previewDragging.value || previewPointerId !== event.pointerId) return;
  const deltaX = event.clientX - previewDragStartX;
  const deltaY = event.clientY - previewDragStartY;
  previewOffsetX.value = previewDragOriginOffsetX + deltaX;
  previewOffsetY.value = previewDragOriginOffsetY + deltaY;
}

function onPreviewPointerUp(event: PointerEvent) {
  if (previewPointerId !== null && previewPointerId !== event.pointerId) return;
  previewDragging.value = false;
  previewPointerId = null;
}

function closeConversationList() {
  conversationListOpen.value = false;
}

function syncConversationLayoutMode() {
  const el = chatLayoutRoot.value;
  if (!el) return;
  const width = el.clientWidth;
  const height = el.clientHeight;
  if (width <= 0 || height <= 0) return;
  showSideConversationList.value = width > height * CHAT_SIDE_LIST_RATIO_THRESHOLD;
  if (showSideConversationList.value) {
    closeConversationList();
  }
}

function handleCreateConversation() {
  closeConversationList();
  createConversationTitle.value = "";
  createConversationDialogOpen.value = true;
  nextTick(() => createConversationInputRef.value?.focus());
}

function closeCreateConversationDialog() {
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
}

function applyRecentConversationTopic(topic: string) {
  createConversationTitle.value = String(topic || "").trim();
  nextTick(() => createConversationInputRef.value?.focus());
}

function confirmCreateConversation() {
  const title = String(createConversationTitle.value || "").trim();
  if (title) {
    pushRecentConversationTopic(title);
  }
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
  emit("createConversation", title);
}

function handleCreateConversationDialogKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    confirmCreateConversation();
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    closeCreateConversationDialog();
  }
}

function normalizedPreviewMessages(item: ChatConversationOverviewItem): ConversationPreviewMessage[] {
  return conversationPreviewCache.value.get(String(item.conversationId || "").trim()) || [];
}

function runtimeStateText(runtimeState?: ChatConversationOverviewItem["runtimeState"]): string {
  if (runtimeState === "assistant_streaming") return t("chat.runtimeStreaming");
  if (runtimeState === "organizing_context") return t("chat.runtimeOrganizing");
  return t("chat.runtimeIdle");
}

function speakerLabel(preview: ConversationPreviewMessage): string {
  if (preview.role === "tool") return t("archives.roleTool");
  const speakerId = String(preview.speakerAgentId || "").trim();
  if (!speakerId || speakerId === "user-persona") {
    return props.userAlias || t("archives.roleUser");
  }
  return props.personaNameMap?.[speakerId] || speakerId;
}

function previewText(preview: ConversationPreviewMessage): string {
  const text = String(preview.textPreview || "").trim();
  if (text) return text;
  if (preview.hasPdf) return t("chat.previewPdf");
  if (preview.hasImage) return t("chat.previewImage");
  if (preview.hasAudio) return t("chat.previewAudio");
  if (preview.hasAttachment) return t("chat.previewAttachment");
  return t("chat.conversationNoPreview");
}

function formatConversationTime(value?: string): string {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function sideListLastSpeakerInitial(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "?";
  return speakerLabel(previews[previews.length - 1]).charAt(0).toUpperCase();
}

function sideListLastSpeakerLabel(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";
  return speakerLabel(previews[previews.length - 1]);
}

function sideListLastSpeakerAvatarUrl(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";
  const speakerId = String(previews[previews.length - 1].speakerAgentId || "").trim();
  if (!speakerId || speakerId === "user-persona") {
    return props.userAvatarUrl || "";
  }
  return props.personaAvatarUrlMap?.[speakerId] || "";
}

function toggleConversationList() {
  conversationListOpen.value = !conversationListOpen.value;
}

function handleConversationListSelect(conversationId: string) {
  const normalizedConversationId = String(conversationId || "").trim();
  closeConversationList();
  if (!normalizedConversationId) return;
  const isCurrent = normalizedConversationId === String(props.activeConversationId || "").trim();
  if (isCurrent) return;
  emit("switchConversation", normalizedConversationId);
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!conversationListOpen.value) return;
  const target = event.target as Node | null;
  const root = conversationListPopoverRef.value;
  if (root && target && !root.contains(target)) {
    closeConversationList();
  }
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && conversationListOpen.value) {
    closeConversationList();
  }
}

const lastBottomState = ref(false);

function updateJumpToBottomOffset() {
  const composerHeight = composerContainer.value?.offsetHeight ?? 0;
  jumpToBottomOffset.value = Math.max(16, composerHeight + 12);
}

function isNearBottom(el: HTMLElement): boolean {
  const threshold = 24;
  const distance = el.scrollHeight - (el.scrollTop + el.clientHeight);
  return distance <= threshold;
}

function onScroll() {
  const el = scrollContainer.value;
  if (!el) return;
  const nearBottom = isNearBottom(el);
  if (nearBottom && !lastBottomState.value) {
    emit("reachedBottom");
  }
  lastBottomState.value = nearBottom;
}

function latestOwnMessageElement(): HTMLElement | null {
  const el = scrollContainer.value;
  if (!el) return null;
  return el.querySelector<HTMLElement>("[data-latest-own-message='true'][data-message-id]");
}

function latestMessageElement(): HTMLElement | null {
  const el = scrollContainer.value;
  if (!el) return null;
  return el.querySelector<HTMLElement>("[data-latest-message='true'][data-message-id]");
}

function latestAssistantContentElement(): HTMLElement | null {
  const el = scrollContainer.value;
  if (!el) return null;
  return el.querySelector<HTMLElement>("[data-latest-assistant-content='true']");
}

function outerHeightWithMargins(el: HTMLElement | null): number {
  if (!el) return 0;
  const styles = window.getComputedStyle(el);
  return el.getBoundingClientRect().height
    + parseFloat(styles.marginTop || "0")
    + parseFloat(styles.marginBottom || "0");
}

function latestOwnMessageOuterHeight(): number {
  return outerHeightWithMargins(latestOwnMessageElement());
}

function latestAssistantContentHeight(): number {
  return outerHeightWithMargins(latestAssistantContentElement());
}

function updateLatestMessageMinHeight() {
  const scrollEl = scrollContainer.value;
  const latestMessageEl = latestMessageElement();
  const latestOwnEl = latestOwnMessageElement();
  if (!scrollEl || !latestMessageEl || !latestOwnEl || latestMessageEl === latestOwnEl) {
    latestMessageMinHeight.value = 0;
    return;
  }
  if (latestMessageEl.getAttribute("data-message-role") !== "assistant") {
    latestMessageMinHeight.value = 0;
    return;
  }
  const scrollStyles = window.getComputedStyle(scrollEl);
  const scrollViewportHeight =
    scrollEl.clientHeight
    - parseFloat(scrollStyles.paddingTop || "0")
    - parseFloat(scrollStyles.paddingBottom || "0");
  const toolbarHeight = toolbarContainer.value?.offsetHeight ?? 0;
  const availableHeight = Math.max(0, scrollViewportHeight - latestOwnMessageOuterHeight() - toolbarHeight);
  const contentHeight = latestAssistantContentHeight();
  const nextMinHeight = Math.max(availableHeight, contentHeight);
  if (Math.abs(latestMessageMinHeight.value - nextMinHeight) > 0.5) {
    latestMessageMinHeight.value = nextMinHeight;
  }
  if (pendingAlignOnLatestContainerReady) {
    pendingAlignOnLatestContainerReady = false;
    nextTick(() => {
      requestAnimationFrame(() => {
        alignLatestOwnMessageToTop();
      });
    });
  }
}

function alignLatestOwnMessageToTop(): boolean {
  const latestOwnMessage = latestOwnMessageElement();
  if (!latestOwnMessage) return false;
  latestOwnMessage.scrollIntoView({
    block: "start",
    behavior: "smooth",
  });
  return true;
}

function syncLatestMessageLayoutObserver() {
  if (latestMessageLayoutObserver) {
    latestMessageLayoutObserver.disconnect();
    latestMessageLayoutObserver = null;
  }
  if (typeof ResizeObserver === "undefined") return;
  latestMessageLayoutObserver = new ResizeObserver(() => {
    updateLatestMessageMinHeight();
  });
  const scrollEl = scrollContainer.value;
  const latestOwnEl = latestOwnMessageElement();
  const contentEl = latestAssistantContentElement();
  const toolbarEl = toolbarContainer.value;
  if (scrollEl) latestMessageLayoutObserver.observe(scrollEl);
  if (latestOwnEl) latestMessageLayoutObserver.observe(latestOwnEl);
  if (contentEl) latestMessageLayoutObserver.observe(contentEl);
  if (toolbarEl) latestMessageLayoutObserver.observe(toolbarEl);
}

async function handleAssistantLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = anchor.getAttribute("href")?.trim() || "";
  if (!href || (!href.startsWith("http://") && !href.startsWith("https://"))) return;
  event.preventDefault();
  event.stopPropagation();
  try {
    await invokeTauri("open_external_url", { url: href });
    linkOpenErrorText.value = "";
  } catch (error) {
    linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
  }
}

onMounted(() => {
  loadChatInputHistory();
  loadRecentConversationTopics();
  document.addEventListener("pointerdown", handleDocumentPointerDown, true);
  window.addEventListener("keydown", handleWindowKeydown);
  nextTick(() => {
    resizeChatInput();
    syncConversationLayoutMode();
    updateJumpToBottomOffset();
    if (composerContainer.value && typeof ResizeObserver !== "undefined") {
      composerResizeObserver = new ResizeObserver(() => {
        updateJumpToBottomOffset();
      });
      composerResizeObserver.observe(composerContainer.value);
    }
    if (chatLayoutRoot.value && typeof ResizeObserver !== "undefined") {
      chatLayoutResizeObserver = new ResizeObserver(() => {
        syncConversationLayoutMode();
        updateLatestMessageMinHeight();
        syncLatestMessageLayoutObserver();
      });
      chatLayoutResizeObserver.observe(chatLayoutRoot.value);
    }
    updateLatestMessageMinHeight();
    syncLatestMessageLayoutObserver();
    const el = scrollContainer.value;
    if (el) {
      lastBottomState.value = isNearBottom(el);
    }
  });
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown, true);
  window.removeEventListener("keydown", handleWindowKeydown);
  if (resizeInputRaf) {
    cancelAnimationFrame(resizeInputRaf);
    resizeInputRaf = 0;
  }
  if (composerResizeObserver) {
    composerResizeObserver.disconnect();
    composerResizeObserver = null;
  }
  if (chatLayoutResizeObserver) {
    chatLayoutResizeObserver.disconnect();
    chatLayoutResizeObserver = null;
  }
  if (latestMessageLayoutObserver) {
    latestMessageLayoutObserver.disconnect();
    latestMessageLayoutObserver = null;
  }
  stopAudioPlayback();
});

watch(
  () => props.chatInput,
  (nextValue, prevValue) => {
    if (!chatInputHistoryApplying && nextValue !== prevValue && chatInputHistoryCursor.value !== -1) {
      chatInputHistoryCursor.value = -1;
      chatInputHistoryDraft.value = "";
    }
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => props.chatting,
  (isChatting, wasChatting) => {
    if (wasChatting && !isChatting && !props.frozen) {
      nextTick(() => chatInputRef.value?.focus({ preventScroll: true }));
    }
  },
);

watch(
  () => props.activeConversationId,
  () => {
    pendingAlignOnLatestContainerReady = false;
    nextTick(() => {
      updateLatestMessageMinHeight();
      syncLatestMessageLayoutObserver();
      updateJumpToBottomOffset();
      const el = scrollContainer.value;
      if (el) {
        lastBottomState.value = isNearBottom(el);
      }
    });
  },
);

watch(showSideConversationList, () => {
  nextTick(() => {
    updateLatestMessageMinHeight();
    syncLatestMessageLayoutObserver();
    updateJumpToBottomOffset();
  });
});

watch(
  () => props.messageBlocks.length,
  (nextLength, prevLength) => {
    nextTick(() => {
      updateLatestMessageMinHeight();
      syncLatestMessageLayoutObserver();
      updateJumpToBottomOffset();
      const el = scrollContainer.value;
      if (el) {
        lastBottomState.value = isNearBottom(el);
      }
      if (typeof prevLength === "number" && nextLength < prevLength) {
        requestAnimationFrame(() => {
          scrollToBottom("smooth");
        });
      }
    });
  },
);

watch(
  () => props.latestOwnMessageAlignRequest,
  (nextValue, prevValue) => {
    if (!nextValue || nextValue === prevValue) return;
    pendingAlignOnLatestContainerReady = true;
    nextTick(() => {
      updateLatestMessageMinHeight();
      syncLatestMessageLayoutObserver();
      updateJumpToBottomOffset();
      const el = scrollContainer.value;
      if (el) lastBottomState.value = isNearBottom(el);
      requestAnimationFrame(() => {
        scrollToBottom("smooth");
      });
    });
  },
);

watch(
  () => props.conversationScrollToBottomRequest,
  (nextValue, prevValue) => {
    if (!nextValue || nextValue === prevValue) return;
    nextTick(() => {
      requestAnimationFrame(() => {
        scrollToBottom("smooth");
      });
    });
  },
);

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

.assistant-markdown :deep(.ecall-markdown-content.ecall-stream-content) > * {
  animation: ecall-stream-fade-in 0.5s ease-out forwards;
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

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4,h5,h6,p,ul,ol,pre,blockquote,figure)) {
  margin: 0.24em 0;
}



.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: currentColor;
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: hsl(var(--bc) / 0.5);
  --tw-prose-hr: hsl(var(--bc) / 0.15);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: hsl(var(--bc) / 0.2);
  --tw-prose-captions: hsl(var(--bc) / 0.75);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: hsl(var(--b2));
  --tw-prose-th-borders: hsl(var(--bc) / 0.2);
  --tw-prose-td-borders: hsl(var(--bc) / 0.15);
}

.assistant-markdown :deep(.ecall-markdown-content pre) {
  overflow-x: auto;
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
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
