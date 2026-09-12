<template>
  <div
    v-bind="attrs"
    class="rounded-box border border-base-300/50 bg-base-100/55 px-2 py-1.5 shadow backdrop-blur-md backdrop-saturate-150 flex items-center justify-between gap-2 text-xs"
  >
    <div class="flex min-w-0 flex-1 items-center gap-1.5">
      <div
        v-if="!hideMenuButton"
        ref="menuDropdownRef"
        class="dropdown dropdown-start"
        :class="menuPlacement === 'top' ? 'dropdown-top' : 'dropdown-bottom'"
        @focusout="handleMenuFocusOut"
      >
        <button
          ref="menuButtonRef"
          type="button"
          tabindex="0"
          class="btn btn-sm btn-ghost btn-circle shrink-0"
          :title="t('chat.conversationMenu.title')"
          @mousedown="updateMenuPlacement"
        >
          <Grip class="h-4 w-4" />
        </button>
        <ul
          tabindex="0"
          class="dropdown-content menu z-50 w-72 max-h-[calc(100vh-8rem)] overflow-y-auto rounded-box border border-base-300 bg-base-100 p-2 text-sm shadow-xl"
          :class="menuPlacement === 'top' ? 'mb-3' : 'mt-3'"
        >
          <li v-if="showTaskCreateMenuItem">
            <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openTaskCreate')">
              <ListTodo class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.newTask") }}</span>
            </button>
          </li>
          <li v-if="showShareMenuItem">
            <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openShareSelection')">
              <Share2 class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.conversationMenu.shareConversation") }}</span>
            </button>
          </li>
          <li v-if="hasDelegateMenuItems">
            <details
              :open="activeSubmenu === 'delegate'"
              @toggle="handleDetailsToggle($event, 'delegate')"
            >
              <summary>
                <Users class="h-4 w-4 shrink-0" />
                <span class="leading-5">{{ t("chat.conversationMenu.groupDelegate") }}</span>
              </summary>
              <ul ref="delegateSubmenuEl">
              <li v-if="showCodeReviewMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openCodeReview')">
                  <ClipboardCheck class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t('chat.toolbar.codeReview') }}</span>
                </button>
              </li>
              <li v-if="showDelegateMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openDelegateSelection')">
                  <ClipboardList class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.startDelegate") }}</span>
                </button>
              </li>
              </ul>
            </details>
          </li>
          <li v-if="hasBranchMenuItems">
            <details
              :open="activeSubmenu === 'branch'"
              @toggle="handleDetailsToggle($event, 'branch')"
            >
              <summary>
                <Split class="h-4 w-4 shrink-0" />
                <span class="leading-5">{{ t("chat.conversationMenu.groupBranch") }}</span>
              </summary>
              <ul ref="branchSubmenuEl">
              <li v-if="showBranchMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openBranchFromCurrent')">
                  <GitBranch class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.branchFromCurrent") }}</span>
                </button>
              </li>
              <li v-if="showBranchMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openBranchSelection')">
                  <GitBranchPlus class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.branchConversation") }}</span>
                </button>
              </li>
              <li v-if="sideChatEnabled">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openSideChat')">
                  <MessageSquareMore class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.sideChatFollowUp") }}</span>
                </button>
              </li>
              </ul>
            </details>
          </li>
          <li v-if="hasInteractionMenuItems">
            <details
              :open="activeSubmenu === 'interaction'"
              @toggle="handleDetailsToggle($event, 'interaction')"
            >
              <summary>
                <Send class="h-4 w-4 shrink-0" />
                <span class="leading-5">{{ t("chat.conversationMenu.groupInteraction") }}</span>
              </summary>
              <ul ref="interactionSubmenuEl">
              <li v-if="showAutoPushMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openAutoPush')">
                  <BellRing class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.autoPushToContact") }}</span>
                </button>
              </li>
              <li v-if="showForwardMenuItem">
                <button type="button" class="flex min-h-9 items-center justify-start gap-3 px-3 py-1.5 text-left" @click="emit('openForwardSelection')">
                  <Package class="h-4 w-4 shrink-0" />
                  <span class="leading-5">{{ t("chat.conversationMenu.forwardToContact") }}</span>
                </button>
              </li>
            </ul>
            </details>
          </li>
          <li>
            <details
              :open="activeSubmenu === 'appearance'"
              @toggle="handleDetailsToggle($event, 'appearance')"
            >
              <summary>
                <Palette class="h-4 w-4 shrink-0" />
                <span class="leading-5">{{ t("chat.conversationMenu.groupAppearance") }}</span>
              </summary>
              <ul ref="appearanceSubmenuEl">
              <li class="menu-title px-2 py-1 text-xs uppercase tracking-wide opacity-60">{{ t("appearance.chatBubble") }}</li>
              <li>
                <label class="flex cursor-pointer items-center justify-between gap-3 px-2 py-1.5">
                  <span class="text-sm">{{ t("appearance.chatBubbleBackground") }}</span>
                  <input
                    :checked="assistantBubbleBackgroundEnabled"
                    type="checkbox"
                    class="toggle toggle-sm"
                    @change="setAssistantBubbleBackgroundEnabled(($event.target as HTMLInputElement).checked)"
                  />
                </label>
              </li>
              <li>
                <label class="flex cursor-pointer items-center justify-between gap-3 px-2 py-1.5">
                  <span class="text-sm">{{ t("appearance.chatBubbleSegmentedMarkdown") }}</span>
                  <input
                    :checked="segmentedMarkdownEnabled"
                    type="checkbox"
                    class="toggle toggle-sm"
                    @change="setSegmentedMarkdownEnabled(($event.target as HTMLInputElement).checked)"
                  />
                </label>
              </li>
              <li>
                <label class="flex cursor-pointer items-center justify-between gap-3 px-2 py-1.5">
                  <span class="text-sm">{{ t("appearance.chatBubbleFullTime") }}</span>
                  <input
                    :checked="chatTimeDisplayMode === 'absolute'"
                    type="checkbox"
                    class="toggle toggle-sm"
                    @change="setChatTimeDisplayMode(($event.target as HTMLInputElement).checked ? 'absolute' : 'relative')"
                  />
                </label>
              </li>
              <li>
                <div class="flex flex-col items-stretch gap-1.5 px-2 py-1.5">
                  <span class="text-xs font-medium text-base-content/80">{{ t("appearance.chatBubbleMarkdownLayout") }}</span>
                  <SegmentedControl
                    :model-value="markdownLayout"
                    :options="markdownLayoutOptions"
                    size="xs"
                    :full-width="true"
                    class="w-full"
                    @change="setChatMarkdownLayout"
                  />
                </div>
              </li>
              <li class="menu-title px-2 py-1 text-xs uppercase tracking-wide opacity-60">{{ t("appearance.inputPanel") }}</li>
              <li>
                <label class="flex cursor-pointer items-center justify-between gap-3 px-2 py-1.5">
                  <span class="text-sm">{{ t("appearance.inputPanelIdeBridgeFileTags") }}</span>
                  <input
                    :checked="ideBridgeFileTagsEnabled"
                    type="checkbox"
                    class="toggle toggle-sm"
                    @change="setIdeBridgeFileTagsEnabled(($event.target as HTMLInputElement).checked)"
                  />
                </label>
              </li>
              <li class="menu-title px-2 py-1 text-xs uppercase tracking-wide opacity-60">{{ t("appearance.fileReader") }}</li>
              <li>
                <label class="flex cursor-pointer items-center justify-between gap-3 px-2 py-1.5">
                  <span class="text-sm">{{ t("appearance.fileReaderLineWrap") }}</span>
                  <input
                    :checked="fileReaderLineWrapEnabled"
                    type="checkbox"
                    class="toggle toggle-sm"
                    @change="setFileReaderLineWrapEnabled(($event.target as HTMLInputElement).checked)"
                  />
                </label>
              </li>
            </ul>
            </details>
          </li>
        </ul>
      </div>
      <SessionControlPanel
        v-if="showSessionControlPanel"
        class="min-w-0 flex-1"
        :workspace-button-label="workspaceButtonLabel"
        :workspace-button-name="workspaceButtonName"
        :workspace-button-disabled="workspaceButtonDisabled"
        :workspace-permission-kind="workspacePermissionKind"
        :auto-push-active="autoPushActive"
        :delegates="delegateStatuses || []"
        :running-task-count="runningTaskCount"
        :running-shell-count="runningShellCount"
        @lock-workspace="emit('lockWorkspace')"
        @open-delegate-summary="emit('openDelegateSummary')"
      />
    </div>
    <div class="flex min-w-0 items-center justify-end gap-1.5">
      <button
        v-if="uniqueMentionEntries.length > 0"
        ref="mentionListButtonRef"
        type="button"
        class="btn btn-ghost btn-sm btn-circle shrink-0 border-0 bg-transparent shadow-none hover:bg-base-200"
        :title="t('chat.toolbar.personaList')"
        @click="toggleMentionListPopup"
      >
        <span class="text-base font-semibold leading-none">@</span>
      </button>
    </div>
  </div>
  <Teleport to="body">
    <div
      v-if="mentionListPopupOpen"
      ref="mentionListPopupRef"
      class="fixed z-1200"
      :style="mentionListPopupStyle"
    >
      <div class="relative overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl">
        <OverlayScrollArea
          ref="mentionListAreaRef"
          scroller-class="ecall-toolbar-mention-scroll max-h-[min(56vh,24rem)] min-w-56 max-w-[min(80vw,20rem)] overscroll-contain p-1"
        >
          <ul class="flex flex-col gap-1">
            <li
              v-for="entry in uniqueMentionEntries"
              :key="entry.agentId"
            >
              <button
                type="button"
                class="flex min-h-0 w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors hover:bg-base-200/80"
                :disabled="chatting || frozen"
                @click="handleCompactPersonaEntryClick($event, entry)"
              >
                <div class="indicator shrink-0">
                  <span
                    v-if="entry.selected"
                    class="indicator-item indicator-top indicator-end inline-flex h-4 w-4 translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-primary text-micro font-bold text-primary-content"
                  >
                    @
                  </span>
                  <span
                    v-else-if="entry.hasBackgroundTask"
                    class="indicator-item indicator-bottom indicator-end inline-flex min-w-5 translate-x-1/4 translate-y-1/4 items-center justify-center rounded-full border border-base-300 bg-base-100 px-1 py-0.5 text-micro text-base-content shadow-sm"
                  >
                    <span class="loading loading-dots loading-xs"></span>
                  </span>
                  <div class="avatar">
                    <div class="w-7 rounded-full">
                      <img
                        v-if="entry.avatarUrl"
                        :src="entry.avatarUrl"
                        :alt="entry.agentName"
                        class="w-7 h-7 rounded-full object-cover"
                        :class="frontSpeakingMuted(entry) ? 'grayscale opacity-75' : ''"
                      />
                      <div
                        v-else
                        class="w-7 h-7 rounded-full flex items-center justify-center text-caption"
                        :class="frontSpeakingMuted(entry)
                          ? 'bg-base-300 text-base-content/70'
                          : 'bg-neutral text-neutral-content'"
                      >
                        {{ avatarInitial(entry.agentName) }}
                      </div>
                    </div>
                  </div>
                </div>
                <div class="min-w-0 flex-1 pr-0.5">
                  <div class="truncate text-sm leading-5">@{{ entry.agentName }}</div>
                  <div class="truncate text-xs leading-4 text-base-content/60">
                    {{ entry.departmentName || t("chat.defaultDepartment") }}
                  </div>
                </div>
              </button>
            </li>
          </ul>
        </OverlayScrollArea>
      </div>
    </div>
  </Teleport>
  <Teleport to="body">
    <div
      v-if="avatarPopupTarget"
      class="fixed z-1200"
      :style="avatarPopupStyle"
    >
      <div
        ref="avatarPopupPanelRef"
        class="relative overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
      >
        <OverlayScrollArea
          ref="avatarPopupAreaRef"
          scroller-class="ecall-toolbar-mention-scroll max-h-[min(56vh,24rem)] w-max max-w-[min(80vw,20rem)] overscroll-contain p-1"
        >
        <ul class="flex flex-col gap-1">
          <li
            v-for="entry in filteredAvatarPopupOptions"
            :key="`${entry.agentId}:${entry.departmentId}`"
          >
            <button
              type="button"
              class="flex min-h-0 w-full items-start gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors hover:bg-base-200/80"
              @click="applyAvatarPopupSelection(entry)"
            >
              <div class="avatar shrink-0">
                <div class="w-7 rounded-full">
                  <img
                    v-if="entry.avatarUrl"
                    :src="entry.avatarUrl"
                    :alt="entry.agentName"
                    class="w-7 h-7 rounded-full object-cover"
                  />
                  <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-caption">
                    {{ avatarInitial(entry.agentName) }}
                  </div>
                </div>
              </div>
              <div class="min-w-0 flex-1 pr-0.5">
                <div class="truncate text-sm leading-5">@{{ entry.agentName }}</div>
                <div class="truncate text-xs leading-4 text-base-content/60">
                  {{ entry.departmentName || t("chat.defaultDepartment") }}
                </div>
              </div>
            </button>
          </li>
        </ul>
        </OverlayScrollArea>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useAttrs, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { BellRing, ClipboardCheck, ClipboardList, GitBranch, GitBranchPlus, Grip, ListTodo, MessageSquareMore, Package, Palette, Send, Share2, Split, Users } from "@lucide/vue";
