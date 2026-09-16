<template>
  <aside class="conversation-time-container flex h-full w-full shrink-0 flex-col border-r border-base-300 bg-base-200">
    <div class="flex items-center p-2 pb-0">
      <SegmentedControl
        v-model="activeConversationTab"
        :options="conversationTabOptions"
        size="sm"
        full-width
        surface-class="bg-base-300"
      />
    </div>
    <ChatConversationFloatingScroll ref="conversationFloatingScrollRef" class="flex-1 min-h-0">
      <Transition :name="conversationTabTransitionName" mode="out-in" @after-enter="handleConversationTabTransitionSettled">
        <div :key="activeConversationTab" class="conversation-tab-panel">
          <ChatTaskSidebarPanel
            v-if="activeConversationTab === 'task'"
            :conversation-items="items"
            :search-query="conversationSearchQuery"
            @edit-task="requestTaskEdit"
            @layout-change="scheduleConversationListScrollbarUpdate"
          />
          <template v-else>
            <CollapsibleGroup
              v-for="section in displayedConversationSections"
              :key="section.key"
              :ref="(el) => setConversationSectionElement(section.key, el)"
              :title="section.title"
              :count="isRecentConversationSection(section.key) ? section.visibleCount : section.totalItemCount"
              :model-value="isConversationSectionCollapsed(section.key)"
              :draggable="isConversationSectionDraggable(section)"
              :drop-indicator="conversationSectionDragIndicator(section)"
              @update:model-value="toggleConversationSection(section.key)"
              @collapse-all="collapseAllConversationSections"
              @after-enter="scheduleConversationListScrollbarUpdate"
              @after-leave="scheduleConversationListScrollbarUpdate"
              @dragstart="handleConversationSectionDragStart(section, $event)"
              @dragover="handleConversationSectionDragOver(section, $event)"
              @drop="handleConversationSectionDrop(section, $event)"
              @dragend="handleConversationSectionDragEnd"
            >
            <template #actions>
              <button
                v-if="section.workspaceRootPath"
                type="button"
                class="btn btn-ghost btn-xs ml-auto h-6 min-h-6 w-6 min-w-6 shrink-0 p-0 text-base-content opacity-0 transition-opacity group-hover/section:opacity-100"
                :title="t('chat.newConversation')"
                @click.stop="createConversationInSection(section)"
                @dblclick.stop
              >
                <SquarePen class="h-3.5 w-3.5" />
              </button>
            </template>
            <template v-for="item in section.visibleItems" :key="item.conversationId">
              <ChatConversationItem
                :item="item"
                :level="compactConversationList ? simpleConversationItemLevel(item) : 'full'"
                :active-conversation-id="props.activeConversationId"
                :user-alias="props.userAlias"
                :user-avatar-url="props.userAvatarUrl"
                :persona-name-map="props.personaNameMap"
                :persona-avatar-url-map="props.personaAvatarUrlMap"
                :pipeline-status-by-id="conversationStatusById"
                :show-source-badge="isRecentConversationSection(section.key)"
                :compact-indicator="compactConversationList"
                @select="(payload) => emit('select', payload)"
                @rename="(payload) => emit('rename', payload)"
                @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                @reveal-section="revealConversationSection(item)"
              />
              <template v-if="(section.simpleFollowers[String(item.conversationId || '').trim()] || []).length > 0">
                <ChatConversationItem
                  v-for="simpleItem in (section.simpleFollowers[String(item.conversationId || '').trim()] || [])"
                  :key="`simple-${simpleItem.conversationId}`"
                  :item="simpleItem"
                  :level="simpleItemLevel(simpleItem)"
                  :active-conversation-id="props.activeConversationId"
                  :user-alias="props.userAlias"
                  :user-avatar-url="props.userAvatarUrl"
                  :persona-name-map="props.personaNameMap"
                  :persona-avatar-url-map="props.personaAvatarUrlMap"
                  :pipeline-status-by-id="conversationStatusById"
                  @select="(payload) => emit('select', payload)"
                  @rename="(payload) => emit('rename', payload)"
                  @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                  @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                  @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                  @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                />
              </template>
            </template>
            <div v-if="section.hiddenItemCount > 0" class="mx-1 pb-1.5 pt-0.5">
              <button
                type="button"
                class="group flex h-7.5 w-full items-center justify-center gap-1.5 rounded-lg px-2 text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                :title="t('chat.loadMore')"
                @click.stop="loadMoreConversationsInSection(section.key)"
              >
                <ChevronDown class="h-3.5 w-3.5 opacity-60 transition-transform duration-200 group-hover:translate-y-0.5 group-hover:opacity-100" />
                <span>{{ t("chat.loadMore") }}</span>
                <span class="rounded-full bg-base-300/60 px-1.5 py-0.5 text-caption font-medium leading-tight tabular-nums text-base-content/50 group-hover:bg-base-content/10 group-hover:text-base-content/75">
                  {{ section.hiddenItemCount }}
                </span>
              </button>
            </div>
            </CollapsibleGroup>
            <div
              v-if="displayedConversationSections.length === 0"
              class="px-3 py-4 text-center text-sm text-base-content/60"
            >
              {{ t("chat.conversationSearchEmpty") }}
            </div>
          </template>
        </div>
      </Transition>
    </ChatConversationFloatingScroll>
    <div class="shrink-0 px-2 py-2">
      <div v-if="showSearch" class="pb-1.5">
        <label class="input input-bordered input-sm flex h-8 min-w-0 items-center gap-2 bg-base-100">
          <Search class="h-3.5 w-3.5 opacity-60" />
          <input
            ref="searchInputRef"
            v-model="conversationSearchQuery"
            type="text"
            class="w-full bg-transparent outline-none"
            :placeholder="searchPlaceholder"
          />
        </label>
      </div>
      <div class="flex items-center justify-between gap-2">
        <div class="dropdown dropdown-top dropdown-start">
          <div
            tabindex="0"
            role="button"
            class="flex h-9 w-9 cursor-pointer items-center justify-center rounded-full bg-neutral text-neutral-content outline-none"
          >
            <img
              v-if="props.userAvatarUrl"
              :src="props.userAvatarUrl"
              :alt="props.userAlias || t('chat.userAvatarAlt')"
              class="h-9 w-9 rounded-full object-cover"
            />
            <span v-else class="text-sm font-bold">{{ userAvatarInitial }}</span>
          </div>
          <ul
            tabindex="0"
            class="dropdown-content menu z-50 mb-2 w-44 rounded-box border border-base-300 bg-base-100 p-1 text-base-content shadow-xl"
          >
            <li>
              <button type="button" @click="handleOpenSettings">
                <Settings class="h-3.5 w-3.5" />
                <span>{{ t("common.settings") }}</span>
              </button>
            </li>
            <li>
              <button type="button" @click="handleOpenBatchArchive">
                <Archive class="h-3.5 w-3.5" />
                <span>{{ t("chat.batchArchive.entryAction") }}</span>
              </button>
            </li>
          </ul>
        </div>
        <div class="flex items-center gap-1">
          <button
            v-if="activeConversationTab !== 'task'"
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0"
            :class="compactConversationList ? 'text-primary' : 'text-base-content/55'"
            :title="compactConversationList ? t('chat.switchToCompositeView') : t('chat.switchToCompactView')"
            @click="compactConversationList = !compactConversationList"
          >
            <List v-if="compactConversationList" class="h-4 w-4" />
            <LayoutList v-else class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0"
            :class="showSearch ? 'text-primary' : 'text-base-content/55'"
            :title="searchPlaceholder"
            @click="showSearch = !showSearch"
          >
            <Search class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
    <dialog ref="batchArchiveDialogRef" class="modal" @close="closeBatchArchiveCard" @cancel.prevent="closeBatchArchiveCard">
      <div class="modal-box flex h-[min(88vh,44rem)] w-[min(94vw,64rem)] max-w-none flex-col overflow-hidden p-0">
        <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 px-5 py-4">
          <h3 class="text-base font-semibold">{{ t("chat.batchArchive.title") }}</h3>
        </div>
        <div class="flex min-h-0 flex-1 flex-col bg-base-200/35 px-5 py-4">
          <section class="shrink-0">
            <div class="text-sm font-semibold">{{ t("chat.batchArchive.conditionTitle") }}</div>
            <div class="mt-3 grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.8fr)]">
              <div class="space-y-2">
                <label class="block text-sm font-medium" for="batch-archive-days">
                  {{ t("chat.batchArchive.daysLabel") }}
                </label>
                <div class="flex items-center gap-2">
                  <input
                    id="batch-archive-days"
                    v-model.number="batchArchiveDays"
                    type="number"
                    min="1"
                    step="1"
                    class="input input-bordered w-28"
                  />
                  <span class="text-sm text-base-content/70">{{ t("chat.batchArchive.daysSuffix") }}</span>
                </div>
              </div>
              <div class="space-y-2">
                <label class="block text-sm font-medium" for="batch-archive-model">
                  {{ t("chat.batchArchive.modelLabel") }}
                </label>
                <ApiConfigPicker
                  id="batch-archive-model"
                  v-model="batchArchiveSelectedModelId"
                  :api-configs="batchArchiveApiConfigs"
                />
              </div>
            </div>
            <label class="mt-4 flex items-start gap-3 px-1 py-1">
              <input
                v-model="batchArchiveKeepOnePerWorkspace"
                type="checkbox"
                class="checkbox checkbox-sm mt-0.5"
              />
              <div class="min-w-0 space-y-1 text-sm">
                <div class="font-medium leading-5">{{ t("chat.batchArchive.keepOnePerWorkspace") }}</div>
                <div class="text-xs leading-5 text-base-content/65">{{ t("chat.batchArchive.keepOnePerWorkspaceHint") }}</div>
              </div>
            </label>
          </section>

          <section v-if="batchArchiveCardOpen" class="mt-5 flex min-h-0 flex-1 flex-col">
            <div class="flex items-center justify-between gap-3">
              <div class="text-sm font-semibold">
                {{ t("chat.batchArchive.previewTitle", { count: batchArchiveCandidateConversations.length }) }}
              </div>
            </div>
            <div v-if="batchArchiveCandidateConversations.length === 0" class="mt-3 px-1 py-4 text-sm text-base-content/60">
              {{ t("chat.batchArchive.empty") }}
            </div>
            <div v-else class="mt-3 min-h-0 flex-1 space-y-2 overflow-auto pr-1">
              <article
                v-for="item in batchArchiveCandidateConversations"
                :key="item.conversationId"
                class="flex items-center gap-3 rounded-lg border border-base-300 bg-base-100 px-3 py-2"
              >
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm shrink-0"
                  :checked="isBatchArchiveConversationSelected(item)"
                  @change="toggleBatchArchiveConversation(item, ($event.target as HTMLInputElement).checked)"
                />
                <Archive class="h-4 w-4 shrink-0 text-base-content/55" />
                <div class="min-w-0 flex-1">
                  <div class="truncate text-sm font-medium">{{ conversationDisplayTitle(item) }}</div>
                </div>
                <div class="badge badge-ghost max-w-28 shrink-0 truncate">
                  {{ conversationWorkspaceLabel(item) }}
                </div>
                <div class="w-24 shrink-0 text-right text-xs text-base-content/65">
                  {{ t("chat.batchArchive.olderThanDays", { count: conversationAgeDays(item) }) }}
                </div>
                <div class="w-12 shrink-0 text-right text-xs text-base-content/60">
                  {{ formatConversationTime(item.updatedAt) }}
                </div>
              </article>
            </div>
          </section>
        </div>
        <div class="flex shrink-0 items-center justify-between gap-3 border-t border-base-300 px-5 py-4">
          <div class="flex items-center gap-2">
            <button type="button" class="btn btn-ghost btn-sm" :disabled="batchArchiveCandidateConversations.length === 0" @click="selectAllBatchArchiveCandidates">
              {{ t("chat.batchArchive.selectAll") }}
            </button>
            <button type="button" class="btn btn-ghost btn-sm" :disabled="batchArchiveSelectedConversationIds.size === 0" @click="clearBatchArchiveSelection">
              {{ t("chat.batchArchive.selectNone") }}
            </button>
            <span class="text-xs text-base-content/60">
              {{ t("chat.batchArchive.selectedCount", { count: batchArchiveSelectedConversationIds.size }) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <button type="button" class="btn btn-sm" @click="closeBatchArchiveCard">
              {{ t("common.cancel") }}
            </button>
            <button
              type="button"
              class="btn btn-primary btn-sm"
              :disabled="batchArchiveStartDisabled"
              @click="submitBatchArchive"
            >
              <span v-if="batchArchiveSubmitting" class="loading loading-spinner loading-xs"></span>
              {{ t("chat.batchArchive.startArchive") }}
            </button>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeBatchArchiveCard">close</button>
      </form>
    </dialog>
  </aside>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Archive, ChevronDown, LayoutList, List, Search, Settings, SquarePen } from "@lucide/vue";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import ChatConversationItem from "./ChatConversationItem.vue";
import type { ApiConfigItem, ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import { stripToolcallMarkers } from "../../../utils/chat-message-semantics";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import { invokeTauri } from "../../../services/tauri-api";
import { usePipelineStatus } from "../../shell/composables/use-pipeline-status";
import ApiConfigPicker from "../../config/components/ApiConfigPicker.vue";
import { formatConversationListTime } from "../utils/conversation-time";
import {
  aggregateConversationItems,
  conversationLastUsedMs,
} from "../utils/conversation-aggregation";
import {
  applyConversationSectionOrder,
  buildConversationSections,
  conversationCountSinceDayStart,
  RECENT_CONVERSATION_SECTION_KEY,
  CURRENT_PROJECT_SECTION_KEY,
  workspaceNameFromPath,
  type ConversationSection,
  type ConversationSectionOrderState,
} from "../utils/conversation-sections";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import { simpleConversationItemLevel } from "../utils/conversation-item-display";
import ChatConversationFloatingScroll from "./ChatConversationFloatingScroll.vue";
import ChatTaskSidebarPanel from "./ChatTaskSidebarPanel.vue";
import SegmentedControl, { type SegmentedControlOption } from "../../config/components/SegmentedControl.vue";

type ConversationSidebarTab = "local" | "contact" | "task";
type DisplayConversationSection = ConversationSection & {
  visibleItems: ChatConversationOverviewItem[];
  /** full 会话 id → 聚合其后的简单条目（同人格旧会话，按更新时间倒序） */
  simpleFollowers: Record<string, ChatConversationOverviewItem[]>;
  visibleCount: number;
  hiddenItemCount: number;
  totalItemCount: number;
};
type BatchArchiveConversationsOutput = {
  success: boolean;
  acceptedConversationIds: string[];
  skipped: Array<{ conversationId: string; reason: string }>;
  activeConversationId?: string;
};

const CONVERSATION_SECTION_UNUSED_DAYS = 7;
const CONVERSATION_SECTION_MIN_VISIBLE = 5;
const CONVERSATION_SECTION_LOAD_MORE_STEP = 10;
const CONVERSATION_SECTION_RESET_DELAY_MS = 30_000;
const COMPACT_CONVERSATION_VIEW_STORAGE_KEY = "easy-call.chat.conversation-compact-view.v1";

/** 读「精简视图」偏好：无记录时默认综合视图 */
function readCompactConversationViewPreference(): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(COMPACT_CONVERSATION_VIEW_STORAGE_KEY) === "1";
}

const props = defineProps<{
  items: ChatConversationOverviewItem[];
  activeConversationId: string;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  activeTab: ConversationSidebarTab;
  chatModelOptions: ApiConfigItem[];
  toolReviewApiConfigId?: string;
  currentWorkspaceRootPath?: string;
}>();

const emit = defineEmits<{
  (e: "select", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
  (e: "rename", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
  (e: "archiveConversation", conversationId: string): void;
  (e: "exportConversation", conversationId: string): void;
  (e: "deleteConversation", conversationId: string): void;
  (e: "update:activeTab", value: ConversationSidebarTab): void;
  (e: "editTask", task: TaskEntry): void;
  (e: "batchArchiveCompleted", payload: { archivedConversationIds: string[]; activeConversationId?: string }): void;
  (e: "openSettings"): void;
}>();

const { t, locale } = useI18n();
/** 头像上拉菜单：DaisyUI dropdown 依赖焦点开合，选中菜单项后主动失焦收起 */
function closeUserMenu(event: MouseEvent) {
  (event.currentTarget as HTMLElement | null)?.closest(".dropdown")
    ?.querySelector<HTMLElement>("[role='button']")?.blur();
}

function handleOpenSettings(event: MouseEvent) {
  closeUserMenu(event);
  emit("openSettings");
}

function handleOpenBatchArchive(event: MouseEvent) {
  closeUserMenu(event);
  openBatchArchiveCard();
}

const conversationSearchQuery = ref("");
const showSearch = ref(false);
const compactConversationList = ref(readCompactConversationViewPreference());
const searchInputRef = ref<HTMLInputElement | null>(null);
const batchArchiveDialogRef = ref<HTMLDialogElement | null>(null);
const batchArchiveCardOpen = ref(false);

function syncBatchArchiveDialog() {
  const d = batchArchiveDialogRef.value;
  if (!d) return;
  if (batchArchiveCardOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(batchArchiveCardOpen, syncBatchArchiveDialog);
watch(batchArchiveDialogRef, syncBatchArchiveDialog);
const batchArchiveDays = ref(30);
const batchArchiveKeepOnePerWorkspace = ref(true);
const batchArchiveSelectedModelId = ref("");
const batchArchiveSelectedConversationIds = ref<Set<string>>(new Set());
const batchArchiveSubmitting = ref(false);
const conversationFloatingScrollRef = ref<InstanceType<typeof ChatConversationFloatingScroll> | null>(null);
const collapsedConversationSectionKeys = ref<Record<string, boolean>>({});
const conversationSectionOrders = ref<ConversationSectionOrderState>({ local: [], contact: [] });
const draggingConversationSectionKey = ref("");
const dragOverConversationSectionKey = ref("");
const dragOverConversationSectionPlacement = ref<"before" | "after">("before");
const savingConversationSectionOrder = ref(false);
const conversationSectionLoadMoreCounts = ref<Record<string, number>>({});
const conversationSectionResetTimers = new Map<ConversationSidebarTab, ReturnType<typeof setTimeout>>();
const conversationTabTransitionName = ref("conversation-tab-slide-left");
const activeConversationTab = computed({
  get: (): ConversationSidebarTab => {
    if (props.activeTab === "contact" || props.activeTab === "task") return props.activeTab;
    return "local";
  },
  set: (value: ConversationSidebarTab) => emit("update:activeTab", value),
});
const conversationTabOptions = computed<Array<SegmentedControlOption<ConversationSidebarTab>>>(() => [
  { value: "local", label: t("chat.localConversationTab") },
  { value: "contact", label: t("chat.contactConversationTab") },
  { value: "task", label: t("chat.taskConversationTab") },
]);
const { conversationStatusById, markConversationRead } = usePipelineStatus({
  activeConversationId: computed(() => String(props.activeConversationId || "").trim()),
});

const conversationPreviewCache = computed(() => new Map(
  props.items.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));

const conversationSections = computed<ConversationSection[]>(() =>
  buildConversationSections(props.items, {
    tab: activeConversationTab.value,
    titles: {
      recent: t("chat.recentConversations"),
      pinned: t("chat.pinnedConversations"),
      other: t("chat.otherConversations"),
      defaultWorkspace: t("chat.defaultWorkspace"),
      currentProject: t("chat.currentProject"),
    },
    locale: locale.value,
    currentWorkspaceRootPath: props.currentWorkspaceRootPath,
    activeConversationId: props.activeConversationId,
    compact: compactConversationList.value,
  }),
);

const orderedConversationSections = computed<ConversationSection[]>(() => {
  const tab = activeConversationTab.value === "contact" ? "contact" : "local";
  return applyConversationSectionOrder(conversationSections.value, conversationSectionOrders.value[tab]).sections;
});

const normalizedConversationSearchQuery = computed(() =>
  String(conversationSearchQuery.value || "").trim().toLocaleLowerCase(),
);

const searchPlaceholder = computed(() =>
  activeConversationTab.value === "task"
    ? t("chat.taskSidebar.searchPlaceholder")
    : t("chat.conversationSearchPlaceholder"),
);

const userAvatarInitial = computed(() => {
  const text = String(props.userAlias || t("chat.userAvatarAlt")).trim();
  return text.charAt(0).toUpperCase() || "U";
});

const batchArchiveApiConfigs = computed(() => props.chatModelOptions.filter((item) => item.enableText));
const batchArchiveModelOptions = computed<Array<{ id: string }>>(() =>
  batchArchiveApiConfigs.value
    .map((item) => ({ id: String(item.id || "").trim() }))
    .filter((item) => !!item.id),
);

const batchArchiveCandidateConversations = computed(() => {
  const thresholdDays = Math.max(1, Math.round(Number(batchArchiveDays.value || 0)));
  const now = Date.now();
  const oldLocalConversations = props.items
    .filter((item) => {
      if (!isLocalConversation(item) || item.isSystemNotificationConversation) return false;
      const updatedAt = Date.parse(String(item.updatedAt || item.lastMessageAt || "").trim());
      if (!Number.isFinite(updatedAt)) return false;
      const ageDays = Math.floor((now - updatedAt) / 86_400_000);
      return ageDays >= thresholdDays;
    });
  if (!batchArchiveKeepOnePerWorkspace.value) {
    return sortBatchArchiveCandidates(oldLocalConversations);
  }
  const preservedConversationIds = new Set<string>();
  const newestByWorkspace = new Map<string, ChatConversationOverviewItem>();
  for (const item of oldLocalConversations) {
    const workspaceKey = conversationWorkspaceKey(item);
    const current = newestByWorkspace.get(workspaceKey);
    if (!current || conversationTimeValue(item) > conversationTimeValue(current)) {
      newestByWorkspace.set(workspaceKey, item);
    }
  }
  for (const item of newestByWorkspace.values()) {
    preservedConversationIds.add(String(item.conversationId || "").trim());
  }
  return sortBatchArchiveCandidates(oldLocalConversations.filter((item) =>
    !preservedConversationIds.has(String(item.conversationId || "").trim()),
  ));
});

const batchArchiveSelectedCandidateIds = computed(() => {
  const selectedIds = batchArchiveSelectedConversationIds.value;
  return batchArchiveCandidateConversations.value
    .map(batchArchiveConversationId)
    .filter((id) => !!id && selectedIds.has(id));
});

const batchArchiveStartDisabled = computed(() =>
  batchArchiveSubmitting.value
  || batchArchiveSelectedCandidateIds.value.length === 0
  || !String(batchArchiveSelectedModelId.value || "").trim(),
);

watch(
  () => [props.toolReviewApiConfigId, batchArchiveModelOptions.value.map((item) => item.id).join("|")] as const,
  () => {
    const quickModelId = String(props.toolReviewApiConfigId || "").trim();
    const optionIds = batchArchiveModelOptions.value.map((item) => item.id);
    if (quickModelId && optionIds.includes(quickModelId)) {
      batchArchiveSelectedModelId.value = quickModelId;
      return;
    }
    if (!optionIds.includes(batchArchiveSelectedModelId.value)) {
      batchArchiveSelectedModelId.value = optionIds[0] || "";
    }
  },
  { immediate: true },
);

watch(
  () => batchArchiveCandidateConversations.value.map((item) => String(item.conversationId || "").trim()).filter(Boolean),
  (candidateIds) => {
    const candidateSet = new Set(candidateIds);
    const next = new Set<string>();
    for (const id of batchArchiveSelectedConversationIds.value) {
      if (candidateSet.has(id)) next.add(id);
    }
    batchArchiveSelectedConversationIds.value = next;
  },
  { immediate: true },
);

const filteredConversationSections = computed(() => {
  const query = normalizedConversationSearchQuery.value;
  if (!query) return orderedConversationSections.value;
  return orderedConversationSections.value
    .map((section) => ({
      ...section,
      items: section.items.filter((item) => conversationMatchesSearch(item, query)),
    }))
    .filter((section) => section.items.length > 0);
});

const displayedConversationSections = computed<DisplayConversationSection[]>(() =>
  filteredConversationSections.value.map((section) => buildDisplayedConversationSection(section)),
);

watch(
  () => props.activeConversationId,
  (conversationId) => markConversationRead(conversationId),
  { immediate: true },
);

watch(
  () => activeConversationTab.value,
  (nextValue, previousValue) => {
    if (previousValue && previousValue !== nextValue) {
      scheduleConversationSectionReset(previousValue);
    }
    clearConversationSectionResetTimer(nextValue);
    if (!previousValue || nextValue === previousValue) return;
    conversationTabTransitionName.value = conversationTabOrder(nextValue) > conversationTabOrder(previousValue)
      ? "conversation-tab-slide-left"
      : "conversation-tab-slide-right";
  },
);

onMounted(() => {
  void loadConversationSectionOrders();
});

onBeforeUnmount(() => {
  clearConversationSectionResetTimer("local");
  clearConversationSectionResetTimer("contact");
  clearConversationSectionResetTimer("task");
});

async function loadConversationSectionOrders() {
  try {
    const result = await invokeTauri<ConversationSectionOrderState>("conversation.sectionOrders.get");
    conversationSectionOrders.value = {
      local: Array.isArray(result?.local) ? result.local.map((item) => String(item || "").trim()).filter(Boolean) : [],
      contact: Array.isArray(result?.contact) ? result.contact.map((item) => String(item || "").trim()).filter(Boolean) : [],
    };
  } catch (error) {
    console.warn("[会话分组排序] 读取失败", { error });
  }
}

async function persistConversationSectionOrder(tab: "local" | "contact", orderedKeys: string[]) {
  savingConversationSectionOrder.value = true;
  try {
    const result = await invokeTauri<{ orderedKeys?: string[] }>("conversation.sectionOrders.save", {
      input: {
        tab,
        orderedKeys,
      },
    });
    conversationSectionOrders.value = {
      ...conversationSectionOrders.value,
      [tab]: Array.isArray(result?.orderedKeys)
        ? result.orderedKeys.map((item) => String(item || "").trim()).filter(Boolean)
        : orderedKeys,
    };
  } catch (error) {
    console.warn("[会话分组排序] 保存失败", { tab, error });
  } finally {
    savingConversationSectionOrder.value = false;
  }
}

function isConversationSectionDraggable(section: ConversationSection): boolean {
  if (normalizedConversationSearchQuery.value) return false;
  return section.key !== "pinned"
    && section.key !== RECENT_CONVERSATION_SECTION_KEY
    && section.key !== CURRENT_PROJECT_SECTION_KEY;
}

function handleConversationSectionDragStart(section: ConversationSection, event: DragEvent) {
  handleConversationSectionDragEnd();
  if (!isConversationSectionDraggable(section)) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  draggingConversationSectionKey.value = section.key;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", section.key);
    const currentTarget = event.currentTarget;
    if (currentTarget instanceof HTMLElement) {
      event.dataTransfer.setDragImage(currentTarget, 16, 16);
    }
  }
}

function handleConversationSectionDragOver(section: ConversationSection, event: DragEvent) {
  if (!draggingConversationSectionKey.value || !isConversationSectionDraggable(section)) return;
  event.preventDefault();
  const currentTarget = event.currentTarget;
  if (currentTarget instanceof HTMLElement) {
    const rect = currentTarget.getBoundingClientRect();
    const offsetY = event.clientY - rect.top;
    dragOverConversationSectionPlacement.value = offsetY >= rect.height / 2 ? "after" : "before";
    dragOverConversationSectionKey.value = section.key;
  }
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = "move";
  }
}

function handleConversationSectionDragEnd() {
  draggingConversationSectionKey.value = "";
  dragOverConversationSectionKey.value = "";
  dragOverConversationSectionPlacement.value = "before";
}

function handleConversationSectionDrop(section: ConversationSection, event: DragEvent) {
  const draggingKey = String(draggingConversationSectionKey.value || "").trim();
  if (!draggingKey || draggingKey === section.key || !isConversationSectionDraggable(section)) {
    handleConversationSectionDragEnd();
    return;
  }
  event.preventDefault();
  const tab = activeConversationTab.value === "contact" ? "contact" : "local";
  const draggableKeys = orderedConversationSections.value
    .filter((item) => isConversationSectionDraggable(item))
    .map((item) => item.key);
  const fromIndex = draggableKeys.indexOf(draggingKey);
  const rawToIndex = draggableKeys.indexOf(section.key);
  const dropPlacement = dragOverConversationSectionKey.value === section.key
    ? dragOverConversationSectionPlacement.value
    : "before";
  const toIndex = dropPlacement === "after" ? rawToIndex + 1 : rawToIndex;
  if (fromIndex < 0 || toIndex < 0) {
    handleConversationSectionDragEnd();
    return;
  }
  const nextDraggableKeys = [...draggableKeys];
  const [moved] = nextDraggableKeys.splice(fromIndex, 1);
  const adjustedToIndex = fromIndex < toIndex ? toIndex - 1 : toIndex;
  nextDraggableKeys.splice(adjustedToIndex, 0, moved);
  const fixedPrefix = orderedConversationSections.value
    .filter((item) => !isConversationSectionDraggable(item))
    .map((item) => item.key);
  const nextOrder = [...fixedPrefix, ...nextDraggableKeys];
  handleConversationSectionDragEnd();
  const currentSavedOrder = conversationSectionOrders.value[tab] || [];
  const orderUnchanged = nextOrder.length === currentSavedOrder.length
    && nextOrder.every((key, index) => key === currentSavedOrder[index]);
  if (orderUnchanged) return;
  conversationSectionOrders.value = {
    ...conversationSectionOrders.value,
    [tab]: nextOrder,
  };
  void persistConversationSectionOrder(tab, nextOrder);
}

function conversationSectionDragIndicator(section: ConversationSection): "before" | "after" | null {
  if (dragOverConversationSectionKey.value !== section.key) return null;
  return dragOverConversationSectionPlacement.value;
}

function defaultVisibleConversationCount(section: ConversationSection): number {
  const items = Array.isArray(section.items) ? section.items : [];
  if (items.length <= CONVERSATION_SECTION_MIN_VISIBLE) return items.length;
  if (normalizedConversationSearchQuery.value) return items.length;
  if (section.key === "pinned") return items.length;
  if (section.key === RECENT_CONVERSATION_SECTION_KEY) {
    // 基础展示量 = 至少 5 条 + 凌晨 4 点至今活跃过的会话，超出部分通过「加载更多」逐步展开。
    return Math.max(CONVERSATION_SECTION_MIN_VISIBLE, conversationCountSinceDayStart(items));
  }
  const thresholdMs = Date.now() - CONVERSATION_SECTION_UNUSED_DAYS * 24 * 60 * 60 * 1000;
  const recentCount = items.reduce((count, item) => (
    conversationLastUsedMs(item) >= thresholdMs ? count + 1 : count
  ), 0);
  const activeConversationId = String(props.activeConversationId || "").trim();
  const activeIndex = activeConversationId
    ? items.findIndex((item) => String(item.conversationId || "").trim() === activeConversationId)
    : -1;
  return Math.min(
    items.length,
    Math.max(CONVERSATION_SECTION_MIN_VISIBLE, recentCount, activeIndex >= 0 ? activeIndex + 1 : 0),
  );
}

function conversationSectionLoadMoreKey(
  sectionKey: string,
  tab: ConversationSidebarTab = activeConversationTab.value,
): string {
  return `${tab}:${sectionKey}`;
}

function buildDisplayedConversationSection(section: ConversationSection): DisplayConversationSection {
  const items = Array.isArray(section.items) ? section.items : [];
  // 精简模式：不做 full 聚合，全部会话以简单条目平铺
  if (compactConversationList.value) {
    return {
      ...section,
      visibleItems: items,
      simpleFollowers: {},
      visibleCount: items.length,
      hiddenItemCount: 0,
      totalItemCount: items.length,
    };
  }
  const stateKey = conversationSectionLoadMoreKey(section.key);
  const baseVisibleCount = defaultVisibleConversationCount(section);
  const extraVisibleCount = Math.max(0, Number(conversationSectionLoadMoreCounts.value[stateKey] || 0));
  const visibleCount = Math.min(items.length, baseVisibleCount + extraVisibleCount);
  const { reorderedItems, simpleFollowers } = aggregateConversationItems(items.slice(0, visibleCount), {
    searchActive: !!normalizedConversationSearchQuery.value,
  });
  return {
    ...section,
    visibleItems: reorderedItems,
    simpleFollowers,
    visibleCount: visibleCount,
    hiddenItemCount: Math.max(0, items.length - visibleCount),
    totalItemCount: items.length,
  };
}

function loadMoreConversationsInSection(sectionKey: string) {
  const key = String(sectionKey || "").trim();
  if (!key) return;
  const stateKey = conversationSectionLoadMoreKey(key);
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    [stateKey]: Math.max(0, Number(conversationSectionLoadMoreCounts.value[stateKey] || 0)) + CONVERSATION_SECTION_LOAD_MORE_STEP,
  };
  scheduleConversationListScrollbarUpdate();
}

function clearConversationSectionResetTimer(tab: ConversationSidebarTab) {
  const timer = conversationSectionResetTimers.get(tab);
  if (!timer) return;
  clearTimeout(timer);
  conversationSectionResetTimers.delete(tab);
}

function resetConversationSectionLoadMore(tab: ConversationSidebarTab) {
  if (tab === "task") return;
  const prefix = `${tab}:`;
  conversationSectionLoadMoreCounts.value = Object.fromEntries(
    Object.entries(conversationSectionLoadMoreCounts.value)
      .filter(([stateKey, value]) => !stateKey.startsWith(prefix) || !(Number(value) > 0)),
  );
  scheduleConversationListScrollbarUpdate();
}

function scheduleConversationSectionReset(tab: ConversationSidebarTab) {
  if (tab === "task") return;
  clearConversationSectionResetTimer(tab);
  conversationSectionResetTimers.set(tab, setTimeout(() => {
    conversationSectionResetTimers.delete(tab);
    if (activeConversationTab.value === tab) return;
    resetConversationSectionLoadMore(tab);
  }, CONVERSATION_SECTION_RESET_DELAY_MS));
}

watch(showSearch, async (visible) => {
  if (visible) {
    await nextTick();
    searchInputRef.value?.focus();
  } else {
    conversationSearchQuery.value = "";
  }
});

watch(compactConversationList, (enabled) => {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(COMPACT_CONVERSATION_VIEW_STORAGE_KEY, enabled ? "1" : "0");
  } catch {
    // 存储不可用（如隐私模式）时仅内存生效
  }
});

function isConversationSectionCollapsed(key: string): boolean {
  if (normalizedConversationSearchQuery.value) return false;
  const collapsed = collapsedConversationSectionKeys.value[key];
  if (collapsed !== undefined) return collapsed;
  // 有「当前项目」分组时：默认展开当前项目、折叠最近会话；
  // 没有（未绑定工作区）时最近会话保持默认展开，避免整列全折叠。
  if (key === CURRENT_PROJECT_SECTION_KEY) return false;
  const hasCurrentProjectSection = conversationSections.value.some(
    (section) => section.key === CURRENT_PROJECT_SECTION_KEY,
  );
  if (key === RECENT_CONVERSATION_SECTION_KEY) return hasCurrentProjectSection;
  return true;
}

function toggleConversationSection(key: string) {
  const nextCollapsed = !isConversationSectionCollapsed(key);
  collapsedConversationSectionKeys.value = {
    ...collapsedConversationSectionKeys.value,
    [key]: nextCollapsed,
  };
  if (nextCollapsed) {
    const stateKey = conversationSectionLoadMoreKey(key);
    if (Number(conversationSectionLoadMoreCounts.value[stateKey] || 0) > 0) {
      // 折叠分组时清掉「加载更多」展开量，重新展开后回到默认可见数量。
      conversationSectionLoadMoreCounts.value = {
        ...conversationSectionLoadMoreCounts.value,
        [stateKey]: 0,
      };
    }
  }
  scheduleConversationListScrollbarUpdate();
}

function expandConversationSection(key: string) {
  if (!key || !isConversationSectionCollapsed(key)) return;
  collapsedConversationSectionKeys.value = {
    ...collapsedConversationSectionKeys.value,
    [key]: false,
  };
  scheduleConversationListScrollbarUpdate();
}

const conversationSectionElements = new Map<string, HTMLElement>();

function setConversationSectionElement(key: string, element: unknown) {
  const root = element instanceof HTMLElement
    ? element
    : (element as { $el?: HTMLElement | null } | null)?.$el ?? null;
  if (root) conversationSectionElements.set(key, root);
  else conversationSectionElements.delete(key);
}

function revealConversationSection(item: ChatConversationOverviewItem) {
  const conversationId = String(item.conversationId || "").trim();
  if (!conversationId) return;
  const section = conversationSections.value.find((entry) =>
    entry.key !== RECENT_CONVERSATION_SECTION_KEY
    && entry.items.some((candidate) => String(candidate.conversationId || "").trim() === conversationId),
  );
  if (!section) return;
  const wasCollapsed = isConversationSectionCollapsed(section.key);
  expandConversationSection(section.key);
  const element = conversationSectionElements.get(section.key);
  window.setTimeout(() => {
    if (element) conversationFloatingScrollRef.value?.scrollToElement(element);
  }, wasCollapsed ? 220 : 0);
}

function collapseAllConversationSections() {
  collapsedConversationSectionKeys.value = conversationSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedConversationSectionKeys.value } as Record<string, boolean>);
  const tabPrefix = `${activeConversationTab.value}:`;
  conversationSectionLoadMoreCounts.value = Object.fromEntries(
    Object.entries(conversationSectionLoadMoreCounts.value)
      .filter(([stateKey, value]) => !stateKey.startsWith(tabPrefix) || !(Number(value) > 0)),
  );
  scheduleConversationListScrollbarUpdate();
}

