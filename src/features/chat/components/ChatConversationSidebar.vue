<template>
  <aside class="flex h-full w-88 shrink-0 flex-col border-r border-base-300 bg-base-200">
    <ChatConversationListHeader
      v-model:search-query="conversationSearchQuery"
      v-model:active-tab="activeConversationTab"
      v-model:compact-list="compactConversationList"
      :search-placeholder="t('chat.conversationSearchPlaceholder')"
      :local-label="t('chat.localConversationTab')"
      :contact-label="t('chat.contactConversationTab')"
    />
    <div class="flex-1 min-h-0 overflow-y-auto p-2 pt-2">
      <section
        v-for="section in filteredConversationSections"
        :key="section.key"
        class="mb-2 last:mb-0"
      >
        <div class="divider my-1 px-1 text-[11px] font-semibold uppercase tracking-wide text-base-content/45 before:bg-base-300 after:bg-base-300">
          {{ section.title }}
        </div>
        <div class="space-y-1">
          <component
            v-for="item in section.items"
            :key="item.conversationId"
            :is="isCurrentConversation(item) ? 'div' : 'button'"
            :type="isCurrentConversation(item) ? undefined : 'button'"
            class="block w-full rounded-box text-left transition-colors hover:bg-base-100"
            :class="[
              item.conversationId === activeConversationId ? 'bg-base-300 hover:bg-base-300' : '',
              isConversationDisabled(item) ? 'cursor-not-allowed opacity-60' : '',
            ]"
            :disabled="!isCurrentConversation(item) && isConversationDisabled(item)"
            :title="conversationItemTitle(item)"
            @click="handleConversationCardClick(item)"
          >
            <div class="flex items-center gap-2 p-2">
              <div class="shrink-0">
                <div class="indicator">
                  <span
                    v-if="conversationIndicatorTone(item)"
                    class="indicator-item indicator-top indicator-end z-10 h-2.5 w-2.5 translate-x-0.5 -translate-y-0.5 rounded-full"
                    :class="conversationIndicatorClass(conversationIndicatorTone(item))"
                    aria-hidden="true"
                  ></span>
                  <div class="avatar">
                    <div class="flex h-10 w-10 items-center justify-center rounded-full bg-neutral text-neutral-content">
                      <img
                        v-if="sideListLastSpeakerAvatarUrl(item)"
                        :src="sideListLastSpeakerAvatarUrl(item)"
                        :alt="sideListLastSpeakerLabel(item)"
                        class="w-10 h-10 rounded-full object-cover"
                      />
                      <span v-else class="text-sm font-bold">{{ sideListLastSpeakerInitial(item) }}</span>
                    </div>
                  </div>
                </div>
              </div>

              <div class="flex-1 min-w-0">
                <div class="flex items-start justify-between gap-1.5">
                  <div class="flex min-w-0 items-center gap-1.5">
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
                      class="min-w-0 truncate text-left text-sm font-medium hover:underline"
                      @click.stop="startConversationTitleEdit(item)"
                      @mousedown.stop
                    >
                      {{ conversationDisplayTitle(item) }}
                    </button>
                    <div v-else class="min-w-0 truncate text-sm font-medium">
                      {{ conversationDisplayTitle(item) }}
                    </div>
                  </div>
                  <div class="flex shrink-0 items-center gap-1">
                    <span class="text-[11px] text-base-content/60">
                      {{ formatConversationTime(item.updatedAt) }}
                    </span>
                    <button
                      v-if="!item.isMainConversation"
                      type="button"
                      class="btn btn-ghost btn-xs h-6 min-h-6 w-6 min-w-6 p-0 text-base-content/45 hover:text-base-content"
                      :title="pinConversationTitle(item)"
                      :disabled="!canToggleConversationPin(item)"
                      @click.stop="toggleConversationPin(item)"
                      @mousedown.stop
                    >
                      <PinOff
                        v-if="item.isPinned || item.isMainConversation"
                        class="h-3.5 w-3.5"
                      />
                      <Pin
                        v-else
                        class="h-3.5 w-3.5"
                      />
                    </button>
                  </div>
                </div>

                <div class="mt-1 flex items-center justify-between gap-2 text-xs">
                  <span class="min-w-0 truncate" :class="compactConversationList ? 'opacity-60' : 'font-medium'">
                    {{ compactConversationList ? latestPreviewLine(item) : workspaceDepartmentLabel(item) }}
                  </span>
                  <div class="flex shrink-0 items-center gap-2">
                    <span v-if="item.runtimeState" class="text-[11px] text-base-content/60">{{ runtimeStateText(item.runtimeState) }}</span>
                    <span
                      v-if="unreadCountBadge(item)"
                      class="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-error px-1.5 text-[11px] font-medium text-error-content"
                    >
                      {{ unreadCountBadge(item) }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <div v-if="!compactConversationList" class="space-y-1 px-2 pb-2">
              <div
                v-for="preview in normalizedPreviewMessages(item).slice(0, 2)"
                :key="preview.messageId"
                class="flex items-start gap-1.5 text-xs"
              >
                <span class="shrink-0 font-medium">{{ speakerLabel(preview) }}:</span>
                <span class="truncate opacity-80">{{ previewText(preview) }}</span>
              </div>
              <div v-if="normalizedPreviewMessages(item).length === 0" class="px-1 text-xs opacity-60">
                {{ t("chat.conversationNoPreview") }}
              </div>
            </div>
          </component>
        </div>
      </section>
      <div
        v-if="filteredConversationSections.length === 0"
        class="px-3 py-4 text-center text-sm text-base-content/60"
      >
        {{ t("chat.conversationSearchEmpty") }}
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { Pin, PinOff } from "lucide-vue-next";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import { usePipelineStatus } from "../../shell/composables/use-pipeline-status";
import { formatConversationListTime } from "../utils/conversation-time";
import ChatConversationListHeader from "./ChatConversationListHeader.vue";

const CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY = "easy_call.chat_conversation_list_tab.v1";
const CHAT_CONVERSATION_LIST_COMPACT_STORAGE_KEY = "easy_call.chat_conversation_list_compact.v1";

const props = defineProps<{
  items: ChatConversationOverviewItem[];
  activeConversationId: string;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
}>();

const emit = defineEmits<{
  (e: "select", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
  (e: "rename", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
}>();

const { t, locale } = useI18n();
const renameInputRef = ref<HTMLInputElement | null>(null);
const editingConversationId = ref("");
const editingTitleDraft = ref("");
const conversationSearchQuery = ref("");
const activeConversationTab = ref<"local" | "contact">(readStoredConversationTab());
const compactConversationList = ref(readStoredCompactConversationList());
const { conversationStatusById, markConversationRead } = usePipelineStatus();

function readStoredConversationTab(): "local" | "contact" {
  if (typeof window === "undefined") return "local";
  const stored = String(window.localStorage.getItem(CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY) || "").trim();
  return stored === "contact" ? "contact" : "local";
}

function readStoredCompactConversationList(): boolean {
  if (typeof window === "undefined") return false;
  return String(window.localStorage.getItem(CHAT_CONVERSATION_LIST_COMPACT_STORAGE_KEY) || "").trim() === "1";
}

watch(activeConversationTab, (value) => {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY, value);
});

watch(compactConversationList, (value) => {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_CONVERSATION_LIST_COMPACT_STORAGE_KEY, value ? "1" : "0");
});

const conversationPreviewCache = computed(() => new Map(
  props.items.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));
const conversationSections = computed(() => {
  const visibleItems = props.items.filter((item) => {
    const kind = String(item.kind || "local_unarchived").trim();
    return activeConversationTab.value === "contact"
      ? kind === "remote_im_contact"
      : kind !== "remote_im_contact";
  });
  const pinned = visibleItems.filter((item) => !!item.isPinned || !!item.isMainConversation);
  const others = visibleItems.filter((item) => !item.isPinned && !item.isMainConversation);
  return [
    pinned.length > 0
      ? {
        key: "pinned",
        title: t("chat.pinnedConversations"),
        items: pinned,
      }
      : null,
    others.length > 0
      ? {
        key: "others",
        title: t("chat.otherConversations"),
        items: others,
      }
      : null,
  ].filter(Boolean) as Array<{ key: string; title: string; items: ChatConversationOverviewItem[] }>;
});

const normalizedConversationSearchQuery = computed(() =>
  String(conversationSearchQuery.value || "").trim().toLocaleLowerCase(),
);

const filteredConversationSections = computed(() => {
  const query = normalizedConversationSearchQuery.value;
  if (!query) return conversationSections.value;
  return conversationSections.value
    .map((section) => ({
      ...section,
      items: section.items.filter((item) => conversationMatchesSearch(item, query)),
    }))
    .filter((section) => section.items.length > 0);
});

watchEffect(() => {
  const editingId = String(editingConversationId.value || "").trim();
  if (!editingId) return;
  const item = props.items.find((entry) => String(entry.conversationId || "").trim() === editingId);
  if (!item || !canRenameConversation(item)) {
    resetConversationTitleEdit();
  }
});

watch(
  () => props.activeConversationId,
  (conversationId) => markConversationRead(conversationId),
  { immediate: true },
);

function resetConversationTitleEdit() {
  editingConversationId.value = "";
  editingTitleDraft.value = "";
}

function setRenameInputRef(element: Element | { $el?: Element | null } | null) {
  renameInputRef.value = element instanceof HTMLInputElement ? element : null;
}

function normalizedPreviewMessages(item: ChatConversationOverviewItem): ConversationPreviewMessage[] {
  return conversationPreviewCache.value.get(String(item.conversationId || "").trim()) || [];
}

function conversationMatchesSearch(item: ChatConversationOverviewItem, query: string): boolean {
  if (!query) return true;
  const title = conversationDisplayTitle(item).toLocaleLowerCase();
  if (title.includes(query)) return true;
  const previewTextBlock = normalizedPreviewMessages(item)
    .slice(-2)
    .map((preview) => previewText(preview).toLocaleLowerCase())
    .join("\n");
  return previewTextBlock.includes(query);
}

function isCurrentConversation(item: ChatConversationOverviewItem): boolean {
  return String(item.conversationId || "").trim() === String(props.activeConversationId || "").trim();
}

function conversationIndicatorTone(item: ChatConversationOverviewItem): "error" | "info" | "success" | "" {
  const conversationId = String(item.conversationId || "").trim();
  if (!conversationId || isCurrentConversation(item)) return "";
  const pipelineStatus = conversationStatusById.value[conversationId];
  if (pipelineStatus === "error") return "error";
  if (pipelineStatus === "busy") return "success";
  return "";
}

function conversationIndicatorClass(tone: "error" | "info" | "success" | ""): string {
  if (tone === "error") return "bg-error";
  if (tone === "success") return "bg-success";
  return "";
}

function isConversationDisabled(item: ChatConversationOverviewItem): boolean {
  return item.runtimeState === "organizing_context" || !!item.detachedWindowOpen;
}

function canRenameConversation(item: ChatConversationOverviewItem): boolean {
  if (item.kind === "remote_im_contact") return false;
  return isCurrentConversation(item) && !item.isMainConversation && !isConversationDisabled(item);
}

function isEditingTitle(item: ChatConversationOverviewItem): boolean {
  return String(item.conversationId || "").trim() === String(editingConversationId.value || "").trim();
}

function conversationDisplayTitle(item: ChatConversationOverviewItem): string {
  if (item.kind === "remote_im_contact") {
    return String(item.remoteContactDisplayName || item.title || "").trim() || t("chat.untitledConversation");
  }
  if (item.isMainConversation) return t("chat.mainConversation");
  return item.title || t("chat.untitledConversation");
}

function conversationItemTitle(item: ChatConversationOverviewItem): string {
  if (item.detachedWindowOpen) return t("chat.detachedWindowOpen");
  if (isConversationDisabled(item)) return t("chat.organizingContextDisabled");
  return item.workspaceLabel || t("chat.defaultWorkspace");
}

function workspaceDepartmentLabel(item: ChatConversationOverviewItem): string {
  if (item.kind === "remote_im_contact") {
    const departmentName = String(item.departmentName || "").trim();
    return departmentName || t("chat.contactConversationTab");
  }
  const workspaceLabel = String(item.workspaceLabel || "").trim() || t("chat.defaultWorkspace");
  const departmentName = String(item.departmentName || "").trim();
  if (!departmentName) return workspaceLabel;
  return `${workspaceLabel}（${departmentName}）`;
}

function handleConversationCardClick(item: ChatConversationOverviewItem) {
  if (isCurrentConversation(item) || isConversationDisabled(item)) return;
  emit("select", {
    conversationId: String(item.conversationId || "").trim(),
    kind: item.kind,
    remoteContactId: String(item.remoteContactId || "").trim() || undefined,
  });
}

function canToggleConversationPin(item: ChatConversationOverviewItem): boolean {
  return item.kind !== "remote_im_contact" && !item.isMainConversation;
}

function pinConversationTitle(item: ChatConversationOverviewItem): string {
  if (item.isMainConversation) return t("chat.mainConversationPinned");
  return item.isPinned ? t("chat.unpinConversation") : t("chat.pinConversation");
}

function toggleConversationPin(item: ChatConversationOverviewItem) {
  if (!canToggleConversationPin(item)) return;
  emit("togglePinConversation", String(item.conversationId || "").trim());
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
  emit("rename", {
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

function latestPreviewLine(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  const latestPreview = previews[previews.length - 1];
  if (!latestPreview) return t("chat.conversationNoPreview");
  return `${speakerLabel(latestPreview)}: ${previewText(latestPreview)}`;
}

function formatConversationTime(value?: string): string {
  return formatConversationListTime(value, locale.value);
}

function sideListLastSpeakerInitial(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "?";
  return speakerLabel(previews[previews.length - 1]).charAt(0).toUpperCase();
}

function sideListLastSpeakerLabel(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";
  return speakerLabel(previews[previews.length - 1]);
}

function sideListLastSpeakerAvatarUrl(item: ChatConversationOverviewItem): string {
  const previews = normalizedPreviewMessages(item);
  if (previews.length === 0) return "";
  const speakerId = String(previews[previews.length - 1].speakerAgentId || "").trim();
  if (!speakerId || speakerId === "user-persona") {
    return props.userAvatarUrl || "";
  }
  return props.personaAvatarUrlMap?.[speakerId] || "";
}
</script>
