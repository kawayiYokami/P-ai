<template>
  <div class="grid gap-2">
    <div class="flex items-center gap-2">
      <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200" @click="$emit('loadArchives')">{{ t("archives.refresh") }}</button>
      <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200" :disabled="!selectedArchiveId" @click="$emit('exportArchive', { format: 'markdown' })">{{ t("archives.exportMarkdown") }}</button>
      <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200" :disabled="!selectedArchiveId" @click="$emit('exportArchive', { format: 'json' })">{{ t("archives.exportJson") }}</button>
    </div>
    <div class="grid grid-cols-1 gap-1 max-h-56 overflow-auto">
      <div
        v-for="a in archives"
        :key="a.archiveId"
        class="flex items-center gap-1"
      >
        <button
          class="btn btn-sm justify-start flex-1 min-w-0 bg-base-100 border-base-300 hover:bg-base-200"
          :class="{ 'btn-active': a.archiveId === selectedArchiveId }"
          @click="$emit('selectArchive', a.archiveId)"
        >
          <span class="truncate">{{ a.title }} <span v-if="a.messageCount">({{ a.messageCount }})</span></span>
        </button>
        <button
          class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 text-error"
          :title="t('archives.deleteTitle')"
          @click="$emit('deleteArchive', a.archiveId)"
        >
          {{ t("archives.delete") }}
        </button>
      </div>
    </div>
    <div class="divider my-1">{{ t("archives.contentDivider") }}</div>
    <div class="max-h-80 overflow-auto space-y-2">
      <div v-for="m in visibleMessages" :key="m.id" class="text-xs border border-base-300 rounded p-2 bg-base-100">
        <div class="flex items-center justify-between mb-1">
          <div class="font-semibold">{{ roleLabel(m.role) }}</div>
          <div class="opacity-60">{{ formatDate(m.createdAt) }}</div>
        </div>
        <div v-if="messageText(m)" class="whitespace-pre-wrap break-words">{{ messageText(m) }}</div>
        <div v-if="toolSummaries(m).length > 0" class="mt-2 space-y-1">
          <details v-for="(tool, idx) in toolSummaries(m)" :key="`${m.id}-tool-${idx}`" class="collapse collapse-arrow border border-base-300 bg-base-200">
            <summary class="collapse-title py-2 px-3 min-h-0 text-xs">{{ t("archives.toolCall", { name: tool.name }) }}</summary>
            <div class="collapse-content px-3 pb-2">
              <div class="whitespace-pre-wrap break-words text-xs opacity-80">{{ tool.content }}</div>
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
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ArchiveSummary, ChatMessage, MessagePart } from "../types/app";

const props = defineProps<{
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
  renderMessage: (msg: ChatMessage) => string;
}>();
const { t, locale } = useI18n();

defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
  (e: "exportArchive", payload: { format: "markdown" | "json" }): void;
  (e: "deleteArchive", archiveId: string): void;
}>();

const visibleMessages = computed(() =>
  props.archiveMessages.filter((m) => m.role === "user" || m.role === "assistant" || m.role === "tool"),
);

function messageText(msg: ChatMessage): string {
  return msg.parts
    .filter((p): p is Extract<MessagePart, { type: "text" }> => p.type === "text")
    .map((p) => p.text)
    .join("\n")
    .trim();
}

function roleLabel(role: string): string {
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
      const role = typeof item.role === "string" ? item.role : "";
      if (role !== "assistant") return null;
      const calls = Array.isArray(item.tool_calls) ? item.tool_calls : [];
      const first = calls[0] as Record<string, unknown> | undefined;
      const fn = (first?.function ?? {}) as Record<string, unknown>;
      const name = typeof fn.name === "string" ? fn.name : "unknown";
      const args = typeof fn.arguments === "string" ? fn.arguments : "";
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
