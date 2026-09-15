<template>
  <div
    class="group relative mx-1"
    @contextmenu.prevent="handleCardContextMenu($event)"
    @pointerdown="handleCardPointerDown($event)"
    @pointerup="handleCardPointerUp"
    @pointerleave="handleCardPointerLeave"
  >
    <!-- 左侧选中指示：仅 full/sim 用主题色竖条，mini 由右侧竖线承担 -->
    <div
      v-if="level !== 'mini'"
      class="pointer-events-none absolute left-0 top-1.5 bottom-1.5 w-0.5 rounded-full bg-primary transition-[transform,opacity] duration-200 ease-out origin-center"
      :class="isActiveConversation ? 'scale-y-100 opacity-100' : 'scale-y-50 opacity-0'"
      aria-hidden="true"
    ></div>
    <!-- 会话项统一模板：level 决定差异（full 头像 / sim+mini 指示灯，full+sim 两行 / mini 一行） -->
    <div
      class="flex items-center gap-2 rounded-lg px-2 py-1 text-left transition-[background-color,box-shadow,transform] duration-200 ease-out will-change-transform hover:bg-base-100/70"
      :class="[
        isActiveConversation ? 'bg-base-300 hover:bg-base-300 shadow-sm' : 'bg-transparent',
        isConversationVisuallyOccupied ? 'opacity-60' : '',
        isActiveConversation ? 'cursor-default' : 'cursor-pointer',
      ]"
      :role="isActiveConversation ? undefined : 'button'"
      :tabindex="isActiveConversation ? undefined : 0"
      :title="level === 'full' ? conversationItemTitle : undefined"
      @click="handleConversationCardClick"
      @keydown.enter.prevent="handleConversationCardClick"
      @keydown.space.prevent="handleConversationCardClick"
    >
      <!-- 左侧：full 显示头像（含状态点/来源徽章），sim/mini 显示竖线指示灯 -->
      <template v-if="level === 'full'">
        <div class="shrink-0">
          <div class="indicator">
            <span
              v-if="indicatorTone"
              class="indicator-item indicator-top indicator-end z-10 h-2.5 w-2.5 translate-x-0.5 -translate-y-0.5 rounded-full"
              :class="indicatorClass"
              aria-hidden="true"
            ></span>
            <div class="avatar relative overflow-visible">
              <div class="flex h-10 w-10 items-center justify-center rounded-full bg-neutral text-neutral-content">
                <img
                  v-if="displaySpeakerAvatarUrl"
                  :src="displaySpeakerAvatarUrl"
                  :alt="displaySpeakerLabel"
                  class="w-10 h-10 rounded-full object-cover"
                />
                <span v-else class="text-sm font-bold">{{ displaySpeakerInitial }}</span>
              </div>
              <span
                v-if="showSourceBadge"
                class="absolute bottom-0 left-1/2 z-20 inline-block max-w-10 -translate-x-1/2 translate-y-1/3 cursor-pointer truncate rounded-full bg-neutral px-1.5 py-[1px] text-micro font-normal leading-3 text-neutral-content shadow-sm transition-colors hover:bg-primary hover:text-primary-content"
                :title="t('chat.revealConversationSection')"
                role="button"
                tabindex="0"
                @click.stop="emit('revealSection')"
                @keydown.enter.stop.prevent="emit('revealSection')"
                @keydown.space.stop.prevent="emit('revealSection')"
              >
                {{ sourceBadgeLabel }}
              </span>
            </div>
          </div>
        </div>
      </template>
      <span v-else class="relative shrink-0 self-stretch" :class="compactIndicator ? 'w-4' : 'w-10'" aria-hidden="true">
        <span
          class="absolute right-0 top-1 bottom-1 w-1 rounded-full transition-colors"
          :class="simpleIndicatorClass"
        ></span>
      </span>

      <!-- 右侧：第一行标题 + 时间，第二行摘要 + 状态区（full/sim 有，mini 无） -->
      <div class="min-w-0 flex-1">
          <div class="flex items-start justify-between gap-1.5">
            <div class="flex min-w-0 items-center gap-1.5">
              <input
                v-if="editing"
                :ref="setRenameInputRef"
                v-model="editingTitleDraft"
                type="text"
                class="input input-bordered input-sm h-8 min-h-0 w-full max-w-full text-sm font-medium"
                @click.stop
                @mousedown.stop
                @keydown.enter.prevent="commitTitleEdit"
                @keydown.esc.prevent="cancelTitleEdit"
                @blur="commitTitleEdit"
              />
              <button
                v-else-if="canRename"
                type="button"
                class="min-w-0 truncate rounded px-0.5 text-left text-sm font-medium hover:bg-base-300/70"
                @click.stop="startTitleEdit"
              >
                {{ displayTitle }}
              </button>
              <div v-else class="min-w-0 truncate text-sm font-medium">
                {{ displayTitle }}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <span class="conversation-time-label text-xs text-base-content/60">
                {{ formatTime(item.updatedAt) }}
              </span>
            </div>
          </div>

          <div v-if="level !== 'mini'" class="mt-1 flex items-center justify-between gap-2 text-xs">
            <div class="flex min-w-0 items-center gap-1.5">
              <span
                v-if="busy"
                class="ecall-conv-dot shrink-0 rounded-full bg-base-content"
                aria-hidden="true"
              ></span>
              <span class="min-w-0 truncate" :class="busy ? 'ecall-conv-typing' : 'opacity-60'">
                {{ latestPreviewLine }}
              </span>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <span v-if="pipelineStatus === 'error'" class="badge badge-error badge-xs">{{ t("common.failed") }}</span>
              <span v-else-if="busyDetailText" class="text-xs text-base-content/60">{{ busyDetailText }}</span>
              <span
                v-if="unreadBadge"
                class="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-error px-1.5 text-xs font-medium text-error-content"
              >
                {{ unreadBadge }}
              </span>
            </div>
          </div>
        </div>
    </div>

    <FloatingConversationMenu
      v-if="isLocalConversation && !editing"
      ref="menuRef"
      :title="t('common.more')"
    >
      <template #default="{ close }">
        <li v-if="!item.isSystemNotificationConversation">
          <button
            type="button"
            :disabled="!canTogglePin"
            @click.stop="close(); togglePin()"
          >
            <PinOff v-if="item.isPinned" class="h-4 w-4" />
            <Pin v-else class="h-4 w-4" />
            <span>{{ pinTitle }}</span>
          </button>
        </li>
        <li v-if="!item.isSystemNotificationConversation">
          <button
            type="button"
            :disabled="!canRename"
            @click.stop="close(); startTitleEdit()"
          >
            <PencilLine class="h-4 w-4" />
            <span>{{ t("common.rename") }}</span>
          </button>
        </li>
        <li>
          <button
            type="button"
            :disabled="!canExport"
            @click.stop="close(); exportConversation()"
          >
            <Upload class="h-4 w-4" />
            <span>{{ t("chat.exportConversation") }}</span>
          </button>
        </li>
        <li v-if="!item.isSystemNotificationConversation">
          <button
            type="button"
            :disabled="!canArchive"
            @click.stop="close(); emit('archiveConversation', itemId)"
          >
            <Archive class="h-4 w-4" />
            <span>{{ t('common.archive') }}</span>
          </button>
        </li>
        <li v-if="!item.isSystemNotificationConversation">
          <button
            type="button"
            :disabled="!canDelete"
            class="text-error"
            @click.stop="close(); emit('deleteConversation', itemId)"
          >
            <Trash2 class="h-4 w-4" />
            <span>{{ t('common.delete') }}</span>
          </button>
        </li>
      </template>
    </FloatingConversationMenu>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Archive, PencilLine, Pin, PinOff, Trash2, Upload } from "@lucide/vue";
