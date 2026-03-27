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
            class="p-2 rounded cursor-pointer hover:bg-base-200"
            :class="{ 'bg-primary/10': c.conversationId === selectedUnarchivedConversationId }"
            @click="$emit('selectUnarchivedConversation', c.conversationId)"
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
          <div v-if="messageText(m)" class="whitespace-pre-wrap wrap-break-word">{{ messageText(m) }}</div>
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
              :src="`data:${img.mime};base64,${img.bytesBase64}`"
              class="rounded max-h-32 object-contain bg-base-100/40 border border-base-300"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";
import type {
  ArchiveSummary,
  ChatMessage,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  MessagePart,
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

function onDeleteArchiveClick(archiveId: string) {
  if (!archiveId) return;
  if (!window.confirm(t("archives.deleteConfirm"))) return;
  emit("deleteArchive", archiveId);
}

function onDeleteUnarchivedClick(conversationId: string) {
  if (!conversationId) return;
  if (!window.confirm(t("archives.deleteUnarchivedConfirm"))) return;
  emit("deleteUnarchivedConversation", conversationId);
}

function onDeleteRemoteImContactClick(contactId: string) {
  if (!contactId) return;
  if (!window.confirm(t("archives.deleteUnarchivedConfirm"))) return;
  emit("deleteRemoteImContactConversation", contactId);
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
  const entries = Array.isArray(msg.toolCall) ? msg.toolCall : [];
  return entries
    .map((item) => {
      if (item.role !== "assistant") return null;
      const first = item.tool_calls?.[0];
      const name = first?.function?.name || "unknown";
      const args = first?.function?.arguments || "";
      return {
        name,
        content: args ? t("archives.toolArgs", { value: args }) : t("archives.toolNoArgs"),
      };
    })
    .filter((v): v is { name: string; content: string } => !!v);
}

function messageImages(msg: ChatMessage): Array<{ mime: string; bytesBase64: string }> {
  return msg.parts
    .filter((p): p is Extract<MessagePart, { type: "image" }> => p.type === "image")
    .map((p) => ({ mime: p.mime, bytesBase64: p.bytesBase64 }));
}
</script>