import type { ChatMentionEntry, ConversationDelegateStatusSummary } from "../../../types/app";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import { useChatComposerAppearance } from "../../shell/composables/use-chat-composer-appearance";
import { useChatMessageAppearance, type ChatMarkdownLayout } from "../../shell/composables/use-chat-message-appearance";
import { useFileReaderAppearance } from "../../shell/composables/use-file-reader-appearance";
import SessionControlPanel from "./SessionControlPanel.vue";
import SegmentedControl from "../../config/components/SegmentedControl.vue";

defineOptions({
  inheritAttrs: false,
});

const props = withDefaults(defineProps<{
  chatting: boolean;
  frozen: boolean;
  conversationBusy?: boolean;
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  workspaceButtonDisabled?: boolean;
  workspacePermissionKind?: "approval" | "full_access" | "autonomous";
  autoPushActive?: boolean;
  mentionEntries: ChatMentionEntry[];
  selectedMentionKeys: string[];
  hideMenuButton?: boolean;
  hideWorkspaceButton?: boolean;
  showTaskCreateMenuItem?: boolean;
  showDelegateMenuItem?: boolean;
  showBranchMenuItem?: boolean;
  showCodeReviewMenuItem?: boolean;
  showForwardMenuItem?: boolean;
  showAutoPushMenuItem?: boolean;
  showShareMenuItem?: boolean;
  showWorkspaceMenuItem?: boolean;
  showOpenInBrowserButton?: boolean;
  openInBrowserDisabled?: boolean;
  sideChatEnabled?: boolean;
  delegateStatuses?: ConversationDelegateStatusSummary[];
  runningTaskCount?: number;
  runningShellCount?: number;
}>(), {
  showTaskCreateMenuItem: true,
  showDelegateMenuItem: true,
  showBranchMenuItem: true,
  showCodeReviewMenuItem: true,
  showForwardMenuItem: true,
  showAutoPushMenuItem: true,
  showShareMenuItem: true,
  showWorkspaceMenuItem: true,
  showOpenInBrowserButton: false,
});

