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
        :class="viewMode === 'archive' ? 'btn-primary' : 'btn-ghost'"
        @click="switchViewMode('archive')"
      >
        {{ t("archives.archivedMessages") }}
      </button>
    </div>
    <div class="flex items-center gap-2">
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="$emit('loadArchives')">{{ t("archives.refresh") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="triggerArchiveImport">{{ t("archives.importJson") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'markdown' })">{{ t("archives.exportMarkdown") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="viewMode !== 'archive' || !selectedArchiveId" @click="$emit('exportArchive', { format: 'json' })">{{ t("archives.exportJson") }}</button>
      <button
        class="btn bg-base-100 border-base-300 hover:bg-base-200 text-error"
        :disabled="(viewMode === 'archive' && !selectedArchiveId) || (viewMode === 'current' && !selectedUnarchivedConversationId)"
        @click="viewMode === 'archive' ? onDeleteArchiveClick(selectedArchiveId) : onDeleteUnarchivedClick(selectedUnarchivedConversationId)"
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
        <div v-else class="flex flex-col gap-2">
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
      </div>
      <div class="flex-1 overflow-auto space-y-2">
        <div class="text-sm opacity-70 sticky top-0 z-10 bg-base-200/90 backdrop-blur px-1 py-1">
          {{ viewMode === "current" ? t("archives.currentUnarchived") : t("archives.archivedMessages") }}
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
            <div class="badge badge-primary badge-sm">{{ roleLabel(m.role) }}</div>
            <div class="opacity-60 text-sm">{{ formatDate(m.createdAt) }}</div>
          </div>
          <div v-if="messageText(m)" class="whitespace-pre-wrap wrap-break-word">{{ messageText(m) }}</div>
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
import type { ArchiveSummary, ChatMessage, ChatRole, MessagePart, UnarchivedConversationSummary } from "../../../types/app";

const props = defineProps<{
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
  archiveSummaryText: string;
  unarchivedConversations: UnarchivedConversationSummary[];
  selectedUnarchivedConversationId: string;
  unarchivedMessages: ChatMessage[];
}>();
const { t, locale } = useI18n();

const emit = defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
  (e: "selectUnarchivedConversation", conversationId: string): void;
  (e: "exportArchive", payload: { format: "markdown" | "json" }): void;
  (e: "deleteArchive", archiveId: string): void;
  (e: "deleteUnarchivedConversation", conversationId: string): void;
  (e: "importArchiveFile", file: File): void;
}>();

const viewMode = ref<"current" | "archive">("archive");

const visibleMessages = computed(() =>
  (viewMode.value === "current" ? props.unarchivedMessages : props.archiveMessages)
    .filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool"),
);
const archiveImportInputRef = ref<HTMLInputElement | null>(null);

function switchViewMode(mode: "current" | "archive") {
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

function messageText(msg: ChatMessage): string {
  return msg.parts
    .filter((p): p is Extract<MessagePart, { type: "text" }> => p.type === "text")
    .map((p) => p.text)
    .join("\n")
    .trim();
}

function roleLabel(role: ChatRole): string {
  if (role === "user") return t("archives.roleUser");
  if (role === "assistant") return t("archives.roleAssistant");
  if (role === "tool") return t("archives.roleTool");
  return role;
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
