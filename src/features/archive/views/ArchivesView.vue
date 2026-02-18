<template>
  <div class="flex flex-col gap-3 h-full">
    <div class="flex items-center gap-2">
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="$emit('loadArchives')">{{ t("archives.refresh") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" @click="triggerArchiveImport">{{ t("archives.importJson") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="!selectedArchiveId" @click="$emit('exportArchive', { format: 'markdown' })">{{ t("archives.exportMarkdown") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200" :disabled="!selectedArchiveId" @click="$emit('exportArchive', { format: 'json' })">{{ t("archives.exportJson") }}</button>
      <button class="btn bg-base-100 border-base-300 hover:bg-base-200 text-error" :disabled="!selectedArchiveId" @click="onDeleteClick(selectedArchiveId)">{{ t("common.delete") }}</button>
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
        <div class="flex flex-col gap-2">
          <div
            v-for="a in archives"
            :key="a.archiveId"
            class="p-2 rounded cursor-pointer hover:bg-base-200"
            :class="{ 'bg-primary/10': a.archiveId === selectedArchiveId }"
            @click="$emit('selectArchive', a.archiveId)"
          >
            <div class="font-medium truncate text-sm">{{ a.title }}</div>
            <div v-if="a.archivedAt" class="text-xs opacity-70 truncate">{{ formatDate(a.archivedAt) }}</div>
          </div>
        </div>
      </div>
      <div class="flex-1 overflow-auto space-y-2">
        <div v-for="m in visibleMessages" :key="m.id" class="border border-base-300 rounded p-3 bg-base-100">
          <div class="flex items-center justify-between mb-1">
            <div class="badge badge-primary badge-sm">{{ roleLabel(m.role) }}</div>
            <div class="opacity-60 text-xs">{{ formatDate(m.createdAt) }}</div>
          </div>
          <div v-if="messageText(m)" class="whitespace-pre-wrap break-words">{{ messageText(m) }}</div>
          <div v-if="toolSummaries(m).length > 0" class="mt-2 space-y-1">
            <details v-for="(tool, idx) in toolSummaries(m)" :key="`${m.id}-tool-${idx}`" class="collapse collapse-arrow border border-base-300 bg-base-200">
              <summary class="collapse-title py-2 px-3 min-h-0 text-sm font-medium">{{ t("archives.toolCall", { name: tool.name }) }}</summary>
              <div class="collapse-content px-3 pb-2">
                <div class="whitespace-pre-wrap break-words text-sm opacity-80">{{ tool.content }}</div>
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
import type { ArchiveSummary, ChatMessage, ChatRole, MessagePart } from "../../../types/app";

const props = defineProps<{
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
}>();
const { t, locale } = useI18n();

const emit = defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
  (e: "exportArchive", payload: { format: "markdown" | "json" }): void;
  (e: "deleteArchive", archiveId: string): void;
  (e: "importArchiveFile", file: File): void;
}>();

const visibleMessages = computed(() =>
  props.archiveMessages.filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool"),
);
const archiveImportInputRef = ref<HTMLInputElement | null>(null);

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

function onDeleteClick(archiveId: string) {
  if (!archiveId) return;
  if (!window.confirm(t("archives.deleteConfirm"))) return;
  emit("deleteArchive", archiveId);
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
