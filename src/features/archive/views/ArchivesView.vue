<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex shrink-0 flex-col items-center gap-2 p-2">
      <div role="tablist" class="tabs tabs-border">
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ 'tab-active font-semibold': viewMode === 'current' }"
          @click="switchViewMode('current')"
        >
          {{ t("archives.currentUnarchived") }}
        </button>
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ 'tab-active font-semibold': viewMode === 'delegate' }"
          @click="switchViewMode('delegate')"
        >
          {{ t("archives.delegateConversations") }}
        </button>
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ 'tab-active font-semibold': viewMode === 'archive' }"
          @click="switchViewMode('archive')"
        >
          {{ t("archives.archivedMessages") }}
        </button>
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ 'tab-active font-semibold': viewMode === 'remoteIm' }"
          @click="switchViewMode('remoteIm')"
        >
          联系人消息
        </button>
      </div>
      <div class="flex flex-wrap items-center justify-center gap-2">
        <button class="btn btn-neutral" @click="$emit('loadArchives')">
          <RefreshCw class="h-4 w-4" />
          {{ t("archives.refresh") }}
        </button>
        <button
          v-if="viewMode === 'archive'"
          class="btn btn-warning"
          :disabled="!selectedArchiveId"
          @click="$emit('unarchiveArchive', selectedArchiveId)"
        >
          <Undo2 class="h-4 w-4" />
          {{ t("archives.unarchive") }}
        </button>
        <button class="btn btn-primary" @click="triggerArchiveImport">
          <Import class="h-4 w-4" />
          {{ t("archives.importJson") }}
        </button>
        <button class="btn btn-primary" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'markdown' })">
          <FileDown class="h-4 w-4" />
          {{ t("archives.exportMarkdown") }}
        </button>
        <button class="btn btn-primary" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'json' })">
          <FileJson2 class="h-4 w-4" />
          {{ t("archives.exportJson") }}
        </button>
        <button
          class="btn btn-error"
          :disabled="(viewMode === 'delegate' && !selectedDelegateConversationId) || (viewMode === 'archive' && !selectedArchiveId) || (viewMode === 'current' && (!selectedUnarchivedConversationId || selectedCurrentConversationSummary?.isSystemNotificationConversation)) || (viewMode === 'remoteIm' && !selectedRemoteImContactId)"
          @click="viewMode === 'archive' ? onDeleteArchiveClick(selectedArchiveId) : viewMode === 'delegate' ? onDeleteDelegateClick(selectedDelegateConversationId) : viewMode === 'remoteIm' ? onDeleteRemoteImContactClick(selectedRemoteImContactId) : onDeleteUnarchivedClick(selectedUnarchivedConversationId)"
        >
          <Trash2 class="h-4 w-4" />
          {{ t("common.delete") }}
        </button>
        <input
          ref="archiveImportInputRef"
          type="file"
          accept=".json,application/json"
          class="hidden"
          @change="onArchiveImportChange"
        />
      </div>
    </div>
    <div class="flex flex-1 min-h-0">
      <div class="w-56 min-w-0 shrink-0 overflow-x-hidden overflow-y-auto">
        <ul v-if="viewMode === 'current'" class="menu menu-sm w-full min-w-0 max-w-full overflow-hidden p-0 gap-1">
          <li v-for="c in unarchivedConversations" :key="c.conversationId" class="min-w-0 max-w-full overflow-hidden">
            <button
              type="button"
              class="flex w-full min-w-0 flex-col items-start text-left"
              :class="{ 'menu-active': c.conversationId === selectedUnarchivedConversationId }"
              @click="$emit('selectUnarchivedConversation', c.conversationId)"
            >
              <span class="block w-full truncate text-sm font-medium">{{ conversationDisplayTitle(c) }}</span>
              <span class="block w-full truncate text-xs opacity-70">{{ formatDate(c.lastMessageAt || c.updatedAt) }}</span>
            </button>
          </li>
        </ul>
        <ul v-else-if="viewMode === 'archive'" class="menu menu-sm w-full min-w-0 max-w-full overflow-hidden p-0 gap-1">
          <li v-for="a in archives" :key="a.archiveId" class="min-w-0 max-w-full overflow-hidden">
            <button
              type="button"
              class="flex w-full min-w-0 flex-col items-start text-left"
              :class="{ 'menu-active': a.archiveId === selectedArchiveId }"
              @click="$emit('selectArchive', a.archiveId)"
            >
              <span class="block w-full truncate text-sm font-medium">{{ a.title }}</span>
              <span v-if="a.archivedAt" class="block w-full truncate text-xs opacity-70">{{ formatDate(a.archivedAt) }}</span>
            </button>
          </li>
        </ul>
        <ul v-else-if="viewMode === 'delegate'" class="menu menu-sm w-full min-w-0 max-w-full overflow-hidden p-0 gap-1">
          <li v-for="c in delegateConversations" :key="c.conversationId" class="min-w-0 max-w-full overflow-hidden">
            <button
              type="button"
              class="flex w-full min-w-0 flex-col items-start text-left"
              :class="{ 'menu-active': c.conversationId === selectedDelegateConversationId }"
              @click="$emit('selectDelegateConversation', c.conversationId)"
            >
              <span class="block w-full truncate text-sm font-medium">{{ conversationDisplayTitle(c) }}</span>
              <span class="block w-full truncate text-xs opacity-70">{{ formatDate(c.archivedAt || c.lastMessageAt || c.updatedAt) }}</span>
            </button>
          </li>
        </ul>
        <ul v-else class="menu menu-sm w-full min-w-0 max-w-full overflow-hidden p-0 gap-1">
          <li v-for="c in remoteImContactConversations" :key="c.contactId" class="min-w-0 max-w-full overflow-hidden">
            <button
              type="button"
              class="flex w-full min-w-0 flex-col items-start text-left"
              :class="{ 'menu-active': c.contactId === selectedRemoteImContactId }"
              @click="$emit('selectRemoteImContactConversation', c.contactId)"
            >
              <span class="block w-full truncate text-sm font-medium">{{ c.contactDisplayName }}</span>
              <span class="block w-full truncate text-xs opacity-70">{{ formatDate(c.lastMessageAt || c.updatedAt) }}</span>
            </button>
          </li>
        </ul>
      </div>
      <div class="flex min-w-0 flex-1 flex-col">
        <div ref="messageScrollerRef" class="flex-1 min-h-0 overflow-auto space-y-2">
          <div
            v-if="(viewMode === 'archive' && archiveBlocks.length > 0) || (viewMode === 'current' && unarchivedBlocks.length > 0) || (viewMode === 'delegate' && delegateBlocks.length > 0) || (viewMode === 'remoteIm' && remoteImContactBlocks.length > 0)"
            class="sticky top-0 z-10 flex items-center justify-between gap-2 rounded border border-base-300 bg-base-200/95 px-3 py-2 backdrop-blur"
          >
            <button
              type="button"
              class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200"
              :disabled="!activeHasPrevBlock"
              @click="focusAdjacentArchiveBlock(-1)"
            >
              {{ t("archives.prevBlock") }}
            </button>
            <div class="min-w-0 flex-1 text-center">
              <div class="truncate text-sm font-medium">
                {{ selectedArchiveBlockSummaryLabel }}
              </div>
              <div class="truncate text-xs opacity-70">
                {{ selectedArchiveBlockRangeLabel }}
              </div>
            </div>
            <button
              type="button"
              class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200"
              :disabled="!activeHasNextBlock"
              @click="focusAdjacentArchiveBlock(1)"
            >
              {{ t("archives.nextBlock") }}
            </button>
          </div>
          <div
            v-for="m in visibleMessages"
            :key="m.id"
            class="chat"
            :class="archiveMessageChatClass(m)"
          >
            <div class="chat-header text-xs opacity-60">
              {{ speakerLabel(m) }}
              <time v-if="m.createdAt" class="ml-2">{{ formatDate(m.createdAt) }}</time>
            </div>
            <div class="chat-bubble max-w-[82%]" :class="archiveMessageBubbleClass(m)">
              <details
                v-if="isCollapsibleArchiveMessage(m)"
                class="collapse collapse-arrow border border-base-300/70 bg-base-200/60"
              >
                <summary class="collapse-title min-h-0 px-3 py-2 text-sm font-medium">
                  {{ collapsibleArchiveMessageTitle(m) }}
                </summary>
                <div class="collapse-content px-3 pb-3 pt-0">
                  <div class="whitespace-pre-wrap break-words text-sm leading-7">
                    {{ messageText(m) }}
                  </div>
                </div>
              </details>
              <div
                v-else-if="messageText(m)"
                class="whitespace-pre-wrap break-words text-sm leading-7"
              >{{ messageText(m) }}</div>
              <div
                v-if="messageAttachments(m).length > 0"
                class="mt-2 flex flex-wrap gap-2"
              >
                <button
                  v-for="(file, idx) in messageAttachments(m)"
                  :key="`${m.id}-attachment-${idx}`"
                  type="button"
                  class="link text-sm"
                  :class="m.role === 'user' ? 'link-secondary' : 'link-primary'"
                  :title="file.path"
                  @click="openAttachment(file.path)"
                >
                  {{ file.fileName }}
                </button>
              </div>
              <div v-if="messageImages(m).length > 0" class="mt-2 grid gap-1">
                <img
                  v-for="(img, idx) in messageImages(m)"
                  :key="`${img.mime}-${idx}`"
                  :src="resolvedArchiveImageSrc(m.id, img, idx)"
                  class="rounded max-h-44 object-contain bg-base-100/40 border border-base-300"
                />
              </div>
              <div v-if="messageAudios(m).length > 0" class="mt-2 grid gap-1">
                <audio
                  v-for="(audio, idx) in messageAudios(m)"
                  :key="`${m.id}-audio-${idx}`"
                  :src="archiveAudioSrc(m.id, audio, idx)"
                  controls
                  preload="none"
                  class="max-w-full"
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <dialog ref="confirmDialog" class="modal">
      <div class="modal-box max-w-md p-4">
        <h3 class="text-sm font-semibold">{{ confirmDialogState.title }}</h3>
        <p class="mt-3 text-sm whitespace-pre-wrap">{{ confirmDialogState.message }}</p>
        <div class="modal-action mt-4">
          <button class="btn btn-sm btn-ghost" type="button" @click="closeConfirmDialog(false)">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-error" type="button" @click="closeConfirmDialog(true)">
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click.prevent="closeConfirmDialog(false)">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect } from "vue";
import { FileDown, FileJson2, Import, RefreshCw, Trash2, Undo2 } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import {
  invokeTauri,
  openTransportWorkspaceFile,
  readTransportChatImage,
  readTransportLocalBinaryFile,
  resolveLocalFileUrl,
} from "../../../services/tauri-api";
import { extractMessageAttachmentFiles, extractMessageAudios, extractMessageImages } from "../../../utils/chat-message";
import { resolveConversationDisplayTitle } from "../../chat/utils/conversation-title";
import type {
  ArchiveSummary,
  ChatMessage,
  ConversationBlockSummary,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  MessagePart,
  UnarchivedConversationSummary,
} from "../../../types/app";

