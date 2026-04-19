<template>
  <div class="flex flex-col gap-3 h-full">
    <div class="join w-fit">
      <button
        type="button"
        class="btn btn-sm join-item"
        :class="viewMode === 'current' ? 'btn-primary' : 'btn-ghost'"
        @click="switchViewMode('current')"
      >
        {{ t("archives.currentUnarchived") }}
      </button>
      <button
        type="button"
        class="btn btn-sm join-item"
        :class="viewMode === 'delegate' ? 'btn-primary' : 'btn-ghost'"
        @click="switchViewMode('delegate')"
      >
        {{ t("archives.delegateConversations") }}
      </button>
      <button
        type="button"
        class="btn btn-sm join-item"
        :class="viewMode === 'archive' ? 'btn-primary' : 'btn-ghost'"
        @click="switchViewMode('archive')"
      >
        {{ t("archives.archivedMessages") }}
      </button>
      <button
        type="button"
        class="btn btn-sm join-item"
        :class="viewMode === 'remoteIm' ? 'btn-primary' : 'btn-ghost'"
        @click="switchViewMode('remoteIm')"
      >
        联系人消息
      </button>
    </div>
    <div class="flex items-center gap-2">
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="$emit('loadArchives')">{{ t("archives.refresh") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="triggerArchiveImport">{{ t("archives.importJson") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'markdown' })">{{ t("archives.exportMarkdown") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'json' })">{{ t("archives.exportJson") }}</button>
      <button
        class="btn bg-base-100 border-base-300 hover:bg-base-200 text-error"
        :disabled="viewMode === 'delegate' || (viewMode === 'archive' && !selectedArchiveId) || (viewMode === 'current' && !selectedUnarchivedConversationId) || (viewMode === 'remoteIm' && !selectedRemoteImContactId)"
        @click="viewMode === 'archive' ? onDeleteArchiveClick(selectedArchiveId) : viewMode === 'remoteIm' ? onDeleteRemoteImContactClick(selectedRemoteImContactId) : onDeleteUnarchivedClick(selectedUnarchivedConversationId)"
      >
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
    <div class="flex gap-3 flex-1 min-h-0">
      <div class="w-56 overflow-auto">
        <div v-if="viewMode === 'current'" class="flex flex-col gap-2">
          <div
            v-for="c in unarchivedConversations"
            :key="c.conversationId"
            class="p-2 rounded"
            :class="[
              c.conversationId === selectedUnarchivedConversationId ? 'bg-primary/10' : '',
              c.runtimeState === 'organizing_context' ? 'cursor-not-allowed opacity-60' : 'cursor-pointer hover:bg-base-200',
            ]"
            :title="c.runtimeState === 'organizing_context' ? t('archives.organizingContextDisabled') : ''"
            @click="c.runtimeState === 'organizing_context' ? undefined : $emit('selectUnarchivedConversation', c.conversationId)"
          >
            <div class="font-medium truncate text-sm">{{ c.title }}</div>
            <div class="text-sm opacity-70 truncate">{{ formatDate(c.lastMessageAt || c.updatedAt) }}</div>
          </div>
        </div>
        <div v-else-if="viewMode === 'archive'" class="flex flex-col gap-2">
          <div
            v-for="a in archives"
            :key="a.archiveId"
            class="p-2 rounded cursor-pointer hover:bg-base-200"
            :class="{ 'bg-primary/10': a.archiveId === selectedArchiveId }"
            @click="$emit('selectArchive', a.archiveId)"
          >
            <div class="font-medium truncate text-sm">{{ a.title }}</div>
            <div v-if="a.archivedAt" class="text-sm opacity-70 truncate">{{ formatDate(a.archivedAt) }}</div>
          </div>
        </div>
        <div v-else-if="viewMode === 'delegate'" class="flex flex-col gap-2">
          <div
            v-for="c in delegateConversations"
            :key="c.conversationId"
            class="p-2 rounded cursor-pointer hover:bg-base-200"
            :class="{ 'bg-primary/10': c.conversationId === selectedDelegateConversationId }"
            @click="$emit('selectDelegateConversation', c.conversationId)"
          >
            <div class="font-medium truncate text-sm">{{ c.title }}</div>
            <div class="text-sm opacity-70 truncate">{{ formatDate(c.archivedAt || c.lastMessageAt || c.updatedAt) }}</div>
          </div>
        </div>
        <div v-else class="flex flex-col gap-2">
          <div
            v-for="c in remoteImContactConversations"
            :key="c.contactId"
            class="p-2 rounded cursor-pointer hover:bg-base-200"
            :class="{ 'bg-primary/10': c.contactId === selectedRemoteImContactId }"
            @click="$emit('selectRemoteImContactConversation', c.contactId)"
          >
            <div class="font-medium truncate text-sm">{{ c.contactDisplayName }}</div>
            <div class="text-xs opacity-70 truncate">{{ c.boundDepartmentId || "-" }} · {{ c.processingMode }}</div>
            <div class="text-sm opacity-70 truncate">{{ formatDate(c.lastMessageAt || c.updatedAt) }}</div>
          </div>
        </div>
      </div>
      <div class="flex-1 overflow-auto space-y-2">
        <div class="text-sm opacity-70 sticky top-0 z-10 bg-base-200/90 backdrop-blur px-1 py-1">
          {{ viewMode === "current" ? t("archives.currentUnarchived") : viewMode === "delegate" ? t("archives.delegateConversations") : viewMode === "archive" ? t("archives.archivedMessages") : "联系人消息" }}
        </div>
        <div
          v-if="viewMode === 'archive' && archiveSummaryText"
          class="border border-primary/20 rounded p-3 bg-primary/5"
        >
          <div class="text-sm opacity-70 mb-1">{{ t("archives.summary") }}</div>
          <div class="whitespace-pre-wrap wrap-break-word text-sm">{{ archiveSummaryText }}</div>
        </div>
        <div v-for="m in visibleMessages" :key="m.id" class="border border-base-300 rounded p-3 bg-base-100">
          <div class="flex items-center justify-between mb-1">
            <div class="badge badge-primary badge-sm">{{ speakerLabel(m) }}</div>
            <div class="opacity-60 text-sm">{{ formatDate(m.createdAt) }}</div>
          </div>
          <div
            v-if="messageMemeSegments(m).length > 0"
            class="archive-meme-segment-flow"
          >
            <template v-for="(segment, index) in messageMemeSegments(m)" :key="`${m.id}-meme-${index}`">
              <span
                v-if="segment.type === 'text' && archiveMemeText(segment)"
                class="archive-meme-segment-text"
              >{{ archiveMemeText(segment) }}</span>
              <img
                v-else-if="segment.type === 'meme'"
                :src="archiveMemeSegmentDataUrl(segment)"
                :alt="archiveMemeName(segment)"
                :title="`:${archiveMemeName(segment)}:`"
                class="archive-inline-meme"
              />
            </template>
          </div>
          <div v-else-if="messageText(m)" class="whitespace-pre-wrap wrap-break-word">{{ messageText(m) }}</div>
          <div
            v-if="messageAttachments(m).length > 0"
            class="mt-2 flex flex-wrap gap-2"
          >
            <button
              v-for="(file, idx) in messageAttachments(m)"
              :key="`${m.id}-attachment-${idx}`"
              type="button"
              class="link link-primary text-sm"
              :title="file.relativePath"
              @click="openAttachment(file.relativePath)"
            >
              {{ file.fileName }}
            </button>
          </div>
          <div v-if="toolSummaries(m).length > 0" class="mt-2 space-y-1">
            <details v-for="(tool, idx) in toolSummaries(m)" :key="`${m.id}-tool-${idx}`" class="collapse collapse-arrow border border-base-300 bg-base-200">
              <summary class="collapse-title py-2 px-3 min-h-0 text-sm font-medium">{{ t("archives.toolCall", { name: tool.name }) }}</summary>
              <div class="collapse-content px-3 pb-2">
                <div class="whitespace-pre-wrap wrap-break-word text-sm opacity-80">{{ tool.content }}</div>
              </div>
            </details>
          </div>
          <div v-if="messageImages(m).length > 0" class="mt-2 grid gap-1">
            <img
              v-for="(img, idx) in messageImages(m)"
              :key="`${img.mime}-${idx}`"
              :src="resolvedArchiveImageSrc(m.id, img, idx)"
              class="rounded max-h-32 object-contain bg-base-100/40 border border-base-300"
            />
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
import { computed, onBeforeUnmount, onMounted, ref, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";
import { summarizeToolActivityForDisplay } from "../../../utils/chat-message-semantics";
import type {
  ArchiveSummary,
  ChatMessage,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  MessagePart,
  MemeMessageSegment,
  UnarchivedConversationSummary,
} from "../../../types/app";

const props = defineProps<{
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
  archiveSummaryText: string;
  unarchivedConversations: UnarchivedConversationSummary[];
  selectedUnarchivedConversationId: string;
  unarchivedMessages: ChatMessage[];
  delegateConversations: DelegateConversationSummary[];
  selectedDelegateConversationId: string;
  delegateMessages: ChatMessage[];
  remoteImContactConversations: RemoteImContactConversationSummary[];
  selectedRemoteImContactId: string;
  remoteImContactMessages: ChatMessage[];
  personaNameMap?: Record<string, string>;
}>();
const { t, locale } = useI18n();

const emit = defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
  (e: "selectUnarchivedConversation", conversationId: string): void;
  (e: "selectDelegateConversation", conversationId: string): void;
  (e: "selectRemoteImContactConversation", contactId: string): void;
  (e: "exportArchive", payload: { format: "markdown" | "json" }): void;
  (e: "deleteArchive", archiveId: string): void;
  (e: "deleteUnarchivedConversation", conversationId: string): void;
  (e: "deleteRemoteImContactConversation", contactId: string): void;
  (e: "importArchiveFile", file: File): void;
}>();

const viewMode = ref<"current" | "delegate" | "archive" | "remoteIm">("archive");
const ARCHIVE_FOCUS_REQUEST_STORAGE_KEY = "easy_call.archives.focus_request.v1";
const ARCHIVE_FOCUS_REQUEST_TTL_MS = 30_000;
const archiveImageDataUrlCache = new Map<string, string>();
const archiveImagePendingCache = new Map<string, Promise<string>>();
const archiveResolvedImageMap = ref<Record<string, string>>({});
const confirmDialog = ref<HTMLDialogElement | null>(null);
const confirmDialogState = ref({
  title: "",
  message: "",
});
let resolveConfirmDialog: ((value: boolean) => void) | null = null;

const visibleMessages = computed(() =>
  viewMode.value === "current"
    ? props.unarchivedMessages.filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool")
    : viewMode.value === "delegate"
      ? props.delegateMessages.filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool")
      : viewMode.value === "remoteIm"
        ? props.remoteImContactMessages.filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool")
        : props.archiveMessages,
);
const archiveImportInputRef = ref<HTMLInputElement | null>(null);

function switchViewMode(mode: "current" | "delegate" | "archive" | "remoteIm") {
  viewMode.value = mode;
}

function readPendingArchiveFocusRequest(): { conversationId: string; viewMode: "current" } | null {
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
    if (requestViewMode !== "current") {
      window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
      return null;
    }
    return {
      conversationId,
      viewMode: "current",
    };
  } catch {
    window.localStorage.removeItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY);
    return null;
  }
}

