<template>
  <div>
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
      <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-ghost gap-1 py-3">
        <ImageIcon v-if="isImageMime(img.mime)" class="h-3.5 w-3.5" />
        <FileText v-else-if="isPdfMime(img.mime)" class="h-3.5 w-3.5" />
        <ImageIcon v-else class="h-3.5 w-3.5" />
        <span class="text-[11px]">{{ isPdfMime(img.mime) ? `PDF ${idx + 1}` : t("chat.image", { index: idx + 1 }) }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="emit('removeClipboardImage', idx)">
          <X class="h-3 w-3" />
        </button>
      </div>
      <div
        v-for="(file, idx) in queuedAttachmentNotices"
        :key="file.id"
        class="badge badge-ghost gap-1 py-3"
      >
        <FileText class="h-3.5 w-3.5" />
        <span class="text-[11px]">{{ file.fileName }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="emit('removeQueuedAttachmentNotice', idx)">
          <X class="h-3 w-3" />
        </button>
      </div>
    </div>
    <div v-if="transcribing" class="mb-1 text-[11px] opacity-80 flex items-center gap-1">
      <span class="loading loading-spinner loading-sm"></span>
      <span>{{ t("chat.transcribing") }}</span>
    </div>
    <div class="flex flex-col gap-2">
      <textarea
        ref="chatInputRef"
        v-model="localChatInput"
        class="w-full textarea resize-none overflow-y-auto chat-input-no-focus scrollbar-gutter-stable min-h-8"
        rows="1"
        :disabled="frozen"
        :placeholder="chatInputPlaceholder"
        @input="scheduleResizeChatInput"
        @keydown="handleChatInputKeydown"
      ></textarea>
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2">
          <button
            class="btn btn-sm btn-circle btn-ghost shrink-0"
            :class="{ 'btn-disabled': frozen }"
            :disabled="frozen"
            :title="t('chat.newConversation')"
            @click="handleCreateConversation"
          >
            <Plus class="h-4 w-4" />
          </button>
        </div>
        <div v-if="!showSideConversationList" ref="conversationListPopoverRef" class="relative flex items-center gap-2">
          <button
            class="btn btn-sm btn-ghost shrink-0 gap-1.5 pl-3 pr-2"
            :title="t('chat.conversationList')"
            @click="toggleConversationList"
          >
            <List class="h-3.5 w-3.5" />
            <span class="text-xs">{{ t("chat.conversationListShort") }}</span>
            <span class="badge badge-ghost badge-xs">{{ unarchivedConversationItems.length }}</span>
          </button>
          <div v-if="conversationListOpen" class="absolute bottom-full left-0 z-40 mb-2">
            <ChatConversationListCard
              :items="unarchivedConversationItems"
              :active-conversation-id="activeConversationId"
              :user-alias="userAlias"
              :persona-name-map="personaNameMap"
              :persona-avatar-url-map="personaAvatarUrlMap"
              :user-avatar-url="userAvatarUrl"
              @select-conversation="handleConversationListSelect"
            />
          </div>
        </div>
        <div class="ml-auto flex items-center gap-2">
          <div v-if="!showSideConversationList" class="h-5 w-px shrink-0 bg-base-300"></div>
          <button
            class="btn btn-sm btn-circle btn-ghost shrink-0"
            :disabled="chatting || frozen"
            :title="t('chat.attach')"
            @click="emit('pickAttachments')"
          >
            <Paperclip class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-circle shrink-0"
            :class="recording ? 'btn-error' : 'btn-ghost'"
            :disabled="!canRecord || chatting || frozen"
            :title="recording ? t('chat.recording', { seconds: Math.max(1, Math.round(recordingMs / 1000)) }) : t('chat.holdRecord', { hotkey: recordHotkey })"
            @mousedown.prevent="emit('startRecording')"
            @mouseup.prevent="emit('stopRecording')"
            @mouseleave.prevent="recording && emit('stopRecording')"
            @touchstart.prevent="emit('startRecording')"
            @touchend.prevent="emit('stopRecording')"
          >
            <Mic class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-circle btn-primary shrink-0"
            :disabled="frozen"
            :title="chatting ? `${t('chat.stop')} / ${t('chat.stopReplying')}` : t('chat.send')"
            @click="chatting ? emit('stopChat') : handleSendChat()"
          >
            <Square v-if="chatting" class="h-3.5 w-3.5 fill-current" />
            <Send v-else class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
    </div>

    <dialog class="modal" :class="{ 'modal-open': createConversationDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="text-base font-semibold">{{ t("chat.newConversation") }}</h3>
        <div class="mt-3 flex flex-col gap-3">
          <div class="rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-sm leading-6 text-base-content/80">
            {{ t("chat.unarchivedConversationMemoryReminder") }}
          </div>
          <input
            ref="createConversationInputRef"
            v-model="createConversationTitle"
            type="text"
            class="input input-bordered w-full"
            :placeholder="t('chat.newConversationTopicPlaceholder')"
            @keydown="handleCreateConversationDialogKeydown"
          />
          <select
            v-model="createConversationDepartmentId"
            class="select select-bordered w-full"
          >
            <option
              v-for="department in createConversationDepartmentOptions"
              :key="department.id"
              :value="department.id"
            >
              {{ departmentOptionLabel(department) }}
            </option>
          </select>
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
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FileText, Image as ImageIcon, List, Mic, Paperclip, Plus, Send, Square, X } from "lucide-vue-next";
import type { ChatConversationOverviewItem } from "../../../types/app";
import ChatConversationListCard from "./ChatConversationListCard.vue";
import ChatQueuePreview from "./ChatQueuePreview.vue";
import { useChatQueue } from "../composables/use-chat-queue";

type BinaryAttachment = { mime: string; bytesBase64: string };
type QueuedAttachmentNotice = { id: string; fileName: string; relativePath: string; mime: string };
type ConversationDepartmentOption = {
  id: string;
  name: string;
  ownerName: string;
};
type CreateConversationInput = { title?: string; departmentId?: string };

const props = defineProps<{
  chatInput: string;
  chatInputPlaceholder: string;
  clipboardImages: BinaryAttachment[];
  queuedAttachmentNotices: QueuedAttachmentNotice[];
  linkOpenErrorText: string;
  chatErrorText: string;
  transcribing: boolean;
  canRecord: boolean;
  recording: boolean;
  recordingMs: number;
  recordHotkey: string;
  chatting: boolean;
  frozen: boolean;
  showSideConversationList: boolean;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  createConversationDepartmentOptions: ConversationDepartmentOption[];
  defaultCreateConversationDepartmentId: string;
}>();

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "switchConversation", conversationId: string): void;
  (e: "createConversation", input?: CreateConversationInput): void;
}>();