function conversationTabOrder(value: ConversationSidebarTab): number {
  if (value === "task") return 2;
  if (value === "contact") return 1;
  return 0;
}

function requestTaskEdit(task: TaskEntry) {
  emit("editTask", task);
}

function scheduleConversationListScrollbarUpdate() {
  void nextTick(() => {
    requestAnimationFrame(() => conversationFloatingScrollRef.value?.updateThumb());
  });
}

function handleConversationTabTransitionSettled() {
  scheduleConversationListScrollbarUpdate();
}

function createConversationInSection(section: ConversationSection) {
  // 新建入口统一打开会话草稿；从文件夹分节新建时把该文件夹作为草稿工作区
  window.dispatchEvent(new CustomEvent("easy-call:open-draft-conversation", {
    detail: {
      workspaceRootPath: String(section.workspaceRootPath || "").trim() || undefined,
    },
  }));
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

function isLocalConversation(item: ChatConversationOverviewItem): boolean {
  return item.kind !== "remote_im_contact";
}

function conversationDisplayTitle(item: ChatConversationOverviewItem): string {
  return resolveConversationDisplayTitle(item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
  });
}

function isRecentConversationSection(sectionKey: string): boolean {
  return sectionKey === RECENT_CONVERSATION_SECTION_KEY;
}

