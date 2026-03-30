<template>
  <div class="flex h-[80vh] w-[80vw] max-h-[calc(100vh-1rem)] max-w-[calc(100vw-1rem)] flex-col rounded-box border border-base-300 bg-base-100 shadow-xl">
    <div class="flex-1 min-h-0 space-y-2 overflow-y-auto p-2">
      <button
        v-for="item in props.items"
        :key="item.conversationId"
        type="button"
        class="w-full rounded-box text-left transition-colors"
        :class="[
          item.conversationId === props.activeConversationId ? 'bg-primary/10' : 'bg-base-100 hover:bg-base-200',
          isConversationItemDisabled(item) ? 'cursor-not-allowed opacity-60' : '',
        ]"
        :disabled="isConversationItemDisabled(item)"
        :title="conversationItemTitle(item)"
        @click="$emit('selectConversation', item.conversationId)"
      >
        <!-- 红色区域：最后一个发言人头像 -->
        <div class="flex items-center gap-3 p-3">
          <div class="shrink-0">
            <div class="avatar">
              <div class="w-10 h-10 rounded-full bg-error text-error-content">
                <img
                  v-if="lastSpeakerAvatarUrl(item)"
                  :src="lastSpeakerAvatarUrl(item)"
                  :alt="lastSpeakerLabel(item)"
                  class="w-10 h-10 rounded-full object-cover"
                />
                <span v-else class="flex h-10 w-10 items-center justify-center rounded-full text-sm font-bold">
                  {{ lastSpeakerInitial(item) }}
                </span>
              </div>
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <!-- 标题 + 会话类型标签 -->
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

            <!-- 蓝色区域：工作空间 + 日期 + 消息数 -->
            <div class="flex items-center gap-2 mt-1 text-xs">
              <span class="font-medium">
                {{ item.workspaceLabel || t("chat.defaultWorkspace") }}
              </span>
              <span class="text-base-content/70">{{ formatConversationTime(item.updatedAt) }}</span>
              <span class="font-medium">
                {{ t("chat.messageCount", { count: item.messageCount }) }}
              </span>
            </div>
          </div>

          <!-- 运行状态 -->
          <span v-if="item.runtimeState" class="badge badge-ghost badge-xs shrink-0">
            {{ runtimeStateText(item.runtimeState) }}
          </span>
        </div>

        <!-- 消息摘要（最多两条） -->
        <div class="px-3 pb-3 space-y-1">
          <div
            v-for="preview in normalizedPreviewMessages(item).slice(0, 2)"
            :key="preview.messageId"
            class="flex items-start gap-2 text-xs"
          >
            <span class="shrink-0 font-medium">
              {{ speakerLabel(preview) }}:
            </span>
            <span class="truncate opacity-80">{{ previewText(preview) }}</span>
          </div>
          <div v-if="normalizedPreviewMessages(item).length === 0" class="text-xs opacity-60 px-2">
            {{ t("chat.conversationNoPreview") }}
          </div>
        </div>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";

const props = defineProps<{
  items: ChatConversationOverviewItem[];
  activeConversationId: string;
  userAlias: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  userAvatarUrl: string;
}>();

defineEmits<{
  (e: "selectConversation", conversationId: string): void;
}>();

const { t, locale } = useI18n();

const conversationPreviewCache = computed(() => new Map(
  props.items.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));

function isConversationItemDisabled(item: ChatConversationOverviewItem): boolean {
  return item.runtimeState === "organizing_context";
}

function conversationItemTitle(item: ChatConversationOverviewItem): string {
  if (item.runtimeState === "organizing_context") {
    return t("chat.organizingContextDisabled");
  }
  return item.workspaceLabel || t("chat.defaultWorkspace");
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
  return date.toLocaleString(locale.value, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function lastSpeakerInitial(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "?";

  const lastPreview = previews[previews.length - 1];
  const label = speakerLabel(lastPreview);
  return label.charAt(0).toUpperCase();
}

function lastSpeakerLabel(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";

  const lastPreview = previews[previews.length - 1];
  return speakerLabel(lastPreview);
}

function lastSpeakerAvatarUrl(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";

  const lastPreview = previews[previews.length - 1];
  const speakerId = String(lastPreview.speakerAgentId || "").trim();

  if (!speakerId || speakerId === "user-persona") {
    return props.userAvatarUrl || "";
  }

  return props.personaAvatarUrlMap?.[speakerId] || "";
}
</script>