const props = defineProps<{
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveBlocks: ConversationBlockSummary[];
  selectedArchiveBlockId?: number | null;
  archiveHasPrevBlock?: boolean;
  archiveHasNextBlock?: boolean;
  archiveMessages: ChatMessage[];
  unarchivedConversations: UnarchivedConversationSummary[];
  unarchivedBlocks: ConversationBlockSummary[];
  selectedUnarchivedConversationId: string;
  selectedUnarchivedBlockId?: number | null;
  unarchivedHasPrevBlock?: boolean;
  unarchivedHasNextBlock?: boolean;
  unarchivedMessages: ChatMessage[];
  delegateConversations: DelegateConversationSummary[];
  delegateBlocks: ConversationBlockSummary[];
  selectedDelegateConversationId: string;
  selectedDelegateBlockId?: number | null;
  delegateHasPrevBlock?: boolean;
  delegateHasNextBlock?: boolean;
  delegateMessages: ChatMessage[];
  remoteImContactConversations: RemoteImContactConversationSummary[];
  remoteImContactBlocks: ConversationBlockSummary[];
  selectedRemoteImContactId: string;
  selectedRemoteImContactBlockId?: number | null;
  remoteImHasPrevBlock?: boolean;
  remoteImHasNextBlock?: boolean;
  remoteImContactMessages: ChatMessage[];
  userAlias: string;
  personaNameMap?: Record<string, string>;
  currentTheme: string;
}>();
const { t, locale } = useI18n();

