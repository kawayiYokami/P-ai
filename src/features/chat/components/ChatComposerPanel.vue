<template>
  <div>
    <div
      v-if="linkOpenErrorText"
      class="alert alert-warning mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-24 overflow-auto"
    >
      <span>{{ linkOpenErrorText }}</span>
    </div>
    <ChatSelectionActionPanel
      v-if="selectionModeEnabled"
      :show-conversation-actions="showConversationActions"
      :delegate-only="selectionDelegateOnly || systemNotificationMode || remoteContactMode"
      :selected-message-count="selectedMessageCount"
      :active-conversation-id="activeConversationId"
      :unarchived-conversation-items="unarchivedConversationItems"
      :remote-im-contact-conversations="remoteImContactConversations"
      :create-conversation-department-options="createConversationDepartmentOptions"
      :persona-avatar-url-map="personaAvatarUrlMap"
      :active-agent-id="activeAgentId"
      @exit-selection-mode="emit('exitSelectionMode')"
      @selection-action-branch="emit('selectionActionBranch')"
      @selection-action-forward="emit('selectionActionForward', $event)"
      @selection-action-delegate="emit('selectionActionDelegate', $event)"
      @selection-action-copy="emit('selectionActionCopy')"
      @selection-action-share="emit('selectionActionShare', $event)"
    />
    <template v-else>
    <div v-if="systemNotificationMode" class="flex flex-wrap items-center justify-center gap-2">
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openDelegateSelection')"
      >
        <ClipboardList class="h-3.5 w-3.5" />
        {{ t("chat.conversationMenu.startDelegate") }}
      </button>
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openTaskCreate')"
      >
        <CalendarPlus class="h-3.5 w-3.5" />
        {{ t("chat.newTask") }}
      </button>
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="openCreateConversationDialog"
      >
        <Plus class="h-3.5 w-3.5" />
        {{ t("chat.newConversation") }}
      </button>
    </div>
    <div v-else-if="remoteContactMode" class="flex flex-wrap items-center justify-center gap-2">
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openTaskCreate')"
      >
        <CalendarPlus class="h-3.5 w-3.5" />
        {{ t("chat.newTask") }}
      </button>
    </div>
    <template v-else>
    <InputPanelDock
      :queue-events="queueEnabled && !systemNotificationMode && !remoteContactMode ? visibleQueueEvents : []"
      :user-persona-name="queueUserPersonaName"
      :queue-visible="props.queueVisible ?? true"
      :is-rounded="!!isRounded"
      dock-bg="base-200"
      @recall-to-input="handleRecallToInput"
      @mark-guided="handleQueueMarkGuided"
    >
        <template #attachments>
          <div v-if="selectedMentions.length > 0" class="mb-2 flex flex-wrap gap-1">
            <span
              v-for="item in selectedMentions"
              :key="`${item.agentId}:${item.departmentId}`"
              class="badge gap-1 bg-base-300 px-3 py-3 text-sm text-base-content border-transparent"
            >
              <span class="max-w-40 truncate leading-none">@{{ mentionDisplayLabel(item) }}</span>
              <button
                type="button"
                class="ml-0.5 inline-flex h-5 w-5 items-center justify-center rounded-full text-base-content transition hover:bg-error hover:text-error-content"
                @click.stop="removeSelectedMention(item)"
              >
                <X class="h-3 w-3" />
              </button>
            </span>
          </div>
          <InputPanelAttachments
            :images="panelAttachmentImages"
            :files="panelAttachmentFiles"
            :bridges="panelBridgeItems"
            @remove-image="removeClipboardImageAt($event)"
            @remove-file="removePanelAttachmentFile($event)"
            @toggle-bridge="togglePanelBridge($event)"
          />
        </template>
        <template #body>
          <div ref="composerRootRef" class="relative">
            <Transition name="ecall-dropdown-up">
              <div
                v-if="instructionPanelOpen"
                class="absolute bottom-full left-0 right-0 z-30 mb-1.5 overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
              >
                <button
                  type="button"
                  class="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm transition-colors"
                  :class="planModeEnabled ? 'bg-info/10' : 'hover:bg-base-200'"
                  :title="`Shift+Tab ${planModeEnabled ? t('chat.plan.exitMode') : t('chat.plan.enterMode')}`"
                  @click="togglePlanMode()"
                >
                  <span class="flex min-w-0 items-center gap-2">
                    <ClipboardList class="h-4 w-4 shrink-0" :class="planModeEnabled ? 'text-info' : 'opacity-60'" />
                    <span class="truncate">{{ planModeEnabled ? t("chat.plan.enabled") : t("chat.plan.enterMode") }}</span>
                    <kbd class="kbd kbd-xs shrink-0 opacity-60">Shift+Tab</kbd>
                  </span>
                  <span
                    class="badge badge-sm shrink-0"
                    :class="planModeEnabled ? 'badge-info' : 'badge-ghost'"
                  >{{ planModeEnabled ? t("chat.plan.exitMode") : t("chat.plan.modeOff") }}</span>
                </button>
                <div class="border-t border-base-300/60" />
                <div class="flex flex-wrap content-start gap-2 max-h-48 overflow-y-auto p-2">
                  <button
                    v-for="(item, index) in normalizedInstructionPresets"
                    :key="item.id"
                    type="button"
                    class="btn btn-sm min-h-0 max-w-full justify-start normal-case px-3"
                    :class="instructionFocusIndex === index ? 'btn-primary' : 'btn-ghost'"
                    :title="item.prompt"
                    @click="applyInstructionPreset(item)"
                  >
                    <span class="block max-w-64 truncate text-left text-sm sm:max-w-80">{{ item.prompt }}</span>
                  </button>
                  <div v-if="normalizedInstructionPresets.length === 0" class="w-full px-2 py-3 text-sm opacity-60">
                    {{ t("chat.noInstructionPresets") }}
                  </div>
                </div>
              </div>
            </Transition>
            <Transition name="ecall-dropdown-up">
              <div
                v-if="mentionPanelOpen"
                class="absolute bottom-full left-0 z-30 mb-1.5 w-max max-w-[min(80vw,20rem)] overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
              >
                <div
                  ref="mentionPanelScrollRef"
                  class="max-h-[min(56vh,24rem)] overflow-y-auto overscroll-contain p-1"
                >
                  <ul class="flex flex-col gap-1">
                    <li
                      v-for="(item, index) in filteredMentionOptions"
                      :key="`${item.agentId}:${item.departmentId}`"
                    >
                      <button
                        type="button"
                        :data-mention-option-index="index"
                        class="flex min-h-0 w-full items-start gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors"
                        :class="[
                          mentionFocusIndex === index ? 'bg-base-200' : '',
                          item.mentionable ? 'hover:bg-base-200/80' : 'opacity-65',
                        ]"
                        :disabled="!item.mentionable"
                        @click="applyMention(item)"
                      >
                        <div class="indicator shrink-0">
                          <span
                            v-if="isMentionSelected(item)"
                            class="indicator-item inline-flex h-4 w-4 items-center justify-center rounded-full bg-primary text-micro font-bold text-primary-content"
                          >
                            @
                          </span>
                          <div class="avatar">
                            <div class="w-7 rounded-full">
                              <img
                                v-if="item.avatarUrl"
                                :src="item.avatarUrl"
                                :alt="item.agentName"
                                class="w-7 h-7 rounded-full object-cover"
                              />
                              <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-caption">
                                {{ avatarInitial(item.agentName) }}
                              </div>
                            </div>
                          </div>
                        </div>
                        <div class="min-w-0 flex-1 pr-0.5">
                          <div class="truncate text-sm leading-5">@{{ mentionDisplayLabel(item) }}</div>
                          <div
                            v-if="!item.mentionable && item.unavailableReason"
                            class="truncate text-xs leading-4 text-base-content/60"
                          >
                            {{ item.unavailableReason }}
                          </div>
                        </div>
                      </button>
                    </li>
                  </ul>
                  <div v-if="filteredMentionOptions.length === 0" class="px-2.5 py-2 text-sm opacity-60">
                    {{ t("chat.noMentionCandidates") }}
                  </div>
                </div>
              </div>
            </Transition>
            <textarea
              ref="chatInputRef"
              v-model="localChatInput"
              class="block max-h-40 min-h-8 w-full resize-none overflow-y-auto bg-transparent px-2 py-1 text-sm leading-6 outline-none chat-input-no-focus"
            rows="1"
            :placeholder="effectiveChatInputPlaceholder"
            @input="handleChatInputInput"
            @compositionstart="handleChatInputCompositionStart"
            @compositionend="handleChatInputCompositionEnd"
            @keydown="handleChatInputKeydown"
            @focus="handleChatInputFocus"
            @blur="handleChatInputBlur"
          ></textarea>
            <FloatingScrollbar v-if="chatInputRef" :target="chatInputRef" />
          </div>
        </template>
        <template #footer>
          <InputPanelToolbar>
            <template #left>
              <button
                type="button"
                class="btn btn-sm btn-circle shrink-0 transition-transform duration-150 ease-out active:scale-90"
                :class="goalActive ? 'btn-primary' : 'btn-ghost'"
                :disabled="frozen || goalDisabled"
                :title="goalTitle || t('chat.goal.buttonTitle')"
                @click="emit('openGoalTask')"
              >
                <Target class="h-3.5 w-3.5" />
              </button>
              <button
                v-if="showConversationActions"
                type="button"
                class="btn btn-sm btn-circle btn-ghost shrink-0 transition-transform duration-150 ease-out active:scale-90"
                :title="t('chat.attach')"
                @click="emit('pickAttachments')"
              >
                <Paperclip class="h-3.5 w-3.5" />
              </button>
              <ChatModelPicker
                variant="chip"
                class="min-w-0 flex-1"
                :model-value="activeModelDisplayId"
                :api-configs="chatModelOptions"
                :theme="teleportTheme"
                @update:model-value="selectConversationPreferredModel"
              />
            </template>
            <template #right>
              <button
                v-if="planModeEnabled"
                type="button"
                class="inline-flex h-8 min-h-8 shrink-0 select-none items-center rounded-full bg-info px-3 text-xs font-medium leading-none text-info-content"
                :title="`Shift+Tab ${t('chat.plan.mode')}`"
                @click="togglePlanMode()"
              >
                {{ t("chat.plan.mode") }}
              </button>
              <button
                v-else-if="planSuggestionVisible"
                type="button"
                class="inline-flex h-8 min-h-8 shrink-0 select-none items-center rounded-full bg-base-200 px-3 text-xs font-medium leading-none text-base-content"
                :title="`Shift+Tab ${t('chat.plan.mode')}`"
                @click="togglePlanMode()"
              >
                {{ t("chat.plan.mode") }}
              </button>
              <button
                v-if="showStopAction"
                type="button"
                class="btn btn-sm btn-circle shrink-0 btn-error transition-transform duration-150 ease-out active:scale-90"
                :disabled="frozen || busy || !!stopChatDisabled"
                :title="`${t('chat.stop')} / ${t('chat.stopReplying')}`"
                @click="emit('stopChat')"
              >
                <Square class="h-3.5 w-3.5 fill-current" />
              </button>
              <div v-else ref="sendModeMenuRef" class="relative flex shrink-0">
                <button
                  type="button"
                  class="btn btn-sm btn-circle shrink-0 transition-transform duration-150 ease-out active:scale-90"
                  :class="composerInputBlank ? 'bg-base-200' : 'btn-success'"
                  :disabled="!composerInputBlank && (frozen || busy)"
                  :title="composerInputBlank ? t('chat.sendModeMenu') : t('chat.send')"
                  @click="composerInputBlank ? (sendModeMenuOpen = !sendModeMenuOpen) : handleSendChat()"
                  @contextmenu.prevent="sendModeMenuOpen = !sendModeMenuOpen"
                >
                  <ArrowUp class="h-3.5 w-3.5" />
                </button>
                <div
                  v-if="sendModeMenuOpen"
                  class="absolute bottom-full right-0 z-50 mb-1.5 min-w-52 overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
                >
                  <div class="flex flex-col p-1">
                    <button
                      type="button"
                      class="flex min-h-8 w-full items-center justify-between gap-3 rounded-lg px-2.5 text-left text-sm transition-colors hover:bg-base-200"
                      @click="setSendMode('enter')"
                    >
                      <span>{{ t("chat.sendModeEnter") }}</span>
                      <Check v-if="sendMode === 'enter'" class="h-4 w-4 shrink-0 text-primary" />
                    </button>
                    <button
                      type="button"
                      class="flex min-h-8 w-full items-center justify-between gap-3 rounded-lg px-2.5 text-left text-sm transition-colors hover:bg-base-200"
                      @click="setSendMode('ctrl_enter')"
                    >
                      <span>{{ t("chat.sendModeCtrlEnter") }}</span>
                      <Check v-if="sendMode === 'ctrl_enter'" class="h-4 w-4 shrink-0 text-primary" />
                    </button>
                    <div class="px-2.5 pt-1 pb-0.5 text-xs opacity-50">{{ t("chat.sendModeAltS") }}</div>
                  </div>
                </div>
              </div>
            </template>
          </InputPanelToolbar>
        </template>
    </InputPanelDock>
    </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUp, CalendarPlus, Check, ClipboardList, Paperclip, Plus, Square, Target, X } from "@lucide/vue";
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMentionEntry, ChatMentionTarget, ConversationForwardTarget, IdeContextReferenceItem, IdeContextWorkspaceGroup, PromptCommandPreset, RemoteImContactConversationOption } from "../../../types/app";
import ChatSelectionActionPanel from "./ChatSelectionActionPanel.vue";
import ChatModelPicker from "../../config/components/ApiConfigPicker.vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import InputPanelAttachments from "./input-panel/InputPanelAttachments.vue";
import InputPanelDock from "./input-panel/InputPanelDock.vue";
import InputPanelToolbar from "./input-panel/InputPanelToolbar.vue";
import { useChatQueue, type ChatQueueEvent } from "../composables/use-chat-queue";
import { chatInputEnterConfirmsComposition } from "../composables/chat-composer-ime";
import { clearChatComposerFocus, registerChatComposerFocus } from "../composables/chat-composer-focus";
import type { DepartmentPersonaOption } from "../../shared/department-persona-options";
import { ideContextReferenceDisplayParts } from "../utils/ide-context-reference-display";
import { mergeComposerIdeContextGroups } from "../utils/ide-context-reference-groups";