const { t } = useI18n();
const { queueEvents, sessionState, removeFromQueue } = useChatQueue();

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;
const RECENT_CONVERSATION_TOPICS_STORAGE_KEY = "easy_call.recent_conversation_topics.v1";
const RECENT_CONVERSATION_TOPICS_LIMIT = 7;

const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const createConversationInputRef = ref<HTMLInputElement | null>(null);
const conversationListPopoverRef = ref<HTMLElement | null>(null);
const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const recentConversationTopics = ref<string[]>([]);
const createConversationDialogOpen = ref(false);
const createConversationTitle = ref("");
const createConversationDepartmentId = ref("");
const conversationListOpen = ref(false);
const chatInputHistoryApplying = ref(false);
const resizeInputRaf = ref(0);

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

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  const minHeight = 32;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.max(Math.min(el.scrollHeight, maxHeight), minHeight);
  el.style.height = `${nextHeight}px`;
  el.style.overflowY = "auto";
}

function scheduleResizeChatInput() {
  if (resizeInputRaf.value) cancelAnimationFrame(resizeInputRaf.value);
  resizeInputRaf.value = requestAnimationFrame(() => {
    resizeChatInput();
    resizeInputRaf.value = 0;
  });
}

function applyChatInputHistoryValue(value: string) {
  chatInputHistoryApplying.value = true;
  localChatInput.value = value;
  nextTick(() => {
    chatInputHistoryApplying.value = false;
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

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function handleRecallToInput(event: { source?: string; messagePreview?: string; id?: string }) {
  if (event.source === "user") {
    localChatInput.value = event.messagePreview || "";
    if (event.id) {
      void removeFromQueue(event.id);
    }
  }
}

function closeConversationList() {
  conversationListOpen.value = false;
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

function handleCreateConversation() {
  closeConversationList();
  createConversationTitle.value = "";
  createConversationDepartmentId.value =
    String(props.defaultCreateConversationDepartmentId || "").trim()
    || String(props.createConversationDepartmentOptions[0]?.id || "").trim();
  createConversationDialogOpen.value = true;
  nextTick(() => createConversationInputRef.value?.focus());
}

function closeCreateConversationDialog() {
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
  createConversationDepartmentId.value = "";
}

function applyRecentConversationTopic(topic: string) {
  createConversationTitle.value = String(topic || "").trim();
  nextTick(() => createConversationInputRef.value?.focus());
}

function departmentOptionLabel(department: ConversationDepartmentOption): string {
  const departmentName = String(department.name || "").trim();
  const ownerName = String(department.ownerName || "").trim();
  return ownerName ? `${departmentName} / ${ownerName}` : departmentName;
}

function confirmCreateConversation() {
  const title = String(createConversationTitle.value || "").trim();
  const departmentId = String(createConversationDepartmentId.value || "").trim();
  if (title) {
    pushRecentConversationTopic(title);
  }
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
  createConversationDepartmentId.value = "";
  emit("createConversation", {
    title,
    departmentId: departmentId || undefined,
  });
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

function focusInput(options?: FocusOptions) {
  chatInputRef.value?.focus(options);
}

defineExpose({
  focusInput,
});

onMounted(() => {
  loadChatInputHistory();
  loadRecentConversationTopics();
  document.addEventListener("pointerdown", handleDocumentPointerDown, true);
  window.addEventListener("keydown", handleWindowKeydown);
  nextTick(() => {
    resizeChatInput();
  });
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown, true);
  window.removeEventListener("keydown", handleWindowKeydown);
  if (resizeInputRaf.value) {
    cancelAnimationFrame(resizeInputRaf.value);
    resizeInputRaf.value = 0;
  }
});

watch(
  () => props.chatInput,
  (nextValue, prevValue) => {
    if (!chatInputHistoryApplying.value && nextValue !== prevValue && chatInputHistoryCursor.value !== -1) {
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
      nextTick(() => focusInput({ preventScroll: true }));
    }
  },
);

watch(
  () => props.activeConversationId,
  () => {
    closeConversationList();
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => props.showSideConversationList,
  (visible) => {
    if (visible) {
      closeConversationList();
    }
  },
);
</script>