function applyPendingArchiveFocusRequest() {
  const pending = readPendingArchiveFocusRequest();
  if (!pending) return false;
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
  () => props.unarchivedConversations,
  () => {
    applyPendingArchiveFocusRequest();
  },
  { immediate: true },
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

function messageMemeSegments(msg: ChatMessage): MemeMessageSegment[] {
  const raw = Array.isArray(msg.providerMeta?.memeSegments) ? msg.providerMeta.memeSegments : [];
  return raw
    .map((item) => {
      if (!item || typeof item !== "object") return undefined;
      const segment = item as Record<string, unknown>;
      const type = String(segment.type || "").trim().toLowerCase();
      if (type === "text") {
        return {
          type: "text",
          text: String(segment.text || ""),
        } satisfies MemeMessageSegment;
      }
      if (type === "meme") {
        const name = String(segment.name || "").trim();
        const category = String(segment.category || "").trim();
        const mime = String(segment.mime || "").trim();
        const relativePath = String(segment.relativePath || "").trim();
        const bytesBase64 = String(segment.bytesBase64 || "").trim();
        if (!name || !category || !mime || !relativePath || !bytesBase64) return undefined;
        return {
          type: "meme",
          name,
          category,
          mime,
          relativePath,
          bytesBase64,
        } satisfies MemeMessageSegment;
      }
      return undefined;
    })
    .filter((item): item is MemeMessageSegment => !!item);
}

function archiveMemeSegmentDataUrl(segment: MemeMessageSegment): string {
  if (segment.type !== "meme") return "";
  return `data:${segment.mime};base64,${segment.bytesBase64}`;
}

function archiveMemeText(segment: MemeMessageSegment): string {
  return segment.type === "text" ? segment.text : "";
}

function archiveMemeName(segment: MemeMessageSegment): string {
  return segment.type === "meme" ? segment.category : "";
}

function speakerLabel(msg: ChatMessage): string {
  const speakerId = String(msg.speakerAgentId || "").trim();
  if (speakerId) {
    return props.personaNameMap?.[speakerId] || speakerId;
  }
  if (msg.role === "tool") return t("archives.roleTool");
  return String(msg.role || "").trim() || "-";
}

function messageAttachments(msg: ChatMessage): Array<{ fileName: string; relativePath: string; mime?: string }> {
  const raw = Array.isArray(msg.providerMeta?.attachments) ? msg.providerMeta?.attachments : [];
  return raw
    .map((item) => {
      const fileName = String(item?.fileName || "").trim();
      const relativePath = String(item?.relativePath || "").trim().replace(/\\/g, "/");
      const mime = typeof item?.mime === "string" ? item.mime.trim() : "";
      if (!fileName || !relativePath) return undefined;
      return { fileName, relativePath, mime: mime || undefined };
    })
    .filter((item): item is NonNullable<typeof item> => !!item);
}

function openAttachment(relativePath: string) {
  if (!relativePath.trim()) return;
  void invokeTauri("open_workspace_file", { relativePath });
}

function formatDate(value?: string): string {
  if (!value) return "-";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString(locale.value);
}

function toolSummaries(msg: ChatMessage): Array<{ name: string; content: string }> {
  return summarizeToolActivityForDisplay(msg).calls.map((tool) => ({
    name: tool.name,
    content: tool.argsText && tool.argsText !== "{}"
      ? t("archives.toolArgs", { value: tool.argsText })
      : t("archives.toolNoArgs"),
  }));
}

function messageImages(msg: ChatMessage): Array<{ mime: string; bytesBase64?: string; mediaRef?: string }> {
  return msg.parts
    .filter((p): p is Extract<MessagePart, { type: "image" }> => p.type === "image")
    .map((p) => ({
      mime: String(p.mime || "").trim(),
      bytesBase64: (() => {
        const raw = String((p as { bytesBase64?: unknown }).bytesBase64 ?? "").trim();
        return raw && !raw.startsWith("@media:") ? raw : undefined;
      })(),
      mediaRef: (() => {
        const raw = String((p as { bytesBase64?: unknown }).bytesBase64 ?? "").trim();
        return raw.startsWith("@media:") ? raw : undefined;
      })(),
    }))
    .filter((item) =>
      item.mime.length > 0
      && (
        (!!item.bytesBase64 && item.bytesBase64 !== "undefined" && item.bytesBase64 !== "null")
        || !!item.mediaRef
      ),
    );
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
  const task = invokeTauri<{ dataUrl: string }>("read_chat_image_data_url", {
    input: { mediaRef, mime },
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

.archive-meme-segment-text {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.archive-inline-meme {
  display: inline-block;
  max-width: min(14rem, 100%);
  max-height: 8rem;
  border-radius: 0.75rem;
  object-fit: contain;
  vertical-align: bottom;
}
</style>