type BinaryAttachment = { mime: string; bytesBase64: string; previewDataUrl?: string };
type QueuedAttachmentNotice = { id: string; fileName: string; path: string; mime: string; pending?: boolean };
type ConversationDepartmentOption = DepartmentPersonaOption;
type MentionOptionView = {
  agentId: string;
  agentName: string;
  departmentId: string;
  departmentName: string;
  avatarUrl?: string;
  mentionable: boolean;
  hidden?: boolean;
  unavailableReason?: string;
};

const props = withDefaults(defineProps<{
  composerScope?: "main" | "side";
  selectionModeEnabled: boolean;
  selectionDelegateOnly?: boolean;
  selectedMessageCount: number;
  chatInput: string;
  instructionPresets: PromptCommandPreset[];
  mentionEntries: ChatMentionEntry[];
  selectedMentions: ChatMentionTarget[];
  clipboardImages: BinaryAttachment[];  queuedAttachmentNotices: QueuedAttachmentNotice[];
  linkOpenErrorText: string;
  conversationCallPrimaryApiConfigId: string;
  preferredChatModelId?: string;
  chatModelOptions: ApiConfigItem[];
  workspaceAccess?: "approval" | "full_access" | "";
  planModeEnabled: boolean;
  chatting: boolean;
  frontendRoundPhase?: "idle" | "queued" | "waiting" | "streaming";
  busy: boolean;
  stopChatDisabled?: boolean;
  frozen: boolean;
  goalActive: boolean;
  goalTitle: string;
  goalDisabled?: boolean;
  systemNotificationMode?: boolean;
  remoteContactMode?: boolean;
  showSideConversationList: boolean;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  remoteImContactConversations: RemoteImContactConversationOption[];
  userAlias: string;
  userAvatarUrl: string;
  personaName: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  createConversationDepartmentOptions: ConversationDepartmentOption[];
  defaultCreateConversationDepartmentId: string;
  ideContextGroups: IdeContextWorkspaceGroup[];
  attachedIdeContextReferences: IdeContextReferenceItem[];
  currentTheme?: string;
  showConversationActions?: boolean;
  chatUsagePercent?: number;
  activeAgentId?: string;
  isRounded?: boolean;
  queueEventsOverride?: ChatQueueEvent[] | null;
  queueVisible?: boolean;
}>(), {
  /** Vue 会把未传的可选布尔 prop 转成 false 而不是 undefined，必须显式默认显示，否则生产面板队列永远隐藏 */
  queueVisible: true,
});