const emit = defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
  (e: "selectArchiveBlock", blockId?: number | null): void;
  (e: "selectUnarchivedConversation", conversationId: string): void;
  (e: "selectUnarchivedBlock", blockId?: number | null): void;
  (e: "selectDelegateConversation", conversationId: string): void;
  (e: "selectDelegateBlock", blockId?: number | null): void;
  (e: "selectRemoteImContactConversation", contactId: string): void;
  (e: "selectRemoteImContactBlock", blockId?: number | null): void;
  (e: "exportArchive", payload: { format: "markdown" | "json" }): void;
  (e: "unarchiveArchive", archiveId: string): void;
  (e: "deleteArchive", archiveId: string): void;
  (e: "deleteUnarchivedConversation", conversationId: string): void;
  (e: "deleteDelegateConversation", conversationId: string): void;
  (e: "deleteRemoteImContactConversation", contactId: string): void;
  (e: "importArchiveFile", file: File): void;
}>();

const viewMode = ref<"current" | "delegate" | "archive" | "remoteIm">("archive");
const ARCHIVE_FOCUS_REQUEST_STORAGE_KEY = "easy_call.archives.focus_request.v1";
const ARCHIVE_FOCUS_REQUEST_TTL_MS = 30_000;
const archiveImageDataUrlCache = new Map<string, string>();
const archiveImagePendingCache = new Map<string, Promise<string>>();
const archiveResolvedImageMap = ref<Record<string, string>>({});
const archiveResolvedAudioMap = ref<Record<string, string>>({});
const messageScrollerRef = ref<HTMLElement | null>(null);
const confirmDialog = ref<HTMLDialogElement | null>(null);
const confirmDialogState = ref({
  title: "",
  message: "",
});
let resolveConfirmDialog: ((value: boolean) => void) | null = null;