const emit = defineEmits<{
  (e: "lockWorkspace"): void;
  (e: "openBranchSelection"): void;
  (e: "openCodeReview"): void;
  (e: "openTaskCreate"): void;
  (e: "openDelegateSelection"): void;
  (e: "openDelegateSummary"): void;
  (e: "openForwardSelection"): void;
  (e: "openAutoPush"): void;
  (e: "openShareSelection"): void;
  (e: "openConversationInBrowser"): void;
  (e: "openBranchFromCurrent"): void;
  (e: "openSideChat"): void;
  (e: "mentionEntry", entry: ChatMentionEntry): void;
}>();

const attrs = useAttrs();
const { t } = useI18n();
const { ideBridgeFileTagsEnabled, setIdeBridgeFileTagsEnabled } = useChatComposerAppearance();
const {
  assistantBubbleBackgroundEnabled,
  segmentedMarkdownEnabled,
  chatTimeDisplayMode,
  markdownLayout,
  setAssistantBubbleBackgroundEnabled,
  setSegmentedMarkdownEnabled,
  setChatTimeDisplayMode,
  setChatMarkdownLayout,
} = useChatMessageAppearance();
const markdownLayoutOptions = computed<Array<{ value: ChatMarkdownLayout; label: string }>>(() => [
  { value: "compact", label: t("appearance.markdownLayoutCompact") },
  { value: "comfortable", label: t("appearance.markdownLayoutComfortable") },
  { value: "relaxed", label: t("appearance.markdownLayoutRelaxed") },
]);
const {
  fileReaderLineWrapEnabled,
  setFileReaderLineWrapEnabled,
} = useFileReaderAppearance();
const busy = computed(() => props.chatting || props.frozen || !!props.conversationBusy);
const showTaskCreateMenuItem = computed(() => props.showTaskCreateMenuItem);
const showDelegateMenuItem = computed(() => props.showDelegateMenuItem);
const showBranchMenuItem = computed(() => props.showBranchMenuItem);
const showCodeReviewMenuItem = computed(() => props.showCodeReviewMenuItem);
const showForwardMenuItem = computed(() => props.showForwardMenuItem);
const showAutoPushMenuItem = computed(() => props.showAutoPushMenuItem);
const showShareMenuItem = computed(() => props.showShareMenuItem);
const showWorkspaceMenuItem = computed(() => props.showWorkspaceMenuItem);
type SubmenuKey = "delegate" | "branch" | "interaction" | "appearance";
const activeSubmenu = ref<SubmenuKey | null>(null);
/** details 原生展开/收起与 activeSubmenu 双向同步，保证同时只展开一组 */
function handleDetailsToggle(event: ToggleEvent, key: SubmenuKey) {
  const details = event.target as HTMLDetailsElement | null;
  if (details?.open) {
    activeSubmenu.value = key;
  } else if (activeSubmenu.value === key) {
    activeSubmenu.value = null;
  }
}
const delegateSubmenuEl = ref<HTMLElement | null>(null);
const branchSubmenuEl = ref<HTMLElement | null>(null);
const interactionSubmenuEl = ref<HTMLElement | null>(null);
const appearanceSubmenuEl = ref<HTMLElement | null>(null);
const submenuEls: Record<SubmenuKey, Ref<HTMLElement | null>> = {
  delegate: delegateSubmenuEl,
  branch: branchSubmenuEl,
  interaction: interactionSubmenuEl,
  appearance: appearanceSubmenuEl,
};

