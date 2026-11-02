<template>
  <div
    class="group relative mx-1 min-w-0"
    @contextmenu.prevent="handleCardContextMenu($event)"
    @pointerdown="handleCardPointerDown($event)"
    @pointerup="handleCardPointerUp"
    @pointerleave="handleCardPointerLeave"
  >
    <!-- 会话项统一模板：level 决定差异（full 头像 / sim+mini 无左指示，full+sim 两行 / mini 一行） -->
    <div
      class="group/item flex min-w-0 items-center gap-2 rounded-lg px-2.5 py-1 text-left transition-[background-color,box-shadow,transform] duration-200 ease-out"
      :class="[
        isActiveConversation ? 'bg-base-300' : 'bg-transparent hover:bg-base-300/60',
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
      <!-- 左侧：full 显示头像（含来源徽章），sim/mini 无左指示 -->
      <template v-if="level === 'full'">
        <div class="shrink-0" :style="leadColumnStyle">
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
      </template>
      <span v-else class="relative shrink-0 self-stretch" :style="leadColumnStyle" aria-hidden="true"></span>
      <!-- 右侧：第一行标题 + 时间，第二行摘要 + 状态区（full/sim 有，mini 无） -->
      <div class="min-w-0 flex-1">
          <div class="flex items-baseline justify-between gap-1.5">
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
              <div
                v-else
                class="min-w-0 truncate text-sm font-medium group-hover/item:text-base-content"
                :class="isActiveConversation ? 'text-base-content' : 'text-base-content/100'"
              >
                {{ displayTitle }}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1 pr-1">
              <Pin
                v-if="showPinIndicator"
                class="h-3.5 w-3.5 text-base-content/40"
                :aria-label="t('chat.pinnedConversations')"
              />
              <span v-else class="conversation-time-label text-xs text-base-content/60">
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
            <!-- 第二行右端簇，两件事各说各的，谁也不吞谁：
                 待定问题数（中性色，讲「还有几个问题等你回答」）→ 未读红圆（只讲「有几条没看」）。
                 两者语义无关，所以不合并、不为了省位置改写未读的既有形态。 -->
            <div class="ecall-conv-tail flex shrink-0 items-center gap-3">
              <span v-if="pipelineStatus === 'error'" class="badge badge-error badge-xs">{{ t("common.failed") }}</span>
              <span v-else-if="busyDetailText" class="text-xs text-base-content/60">{{ busyDetailText }}</span>
              <!-- 待定问题数：提醒，不是警报。中性填充 + 问号图标，
                   形状与未读红圆明确不同，避免又一次「两个数字挨着被读成一个量词」。 -->
              <span
                v-if="pendingQuestionBadge"
                class="ecall-conv-pending inline-flex h-5 shrink-0 items-center justify-center gap-0.5 rounded-full bg-base-content/10 px-1.5 text-xs font-medium text-base-content/70"
                :title="pendingQuestionTitle"
                :aria-label="pendingQuestionTitle"
                role="img"
              >
                <HelpCircle class="h-3 w-3" aria-hidden="true" />
                <span class="leading-none">{{ pendingQuestionBadge }}</span>
              </span>
              <span
                v-if="unreadBadge"
                class="ecall-conv-unread inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-error px-1.5 text-xs font-medium text-error-content"
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
        <li v-if="showJumpToSection">
          <button
            type="button"
            @click.stop="close(); emit('revealSection')"
          >
            <ListTree class="h-4 w-4" />
            <span>{{ t("chat.jumpToConversationSection") }}</span>
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
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Archive, HelpCircle, ListTree, PencilLine, Pin, PinOff, Trash2, Upload } from "@lucide/vue";
import FloatingConversationMenu from "./FloatingConversationMenu.vue";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import { stripPreviewMarkdown, stripToolcallMarkers } from "../../../utils/chat-message-semantics";
import { formatConversationListTime } from "../utils/conversation-time";
import { workspaceNameFromPath } from "../utils/conversation-sections";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import { SYSTEM_PERSONA_ID, USER_PERSONA_ID } from "../../shared/builtin-persona-ids";
import {
  conversationPendingQuestionBadge,
  conversationRuntimeBusy,
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
  /** 最近区：右键菜单提供「跳转所在分组」 */
  showJumpToSection?: boolean;
  /** 精简模式：小卡左侧占位收窄 */
  compactIndicator?: boolean;
}>(), {
  showSourceBadge: false,
  showJumpToSection: false,
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

/** 时间位置显示图钉：置顶会话可见，系统通知会话不显示（它不参与分组置顶语义） */
const showPinIndicator = computed(() => !!props.item.isPinned && !isSystemNotification.value);

// ==================== 展示 ====================

/** 前导列宽度：优先沿用所在分组折叠头透传的 CSS 变量，缺省时回到自身默认宽度 */
const leadColumnStyle = computed(() => {
  const fallback = props.compactIndicator ? "1rem" : "2.5rem";
  return { width: `max(var(--ecall-section-lead, ${fallback}), ${fallback})` };
});

const displayTitle = computed(() =>
  resolveConversationDisplayTitle(props.item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
    systemNotificationLabel: t("chat.systemPersona"),
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
  const text = stripPreviewMarkdown(stripToolcallMarkers(preview.textPreview || ""));
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

// 待定问题数：与未读同属「数量」层，含义却不同——它数的是等你回答的问题。
// 后端查询尚未接上时字段缺失 → 空串 → 整个元素不渲染，不给列表加一排恒为 0 的空壳。
const pendingQuestionBadge = computed(() => conversationPendingQuestionBadge(props.item));
const pendingQuestionTitle = computed(() =>
  t("chat.pendingQuestionsTooltip", { count: pendingQuestionBadge.value }),
);

const latestPreviewLine = computed(() => {
  if (busy.value) return t("chat.runtimeTyping");
  const previews = normalizedPreviewMessages.value;
  const latestPreview = [...previews].reverse().find(hasVisiblePreview);
  if (!latestPreview) return t("chat.conversationNoPreview");
  return previewText(latestPreview);
});

// ==================== 完整项：头像与状态指示 ====================

function speakerLabel(preview: ConversationPreviewMessage): string {
  if (preview.role === "tool") return t("archives.roleTool");
  const speakerId = String(preview.speakerAgentId || "").trim();
  if (!speakerId || speakerId === USER_PERSONA_ID) {
    return props.userAlias || t("archives.roleUser");
  }
  return props.personaNameMap?.[speakerId] || speakerId;
}

function systemPersonaLabel(): string {
  return props.personaNameMap?.[SYSTEM_PERSONA_ID] || t("chat.systemPersona");
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
  if (!speakerId || speakerId === USER_PERSONA_ID) {
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

// ==================== 操作 ====================

const canRename = computed(() =>
  isLocalConversation.value
  && !isSystemNotification.value,
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