const activeMessageSource = computed(() =>
  viewMode.value === "current"
    ? props.unarchivedMessages
    : viewMode.value === "delegate"
      ? props.delegateMessages
      : viewMode.value === "remoteIm"
        ? props.remoteImContactMessages
        : props.archiveMessages,
);
const visibleMessages = computed(() =>
  activeMessageSource.value.filter(isArchiveDialogueMessage),
);
const selectedArchiveBlockSummary = computed(() =>
  props.archiveBlocks.find((item) => item.blockId === props.selectedArchiveBlockId) ?? null,
);
const selectedUnarchivedBlockSummary = computed(() =>
  props.unarchivedBlocks.find((item) => item.blockId === props.selectedUnarchivedBlockId) ?? null,
);
const selectedDelegateBlockSummary = computed(() =>
  props.delegateBlocks.find((item) => item.blockId === props.selectedDelegateBlockId) ?? null,
);
const selectedRemoteImBlockSummary = computed(() =>
  props.remoteImContactBlocks.find((item) => item.blockId === props.selectedRemoteImContactBlockId) ?? null,
);
const activeBlocks = computed(() =>
  viewMode.value === "current"
    ? props.unarchivedBlocks
    : viewMode.value === "delegate"
      ? props.delegateBlocks
    : viewMode.value === "remoteIm"
      ? props.remoteImContactBlocks
      : props.archiveBlocks,
);
const activeSelectedBlockId = computed(() =>
  viewMode.value === "current"
    ? props.selectedUnarchivedBlockId
    : viewMode.value === "delegate"
      ? props.selectedDelegateBlockId
    : viewMode.value === "remoteIm"
      ? props.selectedRemoteImContactBlockId
      : props.selectedArchiveBlockId,
);
const activeHasPrevBlock = computed(() =>
  viewMode.value === "current"
    ? !!props.unarchivedHasPrevBlock
    : viewMode.value === "delegate"
      ? !!props.delegateHasPrevBlock
    : viewMode.value === "remoteIm"
      ? !!props.remoteImHasPrevBlock
      : !!props.archiveHasPrevBlock,
);
const activeHasNextBlock = computed(() =>
  viewMode.value === "current"
    ? !!props.unarchivedHasNextBlock
    : viewMode.value === "delegate"
      ? !!props.delegateHasNextBlock
    : viewMode.value === "remoteIm"
      ? !!props.remoteImHasNextBlock
      : !!props.archiveHasNextBlock,
);
const activeSelectedBlockSummary = computed(() =>
  viewMode.value === "current"
    ? selectedUnarchivedBlockSummary.value
    : viewMode.value === "delegate"
      ? selectedDelegateBlockSummary.value
    : viewMode.value === "remoteIm"
      ? selectedRemoteImBlockSummary.value
      : selectedArchiveBlockSummary.value,
);
const selectedArchiveBlockSummaryLabel = computed(() => {
  const block = activeSelectedBlockSummary.value;
  if (!block) return "";
  return t("archives.blockSummary", {
    id: block.blockId + 1,
    count: block.messageCount,
  });
});
const selectedArchiveBlockRangeLabel = computed(() => {
  const block = activeSelectedBlockSummary.value;
  if (!block) return "";
  const startRaw = formatDate(block.firstCreatedAt || "");
  const endRaw = formatDate(block.lastCreatedAt || "");
  const start = startRaw === "-" ? "" : startRaw;
  const end = endRaw === "-" ? "" : endRaw;
  if (start && end && start !== end) {
    return `${start} - ${end}`;
  }
  return start || end || "";
});
const selectedCurrentConversationSummary = computed(() =>
  props.unarchivedConversations.find(
    (item) => String(item.conversationId || "").trim() === String(props.selectedUnarchivedConversationId || "").trim(),
  ) ?? null,
);
const archiveImportInputRef = ref<HTMLInputElement | null>(null);