/** 简单项等级：未读或 7 天内有更新 → sim（摘要常显）；否则 → mini（摘要折叠、hover 展开） */
function simpleItemLevel(item: ChatConversationOverviewItem): "sim" | "mini" {
  return simpleConversationItemLevel(item);
}

function openBatchArchiveCard() {
  batchArchiveCardOpen.value = true;
  selectAllBatchArchiveCandidates();
}

function closeBatchArchiveCard() {
  if (batchArchiveSubmitting.value) return;
  batchArchiveCardOpen.value = false;
}

function batchArchiveConversationId(item: ChatConversationOverviewItem): string {
  return String(item.conversationId || "").trim();
}

function isBatchArchiveConversationSelected(item: ChatConversationOverviewItem): boolean {
  const id = batchArchiveConversationId(item);
  return !!id && batchArchiveSelectedConversationIds.value.has(id);
}

function toggleBatchArchiveConversation(item: ChatConversationOverviewItem, checked: boolean) {
  const id = batchArchiveConversationId(item);
  if (!id) return;
  const next = new Set(batchArchiveSelectedConversationIds.value);
  if (checked) {
    next.add(id);
  } else {
    next.delete(id);
  }
  batchArchiveSelectedConversationIds.value = next;
}

function selectAllBatchArchiveCandidates() {
  batchArchiveSelectedConversationIds.value = new Set(
    batchArchiveCandidateConversations.value
      .map(batchArchiveConversationId)
      .filter(Boolean),
  );
}