import FloatingConversationMenu from "./FloatingConversationMenu.vue";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import { stripToolcallMarkers } from "../../../utils/chat-message-semantics";
import { formatConversationListTime } from "../utils/conversation-time";
import { workspaceNameFromPath } from "../utils/conversation-sections";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import {
  conversationIndicatorClass,
  conversationRuntimeBusy,
  conversationSimpleIndicatorClass,
  conversationStatusIndicatorTone,
  conversationUnreadBadge,
  type ConversationItemLevel,
} from "../utils/conversation-item-display";

export type { ConversationItemLevel } from "../utils/conversation-item-display";

const props = withDefaults(defineProps<{
  item: ChatConversationOverviewItem;
  level: ConversationItemLevel;
  activeConversationId: string;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  pipelineStatusById: Record<string, ConversationPipelineStatus>;
  /** recent 分组下显示来源徽章（仅 full 项） */
  showSourceBadge?: boolean;
  /** 精简模式：小卡左侧不预留头像宽度，指示条贴左 */
  compactIndicator?: boolean;
}>(), {
  showSourceBadge: false,
  compactIndicator: false,
});

const emit = defineEmits<{
  (e: "select", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
  (e: "rename", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
  (e: "archiveConversation", conversationId: string): void;
  (e: "exportConversation", conversationId: string): void;
  (e: "deleteConversation", conversationId: string): void;
  (e: "revealSection"): void;
}>();

const { t, locale } = useI18n();

const itemId = computed(() => String(props.item.conversationId || "").trim());
const isLocalConversation = computed(() => props.item.kind !== "remote_im_contact");
const isActiveConversation = computed(() => itemId.value === String(props.activeConversationId || "").trim());
const isSystemNotification = computed(() => !!props.item.isSystemNotificationConversation);

// ==================== 展示 ====================

const displayTitle = computed(() =>
  resolveConversationDisplayTitle(props.item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
  }),
);

const conversationItemTitle = computed(() =>
  props.item.workspaceLabel || t("chat.defaultWorkspace"),
);

function formatTime(value?: string): string {
  return formatConversationListTime(value, locale.value);
}

const normalizedPreviewMessages = computed<ConversationPreviewMessage[]>(() =>
  Array.isArray(props.item.previewMessages) ? props.item.previewMessages : [],
);

function previewText(preview: ConversationPreviewMessage): string {
  const text = stripToolcallMarkers(preview.textPreview || "");
  if (text) return text;
  if (preview.hasPdf) return t("chat.previewPdf");
  if (preview.hasImage) return t("chat.previewImage");
  if (preview.hasAudio) return t("chat.previewAudio");
  if (preview.hasAttachment) return t("chat.previewAttachment");
  return t("chat.conversationNoPreview");
}

function hasVisiblePreview(preview: ConversationPreviewMessage): boolean {
  return !!stripToolcallMarkers(preview.textPreview || "")
    || !!preview.hasPdf
    || !!preview.hasImage
    || !!preview.hasAudio
    || !!preview.hasAttachment;
}

const pipelineStatus = computed(() =>
  props.pipelineStatusById?.[itemId.value] || "",
);

const busy = computed(() =>
  pipelineStatus.value === "busy" || conversationRuntimeBusy(props.item.runtimeState),
);

function runtimeStateText(runtimeState?: ChatConversationOverviewItem["runtimeState"]): string {
  if (runtimeState === "assistant_streaming") return t("chat.runtimeStreaming");
  if (runtimeState === "organizing_context") return t("chat.runtimeOrganizing");
  if (runtimeState === "archiving") return "归档中";
  if (runtimeState === "compacting") return "压缩中";
  return t("chat.runtimeIdle");
}

// 运行中：通用流式由摘要行的「正在输入…」承担，这里只补充更具体的阶段
const busyDetailText = computed(() => {
  const runtimeState = props.item.runtimeState;
  if (runtimeState && runtimeState !== "idle" && runtimeState !== "assistant_streaming") {
    return runtimeStateText(runtimeState);
  }
  return "";
});

const unreadBadge = computed(() =>
  conversationUnreadBadge(props.item, props.activeConversationId),
);

const latestPreviewLine = computed(() => {
  if (busy.value) return t("chat.runtimeTyping");
  const previews = normalizedPreviewMessages.value;
  const latestPreview = [...previews].reverse().find(hasVisiblePreview);
  if (!latestPreview) return t("chat.conversationNoPreview");
  return previewText(latestPreview);
});

// ==================== 完整项：头像与状态指示 ====================

const SYSTEM_PERSONA_ID = "system-persona";

function speakerLabel(preview: ConversationPreviewMessage): string {
  if (preview.role === "tool") return t("archives.roleTool");
  const speakerId = String(preview.speakerAgentId || "").trim();
  if (!speakerId || speakerId === "user-persona") {
    return props.userAlias || t("archives.roleUser");
  }
  return props.personaNameMap?.[speakerId] || speakerId;
}

function systemPersonaLabel(): string {
  return props.personaNameMap?.[SYSTEM_PERSONA_ID] || "P-ai系统";
}

function systemPersonaInitial(): string {
  return systemPersonaLabel().charAt(0).toUpperCase() || "P";
}

function systemPersonaAvatarUrl(): string {
  return props.personaAvatarUrlMap?.[SYSTEM_PERSONA_ID] || "";
}

function lastSpeakerInitial(): string {
  if (props.item.isSystemNotificationConversation) return systemPersonaInitial();
  const previews = normalizedPreviewMessages.value;
  if (previews.length === 0) return "?";
  return speakerLabel(previews[previews.length - 1]).charAt(0).toUpperCase();
}

function lastSpeakerLabel(): string {
  if (props.item.isSystemNotificationConversation) return systemPersonaLabel();
  const previews = normalizedPreviewMessages.value;
  if (previews.length === 0) return "";
  return speakerLabel(previews[previews.length - 1]);
}

function lastSpeakerAvatarUrl(): string {
  if (props.item.isSystemNotificationConversation) return systemPersonaAvatarUrl();
  const previews = normalizedPreviewMessages.value;
  if (previews.length === 0) return "";
  const speakerId = String(previews[previews.length - 1].speakerAgentId || "").trim();
  if (!speakerId || speakerId === "user-persona") {
    return props.userAvatarUrl || "";
  }
  return props.personaAvatarUrlMap?.[speakerId] || "";
}

function assistantLabel(): string {
  const agentId = String(props.item.agentId || "").trim();
  if (!agentId) return lastSpeakerLabel();
  return props.personaNameMap?.[agentId] || agentId;
}

function assistantAvatarUrl(): string {
  const agentId = String(props.item.agentId || "").trim();
  if (!agentId) return "";
  return props.personaAvatarUrlMap?.[agentId] || "";
}

const displaySpeakerLabel = computed(() => {
  if (!busy.value) return lastSpeakerLabel();
  return assistantLabel();
});

const displaySpeakerInitial = computed(() =>
  displaySpeakerLabel.value.charAt(0).toUpperCase() || "?",
);

const displaySpeakerAvatarUrl = computed(() => {
  if (!busy.value) return lastSpeakerAvatarUrl();
  return assistantAvatarUrl() || lastSpeakerAvatarUrl();
});

const indicatorTone = computed(() =>
  conversationStatusIndicatorTone(pipelineStatus.value, isActiveConversation.value),
);

const indicatorClass = computed(() => conversationIndicatorClass(indicatorTone.value));

const isConversationVisuallyOccupied = computed(() => false);

const sourceBadgeLabel = computed(() => {
  if (props.item.kind === "remote_im_contact") {
    return String(
      props.item.channelName
      || props.item.remoteContactDisplayName
      || t("chat.otherConversations"),
    ).trim();
  }
  const workspacePath = String(props.item.workspaceRootPath || "").trim();
  return String(
    props.item.workspaceLabel
    || workspaceNameFromPath(workspacePath)
    || t("chat.defaultWorkspace"),
  ).trim();
});

// ==================== 简单项：指示条与摘要 ====================

const simpleIndicatorClass = computed(() =>
  conversationSimpleIndicatorClass(props.item, unreadBadge.value, normalizedPreviewMessages.value),
);

// ==================== 操作 ====================

const canRename = computed(() =>
  isLocalConversation.value
  && !isSystemNotification.value
  && isActiveConversation.value,
);

const canTogglePin = computed(() =>
  isLocalConversation.value && !isSystemNotification.value,
);

const canArchive = computed(() =>
  isLocalConversation.value && !isSystemNotification.value,
);

const canDelete = computed(() =>
  isLocalConversation.value && !isSystemNotification.value,
);

const canExport = computed(() => isLocalConversation.value);

const pinTitle = computed(() => {
  if (props.item.isSystemNotificationConversation) return t("chat.mainConversationPinned");
  return props.item.isPinned ? t("chat.unpinConversation") : t("chat.pinConversation");
});

function handleConversationCardClick() {
  if (isActiveConversation.value) return;
  emit("select", {
    conversationId: itemId.value,
    kind: props.item.kind,
    remoteContactId: String(props.item.remoteContactId || "").trim() || undefined,
  });
}

function togglePin() {
  if (!canTogglePin.value) return;
  emit("togglePinConversation", itemId.value);
}

function exportConversation() {
  if (!canExport.value) return;
  emit("exportConversation", itemId.value);
}

// ==================== 编辑标题 ====================

const editing = ref(false);
const editingTitleDraft = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

function setRenameInputRef(element: Element | { $el?: Element | null } | null) {
  renameInputRef.value = element instanceof HTMLInputElement ? element : null;
}

async function startTitleEdit() {
  if (!canRename.value) return;
  editing.value = true;
  editingTitleDraft.value = String(props.item.title || "").trim();
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

function resetTitleEdit() {
  editing.value = false;
  editingTitleDraft.value = "";
}

function cancelTitleEdit() {
  resetTitleEdit();
}

function commitTitleEdit() {
  if (!editing.value) return;
  const currentTitle = String(props.item.title || "").trim();
  const nextTitle = String(editingTitleDraft.value || "").trim();
  if (!itemId.value || nextTitle === currentTitle) {
    resetTitleEdit();
    return;
  }
  resetTitleEdit();
  emit("rename", {
    conversationId: itemId.value,
    title: nextTitle,
  });
}

// 当前会话切换后，若不再满足重命名条件（如不再是当前会话），退出编辑态
watch(
  () => props.activeConversationId,
  () => {
    if (editing.value && !canRename.value) {
      resetTitleEdit();
    }
  },
);

// ==================== 右键 / 长按菜单 ====================

const menuRef = ref<InstanceType<typeof FloatingConversationMenu> | null>(null);
let longPressTimer: ReturnType<typeof setTimeout> | null = null;

function clearLongPressTimer() {
  if (longPressTimer !== null) {
    clearTimeout(longPressTimer);
    longPressTimer = null;
  }
}

function handleCardContextMenu(event: MouseEvent) {
  clearLongPressTimer();
  menuRef.value?.openMenu(event.clientX, event.clientY);
}

function handleCardPointerDown(event: PointerEvent) {
  if (event.pointerType !== "touch") return;
  clearLongPressTimer();
  const clientX = event.clientX;
  const clientY = event.clientY;
  longPressTimer = setTimeout(() => {
    menuRef.value?.openMenu(clientX, clientY);
  }, 500);
}

function handleCardPointerUp() {
  clearLongPressTimer();
}

function handleCardPointerLeave() {
  clearLongPressTimer();
}

onBeforeUnmount(() => {
  clearLongPressTimer();
});
</script>

<style scoped>
.conversation-time-label {
  /* 容器宽度过窄时由父容器 @container 规则隐藏 */
}

/* 运行中：摘要行文字扫光——浅色主题黑光、深色主题白光，配一枚呼吸光点 */
.ecall-conv-typing {
  font-weight: 500;
  background-image: linear-gradient(
    90deg,
    color-mix(in oklab, var(--color-base-content) 50%, transparent) 0%,
    var(--color-base-content) 50%,
    color-mix(in oklab, var(--color-base-content) 50%, transparent) 100%
  );
  background-size: 200% 100%;
  background-clip: text;
  -webkit-background-clip: text;
  color: transparent;
  animation: ecall-conv-typing 1.8s ease-in-out infinite;
}

@keyframes ecall-conv-typing {
  0% {
    background-position: 100% 0;
  }
  100% {
    background-position: -100% 0;
  }
}

.ecall-conv-dot {
  width: 6px;
  height: 6px;
  animation: ecall-conv-dot 1.6s ease-in-out infinite;
}

@keyframes ecall-conv-dot {
  0%,
  100% {
    opacity: 0.3;
    transform: scale(0.8);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