function switchViewMode(mode: "current" | "delegate" | "archive" | "remoteIm") {
  viewMode.value = mode;
}

function scrollMessagesToBottom() {
  void nextTick(() => {
    const scroller = messageScrollerRef.value;
    if (!scroller) return;
    scroller.scrollTop = scroller.scrollHeight;
    requestAnimationFrame(() => {
      scroller.scrollTop = scroller.scrollHeight;
    });
  });
}

function focusAdjacentArchiveBlock(step: -1 | 1) {
  const currentIndex = activeBlocks.value.findIndex((item) => item.blockId === activeSelectedBlockId.value);
  if (currentIndex < 0) return;
  const next = activeBlocks.value[currentIndex + step];
  if (!next) return;
  if (viewMode.value === "current") {
    emit("selectUnarchivedBlock", next.blockId);
    return;
  }
  if (viewMode.value === "delegate") {
    emit("selectDelegateBlock", next.blockId);
    return;
  }
  if (viewMode.value === "remoteIm") {
    emit("selectRemoteImContactBlock", next.blockId);
    return;
  }
  emit("selectArchiveBlock", next.blockId);
}

function readPendingArchiveFocusRequest(): { conversationId: string; viewMode: "current" | "delegate" } | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const conversationId = String(parsed.conversationId || "").trim();
    const createdAt = Number(parsed.createdAt || 0);
    const requestViewMode = String(parsed.viewMode || "").trim();
    if (!conversationId) return null;
    if (!Number.isFinite(createdAt) || Date.now() - createdAt > ARCHIVE_FOCUS_REQUEST_TTL_MS) {
      window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
      return null;
    }
    if (requestViewMode !== "current" && requestViewMode !== "delegate") {
      window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
      return null;
    }
    return {
      conversationId,
      viewMode: requestViewMode,
    };
  } catch {
    window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
    return null;
  }
}

function applyPendingArchiveFocusRequest() {
  const pending = readPendingArchiveFocusRequest();
  if (!pending) return false;
  if (pending.viewMode === "delegate") {
    if (!props.delegateConversations.some((item) => String(item.conversationId || "").trim() === pending.conversationId)) {
      return false;
    }
    viewMode.value = pending.viewMode;
    if (typeof window !== "undefined") {
      window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
    }
    emit("selectDelegateConversation", pending.conversationId);
    return true;
  }
  if (!props.unarchivedConversations.some((item) => String(item.conversationId || "").trim() === pending.conversationId)) {
    return false;
  }
  viewMode.value = pending.viewMode;
  if (typeof window !== "undefined") {
    window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
  }
  emit("selectUnarchivedConversation", pending.conversationId);
  return true;
}

watch(
  () => [props.unarchivedConversations, props.delegateConversations],
  () => {
    applyPendingArchiveFocusRequest();
  },
  { immediate: true },
);

watch(
  () => [
    viewMode.value,
    props.selectedArchiveId,
    props.selectedUnarchivedConversationId,
    props.selectedDelegateConversationId,
    props.selectedRemoteImContactId,
    props.selectedArchiveBlockId,
    props.selectedUnarchivedBlockId,
    props.selectedRemoteImContactBlockId,
    visibleMessages.value.length,
    visibleMessages.value.length > 0 ? visibleMessages.value[visibleMessages.value.length - 1]?.id || "" : "",
  ],
  () => {
    scrollMessagesToBottom();
  },
  { flush: "post" },
);

function handleStorageChange(event: StorageEvent) {
  if (event.key !== ARCHIVE_FOCUS_REQUEST_STORAGE_KEY) return;
  applyPendingArchiveFocusRequest();
}

function handleWindowFocus() {
  applyPendingArchiveFocusRequest();
}

function handleVisibilityChange() {
  if (typeof document !== "undefined" && document.visibilityState !== "visible") return;
  applyPendingArchiveFocusRequest();
}