function clearBatchArchiveSelection() {
  batchArchiveSelectedConversationIds.value = new Set();
}

async function submitBatchArchive() {
  const conversationIds = batchArchiveSelectedCandidateIds.value;
  const reflectionApiConfigId = String(batchArchiveSelectedModelId.value || "").trim();
  if (conversationIds.length === 0 || !reflectionApiConfigId || batchArchiveSubmitting.value) return;
  batchArchiveSubmitting.value = true;
  try {
    const payload = { conversationIds, reflectionApiConfigId };
    const result = await invokeTauri<BatchArchiveConversationsOutput>("conversation.batchArchive", payload, 30_000);
    batchArchiveSelectedConversationIds.value = new Set();
    batchArchiveCardOpen.value = false;
    emit("batchArchiveCompleted", {
      archivedConversationIds: Array.isArray(result.acceptedConversationIds) ? result.acceptedConversationIds : [],
      activeConversationId: String(result.activeConversationId || "").trim() || undefined,
    });
    if (Array.isArray(result.skipped) && result.skipped.length > 0) {
      console.warn("[批量归档] 部分会话跳过", result.skipped);
    }
  } catch (error) {
    console.warn("[批量归档] 提交失败", error);
  } finally {
    batchArchiveSubmitting.value = false;
  }
}

