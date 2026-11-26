<template>
  <div class="conversation-tab-panel">
    <div v-if="loading && groupedTimerSections.length === 0" class="flex justify-center px-3 py-6">
      <AppLoading size="sm" class="text-base-content/45" />
    </div>
    <div v-else-if="errorText" class="px-3 py-4 text-center text-sm text-error">
      {{ errorText }}
    </div>
    <template v-else>
      <CollapsibleGroup
        head-opaque
        v-for="section in groupedTimerSections"
        :key="section.key"
        :title="section.title"
        :count="section.items.length"
        :model-value="isTimerSectionCollapsed(section.key)"
        @update:model-value="toggleTimerSection(section.key)"
        @collapse-all="collapseAllTimerSections"
        @after-enter="emit('layoutChange')"
        @after-leave="emit('layoutChange')"
      >
        <TimerListItem
          v-for="timer in section.items"
          :key="timer.id"
          :timer="timer"
          @click="emit('editTimer', timer)"
        />
      </CollapsibleGroup>
      <div v-if="groupedTimerSections.length === 0" class="px-3 py-4 text-center text-sm text-base-content/60">
        {{ normalizedSearchQuery ? t("chat.timerSidebar.searchEmpty") : t("chat.timerSidebar.empty") }}
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppLoading from "../../shared/components/AppLoading.vue";
import { invokeTauri, onTransportNotification, onTransportRecovered } from "../../../services/tauri-api";
import type { ChatConversationOverviewItem } from "../../../types/app";
import type { TimerEntry } from "../utils/timer-entry";
import { toErrorMessage } from "../../../utils/error";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import TimerListItem from "./TimerListItem.vue";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";

type TimerSection = {
  key: string;
  title: string;
  items: TimerEntry[];
  order: number;
};

const props = defineProps<{
  conversationItems: ChatConversationOverviewItem[];
  searchQuery: string;
}>();

const emit = defineEmits<{
  (e: "editTimer", timer: TimerEntry): void;
  (e: "layoutChange"): void;
}>();

const { t, locale } = useI18n();
const timers = ref<TimerEntry[]>([]);
const loading = ref(false);
const errorText = ref("");
const collapsedTimerSectionKeys = ref<Record<string, boolean>>({});

const normalizedSearchQuery = computed(() =>
  String(props.searchQuery || "").trim().toLocaleLowerCase(),
);

const conversationTitleById = computed(() => {
  const map = new Map<string, string>();
  for (const item of props.conversationItems || []) {
    const conversationId = String(item.conversationId || "").trim();
    if (!conversationId) continue;
    map.set(conversationId, resolveConversationDisplayTitle(item, {
      locale: locale.value,
      untitledLabel: t("chat.untitledConversation"),
      systemNotificationLabel: t("chat.systemPersona"),
    }));
  }
  return map;
});

const conversationOrderById = computed(() => {
  const map = new Map<string, number>();
  (props.conversationItems || []).forEach((item, index) => {
    const conversationId = String(item.conversationId || "").trim();
    if (conversationId && !map.has(conversationId)) {
      map.set(conversationId, index);
    }
  });
  return map;
});

/** 全局定时器没有目标会话，统一挂在「系统通知」那一段：它们就在那里起委托 */
const GLOBAL_SECTION_KEY = "conversation:global";

const groupedTimerSections = computed<TimerSection[]>(() => {
  const query = normalizedSearchQuery.value;
  const sections = new Map<string, TimerSection>();
  for (const timer of timers.value) {
    const conversationId = String(timer.conversationId || "").trim();
    const sectionKey = conversationId ? `conversation:${conversationId}` : GLOBAL_SECTION_KEY;
    const title = conversationId
      ? conversationTitleById.value.get(conversationId) || t("chat.timerSidebar.unknownConversation")
      : t("chat.timerSidebar.systemConversation");
    if (query && !timerMatchesSearch(timer, title, query)) continue;
    if (!sections.has(sectionKey)) {
      sections.set(sectionKey, {
        key: sectionKey,
        title,
        items: [],
        order: conversationId ? conversationOrderById.value.get(conversationId) ?? 1_000_000 : -1,
      });
    }
    sections.get(sectionKey)?.items.push(timer);
  }
  return Array.from(sections.values())
    .map((section) => ({
      ...section,
      items: section.items.slice().sort(compareTimerForList),
    }))
    .sort((left, right) => left.order - right.order || left.title.localeCompare(right.title, locale.value));
});

function timerMatchesSearch(timer: TimerEntry, sectionTitle: string, query: string): boolean {
  return [sectionTitle, timer.message].some((value) =>
    String(value || "").toLocaleLowerCase().includes(query),
  );
}

function timerSortTime(timer: TimerEntry): number {
  const raw = String(timer.nextRunAt || timer.runAt || "").trim();
  if (!raw) return Number.POSITIVE_INFINITY;
  const time = new Date(raw).getTime();
  return Number.isFinite(time) ? time : Number.POSITIVE_INFINITY;
}

function compareTimerForList(left: TimerEntry, right: TimerEntry): number {
  const timeDiff = timerSortTime(left) - timerSortTime(right);
  if (timeDiff !== 0) return timeDiff;
  return String(left.createdAt || "").localeCompare(String(right.createdAt || ""));
}

function isTimerSectionCollapsed(key: string): boolean {
  return !!collapsedTimerSectionKeys.value[key];
}

function toggleTimerSection(key: string) {
  collapsedTimerSectionKeys.value = {
    ...collapsedTimerSectionKeys.value,
    [key]: !collapsedTimerSectionKeys.value[key],
  };
  emit("layoutChange");
}

function collapseAllTimerSections() {
  collapsedTimerSectionKeys.value = groupedTimerSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedTimerSectionKeys.value } as Record<string, boolean>);
  emit("layoutChange");
}

async function loadTimers() {
  loading.value = true;
  errorText.value = "";
  try {
    timers.value = await invokeTauri<TimerEntry[]>("timer.list", { conversationId: null });
    console.info("[侧栏定时器列表] 完成", {
      total: timers.value.length,
      enabled: timers.value.filter((timer) => timer.enabled).length,
    });
  } catch (error) {
    errorText.value = `${t("config.timer.listLoadFailed")}: ${toErrorMessage(error)}`;
  } finally {
    loading.value = false;
    emit("layoutChange");
  }
}

type MonitorChangedPayload = { domain?: string };

function handleTimerRefreshEvent(payload: MonitorChangedPayload | null | undefined) {
  if (String(payload?.domain || "").trim() !== "timer") return;
  void loadTimers();
}

let unlistenTimerChanged: (() => void) | null = null;
let unlistenTimerRecovered: (() => void) | null = null;

onMounted(() => {
  void loadTimers();
  unlistenTimerChanged = onTransportNotification("monitor.changed", handleTimerRefreshEvent);
  unlistenTimerRecovered = onTransportRecovered(() => {
    void loadTimers();
  });
});

onBeforeUnmount(() => {
  unlistenTimerChanged?.();
  unlistenTimerChanged = null;
  unlistenTimerRecovered?.();
  unlistenTimerRecovered = null;
});
</script>