const emit = defineEmits<{
  (e: "exitSelectionMode"): void;
  (e: "selectionActionBranch"): void;
  (e: "selectionActionForward", target: ConversationForwardTarget): void;
  (e: "selectionActionDelegate", payload: { departmentId: string; agentId: string; presetId: string; why: string; goal: string; todo: string }): void;
  (e: "selectionActionCopy"): void;
  (e: "selectionActionShare", format: "html" | "png" | "copyPng"): void;
  (e: "update:chatInput", value: string): void;
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", value: string | { agentId: string; departmentId?: string }): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "pickAttachments"): void;
  (e: "update:conversationPreferredApiConfigId", value: string): void;
  (e: "update:workspaceAccess", value: "approval" | "full_access"): void;
  (e: "update:planModeEnabled", value: boolean): void;
  (e: "attachIdeContextReference", value: IdeContextReferenceItem): void;
  (e: "removeIdeContextReference", value: string): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "openDelegateSelection"): void;
  (e: "openTaskCreate"): void;
  (e: "openGoalTask"): void;
  (e: "open-conversation-list"): void;
  (e: "open-settings"): void;
  (e: "trim-conversation"): void;
  (e: "queueRecall", event: ChatQueueEvent): void;
  (e: "queueMarkGuided", eventId: string): void;
  (e: "createConversation", input?: { departmentId?: string; agentId?: string }): void;
}>();