const menuDropdownRef = ref<HTMLElement | null>(null);

/** 目标是否在整套菜单树内（一级菜单容器 + 各二级菜单容器） */
function isInsideMenuTree(target: Node | null): boolean {
  if (!target) return false;
  if (menuDropdownRef.value?.contains(target)) return true;
  for (const key of Object.keys(submenuEls) as SubmenuKey[]) {
    const el = submenuEls[key].value;
    if (el?.contains(target)) return true;
  }
  return false;
}

/** 焦点离开菜单树（含一级菜单整体被关闭）时清理二级菜单，避免残留 */
function handleMenuFocusOut(event: FocusEvent) {
  const nextTarget = event.relatedTarget;
  if (nextTarget instanceof Node && isInsideMenuTree(nextTarget)) return;
  activeSubmenu.value = null;
}

/** 点击菜单树外部时清理二级菜单（主菜单被 daisyui 关闭的兜底） */
function handleGlobalPointerDown(event: PointerEvent) {
  if (!(event.target instanceof Node)) return;
  if (!isInsideMenuTree(event.target)) {
    activeSubmenu.value = null;
  }
}

/* 二级菜单：原生 details 可折叠多级菜单。
   子项在文档流内展开，随主菜单一起滚动，不存在飞出视口问题。
   仅点按 summary 展开，无 hover 预展开。 */

