<template>
  <div
    v-bind="attrs"
    class="flex flex-wrap items-center gap-2 text-xs"
  >
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      leave-active-class="transition duration-200 ease-out"
      leave-to-class="opacity-0 translate-y-1"
    >
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
          :class="SESSION_FLOAT_FROST_CIRCLE"
          :title="t('chat.conversationMenu.title')"
          @mousedown="updateMenuPlacement"
        >
          <Grip class="h-5 w-5" />
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
    </Transition>
      <SessionControlItems
        :show-workspace-button="!hideWorkspaceButton"
        :workspace-button-label="workspaceButtonLabel"
        :workspace-button-name="workspaceButtonName"
        :workspace-button-disabled="workspaceButtonDisabled"
        :workspace-work-mode="workspaceWorkMode || 'directory'"
        :workspace-permission-kind="workspacePermissionKind"
        :auto-push-active="autoPushActive"
        :delegates="delegateStatuses || []"
        :running-task-count="runningTaskCount"
        :running-shell-count="runningShellCount"
        @lock-workspace="emit('lockWorkspace')"
        @open-run-summary="emit('openRunSummary')"
      />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useAttrs, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { BellRing, ClipboardCheck, ClipboardList, GitBranch, GitBranchPlus, Grip, ListTodo, MessageSquareMore, Package, Palette, Send, Share2, Split, Users } from "@lucide/vue";
import type { ConversationDelegateStatusSummary, ShellWorkMode } from "../../../types/app";
import { useChatComposerAppearance } from "../../shell/composables/use-chat-composer-appearance";
import { useChatMessageAppearance, type ChatMarkdownLayout } from "../../shell/composables/use-chat-message-appearance";
import { useFileReaderAppearance } from "../../shell/composables/use-file-reader-appearance";
import SessionControlItems from "./SessionControlItems.vue";
import { SESSION_FLOAT_FROST_CIRCLE } from "./session-float-styles";
import SegmentedControl from "../../config/components/SegmentedControl.vue";

defineOptions({
  inheritAttrs: false,
});

// 与思维链预览条同一套磨砂外观；圆钮用这个底座
const props = withDefaults(defineProps<{
  chatting: boolean;
  frozen: boolean;
  conversationBusy?: boolean;
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  workspaceButtonDisabled?: boolean;
  workspacePermissionKind?: "approval" | "full_access" | "autonomous";
  autoPushActive?: boolean;
  workspaceWorkMode?: ShellWorkMode;
  hideMenuButton?: boolean;
  hideWorkspaceButton?: boolean;
  showTaskCreateMenuItem?: boolean;
  showDelegateMenuItem?: boolean;
  showBranchMenuItem?: boolean;
  showCodeReviewMenuItem?: boolean;
  showForwardMenuItem?: boolean;
  showAutoPushMenuItem?: boolean;
  showShareMenuItem?: boolean;
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
  showOpenInBrowserButton: false,
});

const emit = defineEmits<{
  (e: "lockWorkspace"): void;
  (e: "openBranchSelection"): void;
  (e: "openCodeReview"): void;
  (e: "openTaskCreate"): void;
  (e: "openDelegateSelection"): void;
  (e: "openRunSummary"): void;
  (e: "openForwardSelection"): void;
  (e: "openAutoPush"): void;
  (e: "openShareSelection"): void;
  (e: "openConversationInBrowser"): void;
  (e: "openBranchFromCurrent"): void;
  (e: "openSideChat"): void;
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
const showTaskCreateMenuItem = computed(() => props.showTaskCreateMenuItem);
const showDelegateMenuItem = computed(() => props.showDelegateMenuItem);
const showBranchMenuItem = computed(() => props.showBranchMenuItem);
const showCodeReviewMenuItem = computed(() => props.showCodeReviewMenuItem);
const showForwardMenuItem = computed(() => props.showForwardMenuItem);
const showAutoPushMenuItem = computed(() => props.showAutoPushMenuItem);
const showShareMenuItem = computed(() => props.showShareMenuItem);
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

onMounted(() => {
  updateMenuPlacement();
  window.addEventListener("resize", updateMenuPlacement);
  window.addEventListener("scroll", updateMenuPlacement, true);
  window.addEventListener("pointerdown", handleGlobalPointerDown, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", updateMenuPlacement);
  window.removeEventListener("scroll", updateMenuPlacement, true);
  window.removeEventListener("pointerdown", handleGlobalPointerDown, true);
});
</script>