const { t } = useI18n();
const queueEnabled = computed(() => true);
const showConversationActions = computed(() => props.showConversationActions ?? true);
const systemNotificationMode = computed(() => !!props.systemNotificationMode);
const remoteContactMode = computed(() => !!props.remoteContactMode);

/** 输入区无可发送内容（无文字、无图片、无待发附件）时，发送按钮降级为菜单入口。 */
const composerInputBlank = computed(() => {
  if (String(props.chatInput || "").trim()) return false;
  return props.clipboardImages.length === 0 && props.queuedAttachmentNotices.length === 0;
});

/** 计划类请求关键词：命中时显示可点击的「计划」按钮（base-200），点击进入计划模式。 */
const PLAN_SUGGESTION_KEYWORDS = ["计划", "方案", "plan", "design"];
const planSuggestionVisible = computed(() => {
  if (props.planModeEnabled) return false;
  const text = String(props.chatInput || "").toLowerCase();
  if (!text) return false;
  return PLAN_SUGGESTION_KEYWORDS.some((keyword) => text.includes(keyword.toLowerCase()));
});

// Product rule: an in-flight assistant reply must not lock the input toolbar.
// Users can keep typing while streaming, so do not use `chatting` as the disabled
// condition for attach/record/command/task/delegate actions. Only gate on real
// hard blockers such as frozen state, explicit busy flows, permissions, or action-specific prerequisites.
const teleportTheme = computed(() => {
  const documentTheme = typeof document === "undefined" ? "" : document.documentElement.getAttribute("data-theme");
  return String(props.currentTheme || documentTheme || "light").trim() || "light";
});

function openCreateConversationDialog() {
  if (typeof window === "undefined") {
    emit("createConversation");
    return;
  }
  window.dispatchEvent(new CustomEvent("easy-call:open-draft-conversation"));
}

const menuOpen = ref(false);
const menuTriggerRef = ref<HTMLButtonElement | null>(null);
const menuWrapperRef = ref<HTMLDivElement | null>(null);

function closeMenu() {
  menuOpen.value = false;
}

function handleOpenHistory() {
  closeMenu();
  emit('open-conversation-list');
}

function handleOpenConfig() {
  closeMenu();
  emit('open-settings');
}

function onMenuOutsideClick(event: MouseEvent) {
  const target = event.target as Node | null;
  if (menuOpen.value) {
    if (menuWrapperRef.value && menuWrapperRef.value.contains(target)) return;
    closeMenu();
  }
  if (sendModeMenuOpen.value) {
    const sendModeRoot = sendModeMenuRef.value;
    if (sendModeRoot && sendModeRoot.contains(target)) return;
    sendModeMenuOpen.value = false;
  }
  if (instructionPanelOpen.value) {
    const root = composerRootRef.value;
    if (!root || !root.contains(target)) {
      closeInstructionPanel();
    }
  }
  if (mentionPanelOpen.value) {
    const mentionRoot = mentionPanelScrollRef.value;
    const inputEl = chatInputRef.value;
    const insideMention = !!mentionRoot && mentionRoot.contains(target);
    const insideInput = !!inputEl && inputEl.contains(target);
    const insideComposer = !!composerRootRef.value && composerRootRef.value.contains(target);
    if (!insideMention && !insideInput && !insideComposer) {
      closeMentionPanel();
    }
  }
}

onMounted(() => { document.addEventListener('pointerdown', onMenuOutsideClick); });
onBeforeUnmount(() => { document.removeEventListener('pointerdown', onMenuOutsideClick); });

const { queueEvents, recallQueueEvent, markGuided } = useChatQueue({
  enabled: queueEnabled,
});

const visibleQueueEvents = computed(() => {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    return props.queueEventsOverride;
  }
  const activeConversationId = String(props.activeConversationId || "").trim();
  if (!activeConversationId) return [];
  return queueEvents.value.filter(
    (event) => String(event.conversationId || "").trim() === activeConversationId,
  );
});

const queueUserPersonaName = computed(() =>
  String(props.personaNameMap["user-persona"] || props.userAlias || "").trim(),
);

/** 输入框占位文案：按忙碌/队列/引导状态切换，忙碌态嵌入人格名。 */
const effectiveChatInputPlaceholder = computed(() => {
  const personaName = String(props.personaName || "").trim();
  if (visibleQueueEvents.value.some((event) => event.queueMode === "guided")) {
    return t("chat.placeholderGuided", { personaName });
  }
  if (visibleQueueEvents.value.length > 0) {
    return t("chat.placeholderBusyQueued", { personaName });
  }
  if (props.busy || props.chatting) {
    return t("chat.placeholderBusyIdle", { personaName });
  }
  return t("chat.placeholder", { personaName });
});

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;
const SEND_MODE_STORAGE_KEY = "easy_call.send_mode.v1";
type SendMode = "enter" | "ctrl_enter";
const composerRootRef = ref<HTMLDivElement | null>(null);
const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const chatInputComposing = ref(false);
const chatInputCompositionEndedAt = ref(0);

const sendMode = ref<SendMode>("enter");
const sendModeMenuOpen = ref(false);
const sendModeMenuRef = ref<HTMLDivElement | null>(null);

/** 手机窄屏（触摸）下的单行输入条：默认收起为单行，展开为完整输入模式。 */
function loadSendMode() {
  try {
    const raw = window.localStorage.getItem(SEND_MODE_STORAGE_KEY);
    if (raw === "ctrl_enter") sendMode.value = "ctrl_enter";
  } catch {
    // ignore storage failures
  }
}