const hasDelegateMenuItems = computed(
  () => props.showCodeReviewMenuItem || props.showDelegateMenuItem,
);
const hasBranchMenuItems = computed(
  () => props.showBranchMenuItem || !!props.sideChatEnabled,
);
const hasInteractionMenuItems = computed(
  () => props.showAutoPushMenuItem || props.showForwardMenuItem,
);
const hasDelegateStatuses = computed(() => (props.delegateStatuses || []).length > 0);
const showSessionControlPanel = computed(() => !props.hideWorkspaceButton || hasDelegateStatuses.value);
const POPUP_OFFSET = 8;
const POPUP_VIEWPORT_PADDING = 8;

// ========== 头像栏去重 + 部门弹出 ==========

const mentionListButtonRef = ref<HTMLButtonElement | null>(null);
const mentionListPopupOpen = ref(false);
const mentionListPopupRef = ref<HTMLElement | null>(null);
const mentionListAreaRef = ref<InstanceType<typeof OverlayScrollArea> | null>(null);
const mentionListPopupStyle = ref<Record<string, string>>({
  left: "0px",
  top: "0px",
});

const uniqueMentionEntries = computed(() => {
  const seen = new Map<string, ChatMentionEntry>();
  for (const entry of props.mentionEntries || []) {
    if (!entry.mentionable) continue;
    const agentId = String(entry.agentId || "").trim();
    if (!agentId) continue;
    if (!seen.has(agentId)) {
      seen.set(agentId, { ...entry, selected: false });
    }
  }
  const result = Array.from(seen.values());
  for (const entry of result) {
    const agentId = String(entry.agentId || "").trim();
    entry.selected = agentId ? props.selectedMentionKeys.some((key) => String(key || "").trim().startsWith(`${agentId}:`)) : false;
  }
  return result;
});

