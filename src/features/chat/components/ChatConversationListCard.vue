<template>
  <div class="flex h-[80vh] w-[80vw] max-h-[calc(100vh-1rem)] max-w-[calc(100vw-1rem)] flex-col rounded-box border border-base-300 bg-base-100 shadow-xl">
    <div class="flex-1 min-h-0 space-y-2 overflow-y-auto p-2">
      <component
        v-for="item in props.items"
        :key="item.conversationId"
        :is="isCurrentConversation(item) ? 'div' : 'button'"
        :type="isCurrentConversation(item) ? undefined : 'button'"
        class="w-full rounded-box text-left transition-colors"
        :class="[
          item.conversationId === props.activeConversationId ? 'bg-base-300' : 'bg-base-100 hover:bg-base-200',
          isConversationItemDisabled(item) ? 'cursor-not-allowed opacity-60' : '',
        ]"
        :disabled="!isCurrentConversation(item) && isConversationItemDisabled(item)"
        :title="conversationItemTitle(item)"
        @click="handleConversationCardClick(item)"
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
            <div class="flex items-start justify-between gap-2">
              <div class="flex min-w-0 items-center gap-2">
                <input
                  v-if="isEditingTitle(item)"
                  :ref="setRenameInputRef"
                  v-model="editingTitleDraft"
                  type="text"
                  class="input input-bordered input-sm h-8 min-h-0 w-full max-w-full text-sm font-medium"
                  @click.stop
                  @mousedown.stop
                  @keydown.enter.prevent="commitConversationTitleEdit(item)"
                  @keydown.esc.prevent="cancelConversationTitleEdit()"
                  @blur="handleConversationTitleBlur(item)"
                />
                <button
                  v-else-if="canRenameConversation(item)"
                  type="button"
                  class="truncate text-left text-sm font-medium hover:underline"
                  @click.stop="startConversationTitleEdit(item)"
                  @mousedown.stop
                >
                  {{ conversationDisplayTitle(item) }}
                </button>
                <div v-else class="truncate text-sm font-medium">
                  {{ conversationDisplayTitle(item) }}
                </div>
              </div>
              <span class="shrink-0 text-[11px] text-base-content/60">
                {{ formatConversationTime(item.updatedAt) }}
              </span>
            </div>

            <div class="mt-1 flex items-center justify-between gap-2 text-xs">
              <span class="min-w-0 truncate font-medium">
                {{ workspaceDepartmentLabel(item) }}
              </span>
              <div class="flex shrink-0 items-center gap-2">
                <span v-if="item.runtimeState" class="text-[11px] text-base-content/60">
                  {{ runtimeStateText(item.runtimeState) }}
                </span>
                <span
                  v-if="unreadCountBadge(item)"
                  class="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-base-300 px-1.5 text-[11px] font-medium text-base-content/80"
                >
                  {{ unreadCountBadge(item) }}
                </span>
              </div>
            </div>
          </div>
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
      </component>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import { formatConversationListTime } from "../utils/conversation-time";

const props = defineProps<{
  items: ChatConversationOverviewItem[];
  activeConversationId: string;
  userAlias: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  userAvatarUrl: string;
}>();

const emit = defineEmits<{
  (e: "selectConversation", conversationId: string): void;
  (e: "renameConversation", payload: { conversationId: string; title: string }): void;
}>();

const { t, locale } = useI18n();
const renameInputRef = ref<HTMLInputElement | null>(null);
const editingConversationId = ref("");
const editingTitleDraft = ref("");

const conversationPreviewCache = computed(() => new Map(
  props.items.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));

watchEffect(() => {
  const editingId = String(editingConversationId.value || "").trim();
  if (!editingId) return;
  const item = props.items.find((entry) => String(entry.conversationId || "").trim() === editingId);
  if (!item || !canRenameConversation(item)) {
    resetConversationTitleEdit();
  }
});

function resetConversationTitleEdit() {
  editingConversationId.value = "";
  editingTitleDraft.value = "";
}

function setRenameInputRef(element: Element | { $el?: Element | null } | null) {
  renameInputRef.value = element instanceof HTMLInputElement ? element : null;
}

function isConversationItemDisabled(item: ChatConversationOverviewItem): boolean {
  return item.runtimeState === "organizing_context";
}

function isCurrentConversation(item: ChatConversationOverviewItem): boolean {
  return String(item.conversationId || "").trim() === String(props.activeConversationId || "").trim();
}

function canRenameConversation(item: ChatConversationOverviewItem): boolean {
  return isCurrentConversation(item) && !item.isMainConversation && !isConversationItemDisabled(item);
}

function isEditingTitle(item: ChatConversationOverviewItem): boolean {
  return String(item.conversationId || "").trim() === String(editingConversationId.value || "").trim();
}

function conversationItemTitle(item: ChatConversationOverviewItem): string {
  if (item.runtimeState === "organizing_context") {
    return t("chat.organizingContextDisabled");
  }
  return item.workspaceLabel || t("chat.defaultWorkspace");
}

function workspaceDepartmentLabel(item: ChatConversationOverviewItem): string {
  const workspaceLabel = String(item.workspaceLabel || "").trim() || t("chat.defaultWorkspace");
  const departmentName = String(item.departmentName || "").trim();
  if (!departmentName) return workspaceLabel;
  return `${workspaceLabel}（${departmentName}）`;
}

function handleConversationCardClick(item: ChatConversationOverviewItem) {
  if (isCurrentConversation(item) || isConversationItemDisabled(item)) return;
  emit("selectConversation", item.conversationId);
}

function conversationDisplayTitle(item: ChatConversationOverviewItem): string {
  if (item.isMainConversation) return t("chat.mainConversation");
  return item.title || t("chat.untitledConversation");
}

async function startConversationTitleEdit(item: ChatConversationOverviewItem) {
  if (!canRenameConversation(item)) return;
  editingConversationId.value = String(item.conversationId || "").trim();
  editingTitleDraft.value = String(item.title || "").trim();
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

function cancelConversationTitleEdit() {
  resetConversationTitleEdit();
}

function commitConversationTitleEdit(item: ChatConversationOverviewItem) {
  if (!isEditingTitle(item)) return;
  const conversationId = String(item.conversationId || "").trim();
  const currentTitle = String(item.title || "").trim();
  const nextTitle = String(editingTitleDraft.value || "").trim();
  if (!conversationId || !nextTitle || nextTitle === currentTitle) {
    resetConversationTitleEdit();
    return;
  }
  resetConversationTitleEdit();
  emit("renameConversation", {
    conversationId,
    title: nextTitle,
  });
}

function handleConversationTitleBlur(item: ChatConversationOverviewItem) {
  commitConversationTitleEdit(item);
}

function unreadCountBadge(item: ChatConversationOverviewItem): string {
  if (String(item.conversationId || "").trim() === String(props.activeConversationId || "").trim()) {
    return "";
  }
  const unreadCount = Math.max(0, Number(item.unreadCount || 0));
  if (unreadCount <= 0) return "";
  return unreadCount > 99 ? "99+" : String(unreadCount);
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
  return formatConversationListTime(value, locale.value);
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
