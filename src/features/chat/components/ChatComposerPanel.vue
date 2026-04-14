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
    <div v-if="selectedInstructionPrompts.length > 0" class="mb-2 flex flex-wrap gap-1">
      <div
        v-for="item in selectedInstructionPrompts"
        :key="item.id"
        class="badge badge-outline gap-1 py-3"
      >
        <Command class="h-3.5 w-3.5" />
        <span class="text-[11px]">{{ item.name }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="removeSelectedInstructionPreset(item.id)">
          <X class="h-3 w-3" />
        </button>
      </div>
    </div>
    <div class="flex flex-col gap-2">
      <div v-if="instructionPanelOpen" class="rounded-box border border-base-300 bg-base-100 p-2 grid gap-1 max-h-48 overflow-y-auto">
        <button
          v-for="(item, index) in normalizedInstructionPresets"
          :key="item.id"
          type="button"
          class="btn btn-sm justify-start normal-case h-auto min-h-0 py-2 px-3"
          :class="instructionFocusIndex === index ? 'btn-primary' : 'btn-ghost'"
          @click="applyInstructionPreset(item)"
        >
          <span class="text-left w-full">
            <span class="block text-sm font-medium">{{ item.name }}</span>
            <span class="block text-[11px] opacity-70 whitespace-pre-wrap break-all">{{ item.prompt }}</span>
          </span>
        </button>
        <div v-if="normalizedInstructionPresets.length === 0" class="px-2 py-3 text-sm opacity-60">
          {{ t("chat.noInstructionPresets") }}
        </div>
      </div>
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
            class="btn btn-sm btn-ghost shrink-0"
            :disabled="chatting || frozen"
            :title="t('chat.command')"
            @click="toggleInstructionPanel"
          >
            <Command class="h-3.5 w-3.5" />
            <span>{{ t("chat.command") }}</span>
          </button>
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
        </div>
        <div class="flex items-center gap-2">
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
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Command, FileText, Image as ImageIcon, Mic, Paperclip, Send, Square, X } from "lucide-vue-next";
import type { ChatConversationOverviewItem, PromptCommandPreset } from "../../../types/app";
import ChatQueuePreview from "./ChatQueuePreview.vue";
import { useChatQueue } from "../composables/use-chat-queue";

type BinaryAttachment = { mime: string; bytesBase64: string };
type QueuedAttachmentNotice = { id: string; fileName: string; relativePath: string; mime: string };
type ConversationDepartmentOption = {
  id: string;
  name: string;
  ownerName: string;
};

const props = defineProps<{
  chatInput: string;
  instructionPresets: PromptCommandPreset[];
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
  (e: "update:selectedInstructionPrompts", value: PromptCommandPreset[]): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
}>();

const { t } = useI18n();
const { queueEvents, sessionState, removeFromQueue } = useChatQueue();

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;

const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const chatInputHistoryApplying = ref(false);
const resizeInputRaf = ref(0);
const instructionPanelOpen = ref(false);
const instructionFocusIndex = ref(0);
const selectedInstructionPrompts = ref<PromptCommandPreset[]>([]);

const normalizedInstructionPresets = computed(() =>
  (Array.isArray(props.instructionPresets) ? props.instructionPresets : [])
    .map((item) => ({
      id: String(item?.id || "").trim(),
      name: String(item?.name || "").trim(),
      prompt: String(item?.prompt || "").trim(),
    }))
    .filter((item) => !!item.id && !!item.name && !!item.prompt),
);

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

function pushChatInputHistory(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  chatInputHistory.value = [text, ...chatInputHistory.value.filter((item) => item !== text)].slice(0, CHAT_INPUT_HISTORY_LIMIT);
  saveChatInputHistory();
  chatInputHistoryCursor.value = -1;
  chatInputHistoryDraft.value = "";
}

function emitSelectedInstructionPrompts() {
  emit("update:selectedInstructionPrompts", selectedInstructionPrompts.value);
}

function openInstructionPanel() {
  instructionPanelOpen.value = true;
  if (instructionFocusIndex.value >= normalizedInstructionPresets.value.length) {
    instructionFocusIndex.value = Math.max(0, normalizedInstructionPresets.value.length - 1);
  }
}

function closeInstructionPanel() {
  instructionPanelOpen.value = false;
}

function toggleInstructionPanel() {
  if (instructionPanelOpen.value) {
    closeInstructionPanel();
    return;
  }
  openInstructionPanel();
}

function applyInstructionPreset(item: PromptCommandPreset | undefined) {
  if (!item) return;
  if (selectedInstructionPrompts.value.some((entry) => entry.id === item.id)) return;
  selectedInstructionPrompts.value = [...selectedInstructionPrompts.value, item];
  emitSelectedInstructionPrompts();
}

function removeSelectedInstructionPreset(id: string) {
  selectedInstructionPrompts.value = selectedInstructionPrompts.value.filter((item) => item.id !== id);
  emitSelectedInstructionPrompts();
}

function clearSelectedInstructionPrompts() {
  if (selectedInstructionPrompts.value.length === 0) return;
  selectedInstructionPrompts.value = [];
  emitSelectedInstructionPrompts();
}

function moveInstructionFocus(delta: number) {
  const list = normalizedInstructionPresets.value;
  if (list.length === 0) return;
  const next = instructionFocusIndex.value + delta;
  instructionFocusIndex.value = Math.max(0, Math.min(list.length - 1, next));
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
  clearSelectedInstructionPrompts();
  closeInstructionPanel();
}

function handleChatInputKeydown(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    openInstructionPanel();
    return;
  }
  if (instructionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeInstructionPanel();
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveInstructionFocus(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveInstructionFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      applyInstructionPreset(normalizedInstructionPresets.value[instructionFocusIndex.value]);
      return;
    }
  }
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

function focusInput(options?: FocusOptions) {
  chatInputRef.value?.focus(options);
}

defineExpose({
  focusInput,
});

onMounted(() => {
  loadChatInputHistory();
  nextTick(() => {
    resizeChatInput();
  });
});

onBeforeUnmount(() => {
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
    closeInstructionPanel();
    clearSelectedInstructionPrompts();
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => normalizedInstructionPresets.value,
  (list) => {
    if (list.length === 0) {
      instructionFocusIndex.value = 0;
      selectedInstructionPrompts.value = [];
      emitSelectedInstructionPrompts();
      instructionPanelOpen.value = false;
      return;
    }
    if (instructionFocusIndex.value >= list.length) {
      instructionFocusIndex.value = list.length - 1;
    }
    selectedInstructionPrompts.value = selectedInstructionPrompts.value.filter((item) =>
      list.some((entry) => entry.id === item.id),
    );
    emitSelectedInstructionPrompts();
  },
  { deep: true },
);
</script>