function conversationAgeDays(item: ChatConversationOverviewItem): number {
  const updatedAt = conversationTimeValue(item);
  if (!Number.isFinite(updatedAt)) return 0;
  return Math.max(0, Math.floor((Date.now() - updatedAt) / 86_400_000));
}

function conversationTimeValue(item: ChatConversationOverviewItem): number {
  return Date.parse(String(item.updatedAt || item.lastMessageAt || "").trim()) || 0;
}

function sortBatchArchiveCandidates(items: ChatConversationOverviewItem[]): ChatConversationOverviewItem[] {
  return [...items].sort((left, right) => conversationTimeValue(left) - conversationTimeValue(right));
}

function conversationWorkspaceKey(item: ChatConversationOverviewItem): string {
  const rootPath = String(item.workspaceRootPath || "").trim();
  if (rootPath) return rootPath.toLocaleLowerCase();
  return `label:${conversationWorkspaceLabel(item).toLocaleLowerCase()}`;
}

function conversationWorkspaceLabel(item: ChatConversationOverviewItem): string {
  return String(item.workspaceLabel || workspaceNameFromPath(String(item.workspaceRootPath || "").trim()) || t("chat.defaultWorkspace")).trim();
}

function previewText(preview: ConversationPreviewMessage): string {
  const text = stripToolcallMarkers(preview.textPreview || "");
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

</script>

<style scoped>
.conversation-tab-panel {
  min-height: 100%;
}

.conversation-time-container {
  container-type: inline-size;
}

@container (max-width: 229px) {
  .conversation-time-label {
    display: none;
  }
}

.conversation-tab-slide-left-enter-active,
.conversation-tab-slide-left-leave-active,
.conversation-tab-slide-right-enter-active,
.conversation-tab-slide-right-leave-active {
  transition:
    opacity 200ms ease,
    transform 200ms cubic-bezier(0.22, 1, 0.36, 1);
}

.conversation-tab-slide-left-enter-from,
.conversation-tab-slide-right-leave-to {
  opacity: 0;
  transform: translateX(12px);
}

.conversation-tab-slide-left-leave-to,
.conversation-tab-slide-right-enter-from {
  opacity: 0;
  transform: translateX(-12px);
}
</style>