onMounted(() => {
  applyPendingArchiveFocusRequest();
  scrollMessagesToBottom();
  if (typeof window !== "undefined") {
    window.addEventListener("storage", handleStorageChange);
    window.addEventListener("focus", handleWindowFocus);
  }
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", handleVisibilityChange);
  }
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("storage", handleStorageChange);
    window.removeEventListener("focus", handleWindowFocus);
  }
  if (typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  }
});

function triggerArchiveImport() {
  if (archiveImportInputRef.value) {
    archiveImportInputRef.value.value = "";
    archiveImportInputRef.value.click();
  }
}

function onArchiveImportChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  emit("importArchiveFile", file);
}

async function onDeleteArchiveClick(archiveId: string) {
  if (!archiveId) return;
  const confirmed = await requestConfirmDialog(t("common.delete"), t("archives.deleteConfirm"));
  if (!confirmed) return;
  emit("deleteArchive", archiveId);
}

async function onDeleteUnarchivedClick(conversationId: string) {
  if (!conversationId) return;
  const confirmed = await requestConfirmDialog(t("common.delete"), t("archives.deleteUnarchivedConfirm"));
  if (!confirmed) return;
  emit("deleteUnarchivedConversation", conversationId);
}

async function onDeleteDelegateClick(conversationId: string) {
  if (!conversationId) return;
  const confirmed = await requestConfirmDialog(t("common.delete"), "确定删除这个委托会话吗？");
  if (!confirmed) return;
  emit("deleteDelegateConversation", conversationId);
}

async function onDeleteRemoteImContactClick(contactId: string) {
  if (!contactId) return;
  const confirmed = await requestConfirmDialog(t("common.delete"), t("archives.deleteUnarchivedConfirm"));
  if (!confirmed) return;
  emit("deleteRemoteImContactConversation", contactId);
}

function requestConfirmDialog(title: string, message: string): Promise<boolean> {
  const dialog = confirmDialog.value;
  if (!dialog) return Promise.resolve(false);
  confirmDialogState.value = {
    title,
    message,
  };
  return new Promise<boolean>((resolve) => {
    resolveConfirmDialog = resolve;
    dialog.showModal();
  });
}

function closeConfirmDialog(value: boolean) {
  const dialog = confirmDialog.value;
  if (dialog?.open) {
    dialog.close();
  }
  resolveConfirmDialog?.(value);
  resolveConfirmDialog = null;
}

function messageText(msg: ChatMessage): string {
  const partText = msg.parts
    .filter((p): p is Extract<MessagePart, { type: "text" }> => p.type === "text")
    .map((p) => p.text)
    .join("\n");
  const extraBlocks = Array.isArray(msg.extraTextBlocks) ? msg.extraTextBlocks.join("\n") : "";
  return [partText, extraBlocks]
    .map((item) => String(item || "").trim())
    .filter((item) => item.length > 0)
    .join("\n")
    .trim();
}

function isContextOrganizationMessage(msg: ChatMessage): boolean {
  if (msg.role !== "user") return false;
  return messageText(msg).trimStart().startsWith("[上下文整理]");
}

function isDelegateTaskMessage(msg: ChatMessage): boolean {
  return messageText(msg).trimStart().toLowerCase().startsWith("<delegate task>");
}

function isCollapsibleArchiveMessage(msg: ChatMessage): boolean {
  return isContextOrganizationMessage(msg) || isDelegateTaskMessage(msg);
}

function collapsibleArchiveMessageTitle(msg: ChatMessage): string {
  if (isDelegateTaskMessage(msg)) return "<delegate task>";
  const firstLine = messageText(msg)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => !!line);
  return firstLine || "[上下文整理]";
}

function isArchiveDialogueMessage(msg: ChatMessage): boolean {
  if (msg.role !== "user" && msg.role !== "assistant") return false;
  return !!messageText(msg)
    || messageAttachments(msg).length > 0
    || messageImages(msg).length > 0;
}

function archiveMessageChatClass(msg: ChatMessage): string {
  return msg.role === "user" ? "chat-end" : "chat-start";
}

function archiveMessageBubbleClass(msg: ChatMessage): string {
  return msg.role === "user"
    ? "chat-bubble-primary text-primary-content"
    : "bg-base-100 text-base-content border border-base-300";
}

function remoteImOriginOfMessage(msg: ChatMessage): {
  senderName?: string;
  remoteContactName?: string;
} | null {
  const origin = (msg as ChatMessage & {
    remoteImOrigin?: {
      senderName?: string;
      remoteContactName?: string;
    };
  }).remoteImOrigin;
  return origin && typeof origin === "object" ? origin : null;
}