function setSendMode(mode: SendMode) {
  sendMode.value = mode;
  sendModeMenuOpen.value = false;
  try {
    window.localStorage.setItem(SEND_MODE_STORAGE_KEY, mode);
  } catch {
    // ignore persistence failures
  }
}

const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const chatInputHistoryApplying = ref(false);
const resizeInputRaf = ref(0);
const instructionPanelOpen = ref(false);
const instructionFocusIndex = ref(0);
const mentionPanelOpen = ref(false);
const mentionQuery = ref("");
const mentionFocusIndex = ref(0);
const mentionRange = ref<{ start: number; end: number } | null>(null);
const mentionPanelScrollRef = ref<HTMLDivElement | null>(null);

const normalizedInstructionPresets = computed(() =>
  (Array.isArray(props.instructionPresets) ? props.instructionPresets : [])
    .map((item) => ({
      id: String(item?.id || "").trim(),
      name: String(item?.prompt || item?.name || "").trim(),
      prompt: String(item?.prompt || item?.name || "").trim(),
    }))
    .filter((item) => !!item.id && !!item.prompt),
);
const localModelOptionId = ref("");

function modelOptionIdFromProps(): string {
  return String(props.preferredChatModelId || "").trim()
    || String(props.conversationCallPrimaryApiConfigId || "").trim();
}

watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    localModelOptionId.value = modelOptionIdFromProps();
  },
  { immediate: true },
);

watch(
  () => [
    String(props.preferredChatModelId || "").trim(),
    String(props.conversationCallPrimaryApiConfigId || "").trim(),
  ].join("|"),
  () => {
    localModelOptionId.value = modelOptionIdFromProps();
  },
);

// 传给 ChatModelPicker 的当前模型 id：本地选择为空时回退会话主模型
const activeModelDisplayId = computed(() =>
  localModelOptionId.value || String(props.conversationCallPrimaryApiConfigId || "").trim());
const attachedIdeContextReferenceIds = computed(() => new Set((props.attachedIdeContextReferences || []).map((item) => item.id)));
const mergedIdeContextGroups = computed<IdeContextWorkspaceGroup[]>(() => mergeComposerIdeContextGroups(
  props.ideContextGroups || [],
  props.attachedIdeContextReferences || [],
));
function normalizedComposerPathKey(value: string): string {
  return String(value || "").trim().replace(/\\/g, "/").toLowerCase();
}
const mergedIdeContextPathKeys = computed(() => new Set(
  mergedIdeContextGroups.value
    .flatMap((group) => group.references || [])
    .map((item) => normalizedComposerPathKey(item.filePath || item.relativePath || ""))
    .filter(Boolean),
));
const visibleQueuedAttachmentNotices = computed(() =>
  (Array.isArray(props.queuedAttachmentNotices) ? props.queuedAttachmentNotices : []).filter((item) => {
    const pathKey = normalizedComposerPathKey(item.path || "");
    return !pathKey || !mergedIdeContextPathKeys.value.has(pathKey);
  }),
);

function isIdeContextAttached(referenceId: string): boolean {
  return attachedIdeContextReferenceIds.value.has(referenceId);
}

function toggleIdeContextReference(item: IdeContextReferenceItem) {
  if (isIdeContextAttached(item.id)) {
    emit("removeIdeContextReference", item.id);
  } else {
    emit("attachIdeContextReference", item);
  }
  void nextTick(() => focusInput({ preventScroll: true }));
}

/** 输入卡附件适配：图片 / 文件 / IDE 桥胶囊。 */
const panelAttachmentImages = computed(() =>
  (Array.isArray(props.clipboardImages) ? props.clipboardImages : []).map((img, idx) => ({
    mime: String(img?.mime || ""),
    label: t("chat.image", { index: idx + 1 }),
    previewDataUrl: clipboardImagePreviewSrc(img),
  })),
);
const panelAttachmentFiles = computed(() =>
  visibleQueuedAttachmentNotices.value.map((file) => ({
    id: String(file.id || ""),
    fileName: String(file.fileName || ""),
    pending: !!file.pending,
  })),
);
const panelBridgeItems = computed(() =>
  mergedIdeContextGroups.value.flatMap((group) => group.references || []).map((item) => {
    const parts = ideContextReferenceDisplayParts(item);
    return {
      id: String(item.id || ""),
      fileName: String(parts.fileName || ""),
      lineSuffix: String(parts.lineSuffix || ""),
      title: ideContextReferenceTitle(item),
      attached: isIdeContextAttached(item.id),
    };
  }),
);

function removePanelAttachmentFile(index: number) {
  const target = panelAttachmentFiles.value[index];
  if (!target) return;
  const realIndex = (Array.isArray(props.queuedAttachmentNotices) ? props.queuedAttachmentNotices : [])
    .findIndex((item) => String(item.id || "") === String(target.id || ""));
  if (realIndex < 0) return;
  emit("removeQueuedAttachmentNotice", realIndex);
  void nextTick(() => focusInput({ preventScroll: true }));
}

function togglePanelBridge(id: string) {
  const targetId = String(id || "").trim();
  if (!targetId) return;
  const target = mergedIdeContextGroups.value
    .flatMap((group) => group.references || [])
    .find((item) => String(item.id || "") === targetId);
  if (!target) return;
  toggleIdeContextReference(target);
}

function ideContextReferenceTitle(item: IdeContextReferenceItem): string {
  const relativePath = String(item.relativePath || "").trim();
  const startLine = Number(item.startLine || 0);
  const endLine = Number(item.endLine || 0);
  if (!relativePath) return String(item.displayLabel || "").trim();
  if (startLine > 0 && endLine > startLine) {
    return `${relativePath}:${startLine}-${endLine}`;
  }
  if (startLine > 0) {
    return `${relativePath}:${startLine}`;
  }
  return relativePath;
}