const avatarPopupTarget = ref<{
  agentId: string;
  agentName: string;
  avatarUrl?: string;
} | null>(null);
const avatarPopupAnchorEl = ref<HTMLElement | null>(null);
const avatarPopupPanelRef = ref<HTMLElement | null>(null);
const avatarPopupAreaRef = ref<InstanceType<typeof OverlayScrollArea> | null>(null);

const avatarPopupStyle = ref<Record<string, string>>({
  left: "0px",
  top: "0px",
});

const filteredAvatarPopupOptions = computed(() => {
  const target = avatarPopupTarget.value;
  if (!target) return [];
  return (props.mentionEntries || [])
    .filter((entry) => String(entry.agentId || "").trim() === target.agentId && entry.mentionable)
    .map((entry) => ({
      agentId: String(entry.agentId || "").trim(),
      agentName: String(entry.agentName || "").trim(),
      departmentId: String(entry.departmentId || "").trim(),
      departmentName: String(entry.departmentName || "").trim(),
      avatarUrl: String(entry.avatarUrl || "").trim() || undefined,
    }))
    .filter((entry) => !!entry.agentId && !!entry.departmentId);
});

function handleMentionEntryClick(event: MouseEvent, entry: ChatMentionEntry & { selected?: boolean }) {
  const agentId = String(entry.agentId || "").trim();
  const deptEntries = (props.mentionEntries || []).filter(
    (e) => String(e.agentId || "").trim() === agentId && e.mentionable,
  );
  if (deptEntries.length <= 1) {
    mentionListPopupOpen.value = false;
    emit('mentionEntry', deptEntries[0] || entry);
    return;
  }
  mentionListPopupOpen.value = false;
  avatarPopupTarget.value = { agentId: entry.agentId, agentName: entry.agentName, avatarUrl: entry.avatarUrl };
  const el = event.currentTarget as HTMLElement | null;
  if (el) {
    avatarPopupAnchorEl.value = el;
    void updateAvatarPopupPlacement(el.getBoundingClientRect());
  }
}

function clampPopupPosition(anchorRect: DOMRect, panelEl: HTMLElement | null, options?: {
  preferredWidth?: number;
  alignRight?: boolean;
}) {
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  const measuredWidth = Math.round(panelEl?.offsetWidth || options?.preferredWidth || 0);
  const measuredHeight = Math.round(panelEl?.offsetHeight || 0);
  const spaceAbove = Math.max(0, anchorRect.top - POPUP_VIEWPORT_PADDING - POPUP_OFFSET);
  const spaceBelow = Math.max(0, viewportHeight - anchorRect.bottom - POPUP_VIEWPORT_PADDING - POPUP_OFFSET);
  const openUpward = spaceAbove >= measuredHeight || spaceAbove > spaceBelow;
  const maxLeft = Math.max(
    POPUP_VIEWPORT_PADDING,
    viewportWidth - measuredWidth - POPUP_VIEWPORT_PADDING,
  );
  const preferredLeft = options?.alignRight
    ? Math.round(anchorRect.right - measuredWidth)
    : Math.round(anchorRect.left);
  const left = Math.min(
    Math.max(POPUP_VIEWPORT_PADDING, preferredLeft),
    maxLeft,
  );
  const top = openUpward
    ? Math.max(POPUP_VIEWPORT_PADDING, Math.round(anchorRect.top) - measuredHeight - POPUP_OFFSET)
    : Math.min(
      Math.round(anchorRect.bottom) + POPUP_OFFSET,
      Math.max(POPUP_VIEWPORT_PADDING, viewportHeight - measuredHeight - POPUP_VIEWPORT_PADDING),
    );
  return {
    left: `${left}px`,
    top: `${top}px`,
  };
}

async function updateMentionListPopupPlacement(anchorRect?: DOMRect) {
  const rect = anchorRect || mentionListButtonRef.value?.getBoundingClientRect();
  if (!rect) return;
  await nextTick();
  mentionListPopupStyle.value = clampPopupPosition(rect, mentionListPopupRef.value, {
    preferredWidth: 320,
    alignRight: true,
  });
  mentionListAreaRef.value?.updateThumb();
}