function speakerLabel(msg: ChatMessage): string {
  const remoteImOrigin = remoteImOriginOfMessage(msg);
  if (remoteImOrigin) {
    const senderName = String(remoteImOrigin.senderName || "").trim();
    const remoteContactName = String(remoteImOrigin.remoteContactName || "").trim();
    return senderName || remoteContactName || "IM";
  }
  const speakerId = String(msg.speakerAgentId || "").trim();
  if (speakerId) {
    return props.personaNameMap?.[speakerId] || speakerId;
  }
  if (msg.role === "user") {
    return String(props.userAlias || "").trim() || t("archives.roleUser");
  }
  return String(msg.role || "").trim() || "-";
}

function messageAttachments(msg: ChatMessage): Array<{ fileName: string; path: string; mime?: string }> {
  return extractMessageAttachmentFiles(msg);
}

function openAttachment(path: string) {
  if (!path.trim()) return;
  void openTransportWorkspaceFile(path).catch((error) => {
    console.warn("[归档附件] 打开失败", { path, error });
  });
}

function formatDate(value?: string): string {
  if (!value) return "-";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString(locale.value);
}

function conversationDisplayTitle(
  item: UnarchivedConversationSummary | DelegateConversationSummary,
): string {
  return resolveConversationDisplayTitle(item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
  });
}

function messageImages(msg: ChatMessage): Array<{ mime: string; bytesBase64?: string; mediaRef?: string }> {
  return extractMessageImages(msg);
}

function messageAudios(msg: ChatMessage): Array<{ mime: string; bytesBase64?: string; mediaRef?: string }> {
  return extractMessageAudios(msg);
}

function archiveAudioKey(messageId: string, index: number): string {
  return `${String(messageId || "").trim()}::audio::${index}`;
}

function archiveAudioSrc(
  messageId: string,
  audio: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
): string {
  const bytesBase64 = String(audio.bytesBase64 || "").trim();
  if (bytesBase64) return `data:${audio.mime};base64,${bytesBase64}`;
  return String(archiveResolvedAudioMap.value[archiveAudioKey(messageId, index)] || "").trim();
}

async function readArchiveAudioSource(audio: { mime: string; mediaRef?: string }): Promise<string> {
  const mediaRef = String(audio.mediaRef || "").trim();
  if (!mediaRef || mediaRef.startsWith("@")) return "";
  try {
    const result = await readTransportLocalBinaryFile<{ mime?: string; bytesBase64?: string }>({ path: mediaRef });
    const bytesBase64 = String(result?.bytesBase64 || "").trim();
    const mime = String(result?.mime || audio.mime || "audio/mpeg").trim();
    if (bytesBase64) return `data:${mime};base64,${bytesBase64}`;
  } catch (error) {
    const fallback = resolveLocalFileUrl(mediaRef);
    if (fallback) return fallback;
    console.warn("[归档音频] 读取失败", { path: mediaRef, error });
    return "";
  }
  return resolveLocalFileUrl(mediaRef);
}

function archiveImageKey(messageId: string, index: number): string {
  return `${String(messageId || "").trim()}::${index}`;
}

async function readArchiveImageDataUrl(
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
): Promise<string> {
  const mime = String(image.mime || "").trim() || "image/webp";
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  if (bytesBase64) return `data:${mime};base64,${bytesBase64}`;
  const mediaRef = String(image.mediaRef || "").trim();
  if (!mediaRef) return "";
  const cacheKey = `${mime}::${mediaRef}`;
  const cached = archiveImageDataUrlCache.get(cacheKey);
  if (cached) return cached;
  const pending = archiveImagePendingCache.get(cacheKey);
  if (pending) return pending;
  const legacyMarker = mediaRef.startsWith("@media:") || mediaRef.startsWith("@download:");
  const task = readTransportChatImage({
    ...(legacyMarker ? { mediaRef } : { path: mediaRef }),
    mime,
  })
    .then((result) => {
      const dataUrl = String(result?.dataUrl || "").trim();
      if (dataUrl) archiveImageDataUrlCache.set(cacheKey, dataUrl);
      archiveImagePendingCache.delete(cacheKey);
      return dataUrl;
    })
    .catch((error) => {
      archiveImagePendingCache.delete(cacheKey);
      throw error;
    });
  archiveImagePendingCache.set(cacheKey, task);
  return task;
}