const showStopAction = computed(() =>
  (props.chatting || ["queued", "waiting", "streaming"].includes(String(props.frontendRoundPhase || "idle")))
  && composerInputBlank.value,
);
const selectedMentions = computed(() =>
  (Array.isArray(props.selectedMentions) ? props.selectedMentions : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      departmentId: String(item?.departmentId || "").trim(),
      departmentName: String(item?.departmentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.departmentId && !!item.agentName),
);
const filteredMentionOptions = computed<MentionOptionView[]>(() => {
  const query = mentionQuery.value.trim().toLowerCase();
  return (Array.isArray(props.mentionEntries) ? props.mentionEntries : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      departmentId: String(item?.departmentId || "").trim(),
      departmentName: String(item?.departmentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
      mentionable: !!item?.mentionable,
      hidden: !!item?.hidden,
      unavailableReason: String(item?.unavailableReason || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.agentName && !item.hidden)
    .filter((item) => {
      if (!query) return true;
      if (item.agentName.toLowerCase().includes(query)) return true;
      if (item.departmentName && item.departmentName.toLowerCase().includes(query)) return true;
      return false;
    });
});

const planModeToggleAllowed = computed(() => !props.frozen);

function loadChatInputHistory() {
  try {
    const raw = window.localStorage.getItem(CHAT_INPUT_HISTORY_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    const normalized: string[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
      const text = String(item || "").trim();
      if (!text || seen.has(text)) continue;
      seen.add(text);
      normalized.push(text);
      if (normalized.length >= CHAT_INPUT_HISTORY_LIMIT) break;
    }
    chatInputHistory.value = normalized;
  } catch {
    chatInputHistory.value = [];
  }
}

function saveChatInputHistory() {
  try {
    window.localStorage.setItem(CHAT_INPUT_HISTORY_STORAGE_KEY, JSON.stringify(chatInputHistory.value));
  } catch {
    // ignore persistence failures
  }
}

function pushChatInputHistory(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  chatInputHistory.value = [text, ...chatInputHistory.value.filter((item) => item !== text)].slice(0, CHAT_INPUT_HISTORY_LIMIT);
  saveChatInputHistory();
  chatInputHistoryCursor.value = -1;
  chatInputHistoryDraft.value = "";
}

function openInstructionPanel() {
  instructionPanelOpen.value = true;
  if (instructionFocusIndex.value >= normalizedInstructionPresets.value.length) {
    instructionFocusIndex.value = Math.max(0, normalizedInstructionPresets.value.length - 1);
  }
}

function closeInstructionPanel() {
  instructionPanelOpen.value = false;
}

function closeMentionPanel() {
  mentionPanelOpen.value = false;
  mentionQuery.value = "";
  mentionFocusIndex.value = 0;
  mentionRange.value = null;
}

function toggleInstructionPanel() {
  if (instructionPanelOpen.value) {
    closeInstructionPanel();
    return;
  }
  openInstructionPanel();
}

function buildInstructionPresetInput(currentText: string, prompt: string): string {
  const current = String(currentText || "");
  const nextPrompt = String(prompt || "").trim();
  if (!nextPrompt) return current;
  if (!current) return nextPrompt;
  return `${current}\n\n${nextPrompt}`;
}

function applyInstructionPreset(item: PromptCommandPreset | undefined) {
  if (!item) return;
  const prompt = String(item.prompt || item.name || "").trim();
  if (!prompt) return;
  const nextValue = buildInstructionPresetInput(localChatInput.value, prompt);
  localChatInput.value = nextValue;
  closeInstructionPanel();
  closeMentionPanel();
  nextTick(() => {
    scheduleResizeChatInput();
    const el = chatInputRef.value;
    if (!el) return;
    el.focus({ preventScroll: true });
    const cursor = nextValue.length;
    el.setSelectionRange(cursor, cursor);
  });
}

function selectInstructionPresetByIndex(index: number) {
  const list = normalizedInstructionPresets.value;
  if (list.length === 0) return;
  const nextIndex = Math.max(0, Math.min(list.length - 1, index));
  instructionFocusIndex.value = nextIndex;
  applyInstructionPreset(list[nextIndex]);
}

function moveInstructionFocus(delta: number) {
  const list = normalizedInstructionPresets.value;
  if (list.length === 0) return;
  const next = instructionFocusIndex.value + delta;
  instructionFocusIndex.value = Math.max(0, Math.min(list.length - 1, next));
}

function removeSelectedMention(item: ChatMentionTarget | undefined) {
  if (!item) return;
  emit("removeMention", {
    agentId: String(item.agentId || "").trim(),
    departmentId: String(item.departmentId || "").trim() || undefined,
  });
  closeMentionPanel();
}

function applyMention(item: MentionOptionView | undefined) {
  if (!item || !item.mentionable || !mentionRange.value) return;
  const current = String(localChatInput.value || "");
  const before = current.slice(0, mentionRange.value.start);
  const after = current.slice(mentionRange.value.end);
  const nextValue = `${before}${after}`;
  localChatInput.value = nextValue;
  if (selectedMentions.value.some((entry) =>
    String(entry.agentId || "").trim() === String(item.agentId || "").trim()
    && String(entry.departmentId || "").trim() === String(item.departmentId || "").trim()
  )) {
    emit("removeMention", {
      agentId: String(item.agentId || "").trim(),
      departmentId: String(item.departmentId || "").trim() || undefined,
    });
  } else {
    emit("addMention", {
      agentId: String(item.agentId || "").trim(),
      agentName: String(item.agentName || "").trim(),
      departmentId: String(item.departmentId || "").trim(),
      departmentName: String(item.departmentName || "").trim(),
      avatarUrl: String(item.avatarUrl || "").trim() || undefined,
    });
  }
  closeMentionPanel();
  nextTick(() => {
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = Math.min(before.length, nextValue.length);
    el.focus();
    el.setSelectionRange(cursor, cursor);
    scheduleResizeChatInput();
  });
}

function selectMentionByIndex(index: number) {
  const list = filteredMentionOptions.value;
  if (list.length === 0) return;
  const nextIndex = Math.max(0, Math.min(list.length - 1, index));
  const target = list[nextIndex];
  if (!target.mentionable) return;
  mentionFocusIndex.value = nextIndex;
  applyMention(target);
}

function moveMentionFocus(delta: number) {
  const list = filteredMentionOptions.value;
  if (list.length === 0) return;
  let next = mentionFocusIndex.value + delta;
  while (next >= 0 && next < list.length && !list[next].mentionable) {
    next += delta;
  }
  if (next < 0 || next >= list.length) return;
  mentionFocusIndex.value = next;
  scrollMentionFocusIntoView();
}

function scrollMentionFocusIntoView() {
  nextTick(() => {
    const container = mentionPanelScrollRef.value;
    if (!container) return;
    const active = container.querySelector<HTMLElement>(`[data-mention-option-index="${mentionFocusIndex.value}"]`);
    active?.scrollIntoView({ block: "nearest" });
  });
}

function updateMentionState() {
  const el = chatInputRef.value;
  if (!el || el.selectionStart !== el.selectionEnd) {
    closeMentionPanel();
    return;
  }
  const value = String(localChatInput.value || "");
  const cursor = el.selectionStart ?? value.length;
  const beforeCursor = value.slice(0, cursor);
  const match = beforeCursor.match(/(?:^|\s)@([^\s@]*)$/);
  if (!match) {
    closeMentionPanel();
    return;
  }
  const query = match[1];
  mentionQuery.value = query;
  const atStart = cursor - 1 - query.length;
  mentionRange.value = { start: atStart, end: cursor };
  mentionPanelOpen.value = true;
  const firstMentionable = filteredMentionOptions.value.findIndex((item) => item.mentionable);
  mentionFocusIndex.value = firstMentionable >= 0 ? firstMentionable : 0;
}

function selectConversationPreferredModel(id: string) {
  const nextId = String(id || "").trim();
  if (!nextId || nextId === activeModelDisplayId.value) return;
  localModelOptionId.value = nextId;
  emit("update:conversationPreferredApiConfigId", nextId);
}

function togglePlanMode() {
  if (!planModeToggleAllowed.value) return;
  emit("update:planModeEnabled", !props.planModeEnabled);
}

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  // 单布局：空态单行高度，有字随内容增高，上限 160。
  const minHeight = 24;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.max(Math.min(el.scrollHeight, maxHeight), minHeight);
  el.style.height = `${nextHeight}px`;
  el.style.overflowY = "auto";
}

function handleChatInputInput() {
  scheduleResizeChatInput();
  updateMentionState();
}

function handleChatInputCompositionStart() {
  chatInputComposing.value = true;
}

function handleChatInputCompositionEnd() {
  chatInputComposing.value = false;
  chatInputCompositionEndedAt.value = performance.now();
}

function scheduleResizeChatInput() {
  if (resizeInputRaf.value) cancelAnimationFrame(resizeInputRaf.value);
  resizeInputRaf.value = requestAnimationFrame(() => {
    resizeChatInput();
    resizeInputRaf.value = 0;
  });
}

function applyChatInputHistoryValue(value: string) {
  chatInputHistoryApplying.value = true;
  localChatInput.value = value;
  nextTick(() => {
    chatInputHistoryApplying.value = false;
    scheduleResizeChatInput();
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = value.length;
    el.setSelectionRange(cursor, cursor);
  });
}

function canNavigateHistory(el: HTMLTextAreaElement, direction: "up" | "down"): boolean {
  if (el.selectionStart !== el.selectionEnd) return false;
  if (direction === "up") return el.selectionStart === 0;
  return el.selectionStart === el.value.length;
}

function navigateChatInputHistory(direction: "up" | "down"): boolean {
  const list = chatInputHistory.value;
  if (list.length === 0) return false;
  if (direction === "up") {
    if (chatInputHistoryCursor.value === -1) {
      chatInputHistoryDraft.value = localChatInput.value;
      chatInputHistoryCursor.value = 0;
      applyChatInputHistoryValue(list[0]);
      return true;
    }
    if (chatInputHistoryCursor.value < list.length - 1) {
      chatInputHistoryCursor.value += 1;
      applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
      return true;
    }
    return false;
  }
  if (chatInputHistoryCursor.value === -1) return false;
  if (chatInputHistoryCursor.value === 0) {
    chatInputHistoryCursor.value = -1;
    const draft = chatInputHistoryDraft.value;
    chatInputHistoryDraft.value = "";
    applyChatInputHistoryValue(draft);
    return true;
  }
  chatInputHistoryCursor.value -= 1;
  applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
  return true;
}

function recordSentTextIfNeeded(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  setTimeout(() => {
    if (String(props.chatInput || "").trim()) return;
    pushChatInputHistory(text);
  }, 0);
}

function handleSendChat() {
  const plainText = String(localChatInput.value || "").trim();
  emit("sendChat");
  recordSentTextIfNeeded(plainText);
  closeInstructionPanel();
  closeMentionPanel();
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.repeat) return;
  if (event.key !== "Tab" || !event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return;
  if (!planModeToggleAllowed.value) return;
  const activeElement = document.activeElement;
  const textareaFocused = !!chatInputRef.value && activeElement === chatInputRef.value;
  const composerFocused = !!composerRootRef.value && activeElement === composerRootRef.value;
  if (!textareaFocused && !composerFocused) return;
  event.preventDefault();
  togglePlanMode();
}

function handleChatInputFocus() {
  if (props.composerScope) {
    registerChatComposerFocus(props.composerScope);
  }
}

function handleChatInputBlur() {
  if (props.composerScope) {
    clearChatComposerFocus(props.composerScope);
  }
}

function handleChatInputKeydown(event: KeyboardEvent) {
  if (
    chatInputEnterConfirmsComposition(
      event,
      chatInputComposing.value,
      chatInputCompositionEndedAt.value,
      performance.now(),
    )
  ) {
    return;
  }
  if (event.key === "Escape") {
    const hadMention = mentionPanelOpen.value;
    const hadInstruction = instructionPanelOpen.value;
    const hadSendMode = sendModeMenuOpen.value;
    if (hadMention || hadInstruction || hadSendMode) {
      event.preventDefault();
      if (hadMention) closeMentionPanel();
      if (hadInstruction) closeInstructionPanel();
      else instructionPanelOpen.value = false;
      if (hadSendMode) sendModeMenuOpen.value = false;
      return;
    }
  }
  if (mentionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMentionPanel();
      instructionPanelOpen.value = false;
      sendModeMenuOpen.value = false;
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveMentionFocus(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveMentionFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      selectMentionByIndex(mentionFocusIndex.value);
      return;
    }
  }
  if (event.key === "Tab" && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    toggleInstructionPanel();
    return;
  }
  if (instructionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeInstructionPanel();
      return;
    }
    if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
      event.preventDefault();
      moveInstructionFocus(-1);
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowRight") {
      event.preventDefault();
      moveInstructionFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      selectInstructionPresetByIndex(instructionFocusIndex.value);
      return;
    }
  }
  if (event.key === "Escape" && props.chatting && showStopAction.value && !props.stopChatDisabled) {
    event.preventDefault();
    emit("stopChat");
    return;
  }
  const ctrlEnterPressed = event.key === "Enter" && event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  const plainEnterPressed = event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  // Alt+S 兜底发送：无论当前发送模式，始终可用（输入法组合中除外）
  const altSPressed = event.key.toLowerCase() === "s" && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.isComposing;
  if (altSPressed) {
    if (props.frozen) return;
    event.preventDefault();
    handleSendChat();
    return;
  }
  if (sendMode.value === "ctrl_enter") {
    if (ctrlEnterPressed) {
      if (props.frozen) return;
      event.preventDefault();
      handleSendChat();
      return;
    }
    // Ctrl+Enter 模式：普通 Enter 保留为换行
    if (plainEnterPressed) return;
  } else if (plainEnterPressed) {
    if (props.frozen) return;
    event.preventDefault();
    handleSendChat();
    return;
  }
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
  const el = chatInputRef.value;
  if (!el) return;
  const direction = event.key === "ArrowUp" ? "up" : "down";
  if (!canNavigateHistory(el, direction)) return;
  if (navigateChatInputHistory(direction)) {
    event.preventDefault();
  }
}

function clipboardImagePreviewSrc(image: BinaryAttachment): string {
  const previewDataUrl = String(image?.previewDataUrl || "").trim();
  if (previewDataUrl.startsWith("data:image/")) return previewDataUrl;
  const mime = String(image?.mime || "").trim().toLowerCase();
  const bytesBase64 = String(image?.bytesBase64 || "").trim();
  if (!mime.startsWith("image/") || !bytesBase64) return "";
  return `data:${mime};base64,${bytesBase64}`;
}

function removeClipboardImageAt(index: number) {
  emit("removeClipboardImage", index);
  void nextTick(() => focusInput({ preventScroll: true }));
}

function avatarInitial(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function mentionDisplayLabel(target: Pick<ChatMentionTarget, "agentName" | "departmentName">): string {
  const agentName = String(target?.agentName || "").trim();
  const departmentName = String(target?.departmentName || "").trim();
  if (!departmentName) return agentName;
  return `${agentName} / ${departmentName}`;
}

function isMentionSelected(target: Pick<ChatMentionTarget, "agentId" | "departmentId"> | undefined): boolean {
  const agentId = String(target?.agentId || "").trim();
  const departmentId = String(target?.departmentId || "").trim();
  if (!agentId || !departmentId) return false;
  return selectedMentions.value.some((item) =>
    String(item.agentId || "").trim() === agentId
    && String(item.departmentId || "").trim() === departmentId
  );
}

async function handleRecallToInput(event: {
  source?: string;
  messagePreview?: string;
  messageText?: string;
  id?: string;
  queueMode?: "normal" | "guided";
}) {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    emit("queueRecall", event as ChatQueueEvent);
    return;
  }
  if (event.source === "user" && event.queueMode !== "guided") {
    if (event.id) {
      const result = await recallQueueEvent(event.id);
      if (result.removed) {
        localChatInput.value = result.messageText || event.messageText || event.messagePreview || "";
      }
    }
  }
}

function handleQueueMarkGuided(eventId: string) {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    emit("queueMarkGuided", eventId);
    return;
  }
  markGuided(eventId);
}