async function updateAvatarPopupPlacement(anchorRect?: DOMRect) {
  const rect = anchorRect || avatarPopupAnchorEl.value?.getBoundingClientRect();
  if (!rect) return;
  await nextTick();
  avatarPopupStyle.value = clampPopupPosition(rect, avatarPopupPanelRef.value, {
    preferredWidth: 320,
    alignRight: true,
  });
  avatarPopupAreaRef.value?.updateThumb();
}

function handleCompactPersonaEntryClick(event: MouseEvent, entry: ChatMentionEntry & { selected?: boolean }) {
  handleMentionEntryClick(event, entry);
}

function applyAvatarPopupSelection(entry: {
  agentId: string;
  agentName: string;
  departmentId: string;
  departmentName: string;
  avatarUrl?: string;
}) {
  avatarPopupTarget.value = null;
  const matched = (props.mentionEntries || []).find(
    (e) => String(e.agentId || "").trim() === entry.agentId && String(e.departmentId || "").trim() === entry.departmentId,
  );
  if (matched) {
    emit('mentionEntry', matched);
  }
}

function closeAvatarPopup() {
  avatarPopupTarget.value = null;
  avatarPopupAnchorEl.value = null;
}

function closeMentionListPopup() {
  mentionListPopupOpen.value = false;
}

function toggleMentionListPopup() {
  if (busy.value) return;
  mentionListPopupOpen.value = !mentionListPopupOpen.value;
  if (mentionListPopupOpen.value) {
    closeAvatarPopup();
    void updateMentionListPopupPlacement();
  }
}

function handleAvatarClickOutside(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (!target) {
    closeAvatarPopup();
    closeMentionListPopup();
    return;
  }
  if (
    mentionListPopupOpen.value
    && !mentionListButtonRef.value?.contains(target)
    && !mentionListPopupRef.value?.contains(target)
  ) {
    closeMentionListPopup();
  }
  if (
    avatarPopupTarget.value
    && !avatarPopupPanelRef.value?.contains(target)
  ) {
    closeAvatarPopup();
  }
}

function handleMentionPopupViewportChange() {
  if (!mentionListPopupOpen.value) return;
  void updateMentionListPopupPlacement();
}

function handleAvatarPopupViewportChange() {
  if (!avatarPopupTarget.value) return;
  void updateAvatarPopupPlacement();
}

const menuButtonRef = ref<HTMLButtonElement | null>(null);
const menuPlacement = ref<"top" | "bottom">("top");

function updateMenuPlacement() {
  const rect = menuButtonRef.value?.getBoundingClientRect();
  if (!rect) return;
  menuPlacement.value = rect.top >= window.innerHeight / 2 ? "top" : "bottom";
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function frontSpeakingMuted(entry: ChatMentionEntry): boolean {
  return props.selectedMentionKeys.length > 0 && entry.isFrontSpeaking;
}

onMounted(() => {
  updateMenuPlacement();
  window.addEventListener("resize", updateMenuPlacement);
  window.addEventListener("scroll", updateMenuPlacement, true);
  window.addEventListener("resize", handleMentionPopupViewportChange);
  window.addEventListener("scroll", handleMentionPopupViewportChange, true);
  window.addEventListener("resize", handleAvatarPopupViewportChange);
  window.addEventListener("scroll", handleAvatarPopupViewportChange, true);
  window.addEventListener("click", handleAvatarClickOutside, true);
  window.addEventListener("pointerdown", handleGlobalPointerDown, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", updateMenuPlacement);
  window.removeEventListener("scroll", updateMenuPlacement, true);
  window.removeEventListener("resize", handleMentionPopupViewportChange);
  window.removeEventListener("scroll", handleMentionPopupViewportChange, true);
  window.removeEventListener("resize", handleAvatarPopupViewportChange);
  window.removeEventListener("scroll", handleAvatarPopupViewportChange, true);
  window.removeEventListener("click", handleAvatarClickOutside, true);
  window.removeEventListener("pointerdown", handleGlobalPointerDown, true);
});
</script>

<style scoped>
.ecall-toolbar-mention-scroll {
  scrollbar-gutter: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.ecall-toolbar-mention-scroll::-webkit-scrollbar {
  width: 0;
  height: 0;
}
</style>