watchEffect(() => {
  for (const message of visibleMessages.value) {
    const images = messageImages(message);
    images.forEach((image, index) => {
      const key = archiveImageKey(message.id, index);
      if (archiveResolvedImageMap.value[key]) return;
      void readArchiveImageDataUrl(image)
        .then((dataUrl) => {
          if (!dataUrl) return;
          archiveResolvedImageMap.value = {
            ...archiveResolvedImageMap.value,
            [key]: dataUrl,
          };
        })
        .catch((error) => {
          console.warn("[归档图片] 懒加载失败", {
            messageId: message.id,
            mediaRef: image.mediaRef,
            error,
          });
        });
    });
    messageAudios(message).forEach((audio, index) => {
      if (audio.bytesBase64 || !audio.mediaRef) return;
      const key = archiveAudioKey(message.id, index);
      if (archiveResolvedAudioMap.value[key]) return;
      void readArchiveAudioSource(audio).then((src) => {
        if (!src) return;
        archiveResolvedAudioMap.value = {
          ...archiveResolvedAudioMap.value,
          [key]: src,
        };
      });
    });
  }
});

function resolvedArchiveImageSrc(
  messageId: string,
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
): string {
  const direct = String(image.bytesBase64 || "").trim();
  if (direct) return `data:${image.mime};base64,${direct}`;
  return String(archiveResolvedImageMap.value[archiveImageKey(messageId, index)] || "").trim();
}
</script>

<style scoped>
.archive-meme-segment-flow {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 0.35rem 0.5rem;
  max-width: 100%;
}

.archive-inline-meme {
  display: inline-block;
  max-width: min(14rem, 100%);
  max-height: 8rem;
  border-radius: 0.75rem;
  object-fit: contain;
  vertical-align: bottom;
}

.archive-local-image-thumbnail {
  display: inline-block;
  max-width: min(28rem, 100%);
  max-height: 18rem;
  border-radius: 0.5rem;
  object-fit: contain;
  vertical-align: bottom;
}

.archive-local-image-unavailable {
  display: inline-flex;
  min-width: 5rem;
  max-width: min(16rem, 100%);
  min-height: 3rem;
  align-items: center;
  justify-content: center;
  padding: 0.35rem 0.55rem;
  border: 1px dashed currentColor;
  border-radius: 0.5rem;
  opacity: 0.45;
  font-size: var(--app-text-xs-size);
  line-height: 1.25;
}

.archive-markdown-content:deep(.markdown-renderer),
.archive-markdown-content:deep(.node-slot),
.archive-markdown-content:deep(.node-content),
.archive-markdown-content:deep(.text-node) {
  min-width: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
}

.archive-markdown-content:deep(> :first-child) {
  margin-top: 0;
}

.archive-markdown-content:deep(> :last-child) {
  margin-bottom: 0;
}

.archive-markdown-content:deep(:where(p,ul,ol,blockquote,pre,table,figure,.paragraph-node,.list-node,.blockquote,.table-node-wrapper,.code-block-container,._mermaid,.vmr-container)) {
  margin-top: 0.45rem;
  margin-bottom: 0.45rem;
}

.archive-markdown-content:deep(:where(h1,h2,h3,h4,.heading-node)) {
  margin-top: 0.65rem;
  margin-bottom: 0.45rem;
  line-height: 1.35;
}

.archive-markdown-content:deep(:where(h1,.heading-node.heading-1)) {
  font-size: var(--app-text-xl-size);
}

.archive-markdown-content:deep(:where(h2,.heading-node.heading-2)) {
  font-size: var(--app-text-lg-size);
}

.archive-markdown-content:deep(:where(h3,.heading-node.heading-3,h4,.heading-node.heading-4)) {
  font-size: var(--app-text-base-size);
}

.archive-markdown-content:deep(:where(ul,ol,.list-node)) {
  padding-left: 1.25rem;
}

.archive-markdown-content:deep(:where(a,.link-node)) {
  color: inherit;
  text-decoration: underline;
  text-underline-offset: 0.16em;
}

.archive-markdown-content:deep(:where(blockquote,.blockquote)) {
  border-left: 3px solid color-mix(in oklab, currentColor 35%, transparent);
  padding-left: 0.75rem;
  opacity: 0.88;
}

.archive-markdown-content:deep(:where(:not(pre) > code,.inline-code)) {
  border-radius: 0.25rem;
  color: var(--ecall-md-inline-code-color);
  background: transparent;
  padding: 0.08rem 0.28rem;
}

.archive-markdown-content:deep(:where(table,.table-node)) {
  width: max-content;
  min-width: 100%;
  max-width: 100%;
}
</style>