function focusInput(options?: FocusOptions) {
  chatInputRef.value?.focus(options);
}

defineExpose({
  focusInput,
});

onMounted(() => {
  loadChatInputHistory();
  loadSendMode();
  window.addEventListener("keydown", handleWindowKeydown);
  nextTick(() => {
    resizeChatInput();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
  if (resizeInputRaf.value) {
    cancelAnimationFrame(resizeInputRaf.value);
    resizeInputRaf.value = 0;
  }
});

watch(
  () => props.chatInput,
  (nextValue, prevValue) => {
    if (!chatInputHistoryApplying.value && nextValue !== prevValue && chatInputHistoryCursor.value !== -1) {
      chatInputHistoryCursor.value = -1;
      chatInputHistoryDraft.value = "";
    }
    nextTick(() => scheduleResizeChatInput());
    nextTick(() => {
      updateMentionState();
    });
  },
);

watch(
  () => props.activeConversationId,
  () => {
    closeInstructionPanel();
    closeMentionPanel();
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => normalizedInstructionPresets.value,
  (list) => {
    if (list.length === 0) {
      instructionFocusIndex.value = 0;
      instructionPanelOpen.value = false;
      return;
    }
    if (instructionFocusIndex.value >= list.length) {
      instructionFocusIndex.value = list.length - 1;
    }
  },
  { deep: true },
);

watch(
  () => props.selectedMentions.map((item) => `${item.agentId}:${item.departmentId}`).join("|"),
  () => {
    closeMentionPanel();
  },
);
</script>

<style scoped>
.chat-input-no-focus::-webkit-scrollbar {
  display: none;
}
.chat-input-no-focus {
  scrollbar-width: none;
}
</style>
