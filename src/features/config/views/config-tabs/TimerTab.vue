<template>
  <div class="space-y-3">
    <div
      v-if="message"
      class="rounded-box border px-3 py-2 text-sm"
      :class="messageError ? 'border-error/40 bg-error/10 text-error' : 'border-base-300 bg-base-200/50'"
    >
      {{ message }}
    </div>

    <ConfigTemplate :model-value="{}" :groups="timerTemplateGroups">
      <template #group-actions-timer-list>
        <div class="flex items-center gap-2">
          <button class="btn btn-sm bg-base-100" :disabled="listLoading" @click="loadTimers()">
            <AppIcon name="refresh-cw" class="h-4 w-4" :class="{ 'animate-spin': listLoading }" />
            {{ t("common.refresh") }}
          </button>
        </div>
      </template>

      <template #row-timer-list>
        <div class="grid min-w-0 gap-3">
          <div class="tabs tabs-boxed inline-flex bg-base-200">
            <button
              v-for="tab in timerFilterTabs"
              :key="tab.id"
              class="tab"
              :class="{ 'tab-active': filter === tab.id }"
              @click="filter = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>

          <div v-if="listLoading && !timers.length" class="py-8 text-center text-sm opacity-60">
            {{ t("common.loading") }}
          </div>

          <div v-else-if="pagedTimers.length" class="divide-y divide-base-300/60">
            <button
              v-for="timer in pagedTimers"
              :key="timer.id"
              type="button"
              class="flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors hover:bg-base-200/50"
              :class="editorOpen && editorTimer?.id === timer.id ? 'bg-primary/10' : ''"
              @click="openEditEditor(timer)"
            >
              <!-- 状态只用一个图标：还会再跑 / 已经结束，不再堆状态文字 -->
              <span
                class="shrink-0"
                :title="timerIsFinished(timer) ? t('config.timer.finished') : t('config.timer.running')"
              >
                <AppIcon
                  :name="timerIsFinished(timer) ? 'check-circle' : 'clock'"
                  class="h-4 w-4"
                  :class="timerIsFinished(timer) ? 'text-base-content/35' : 'text-success'"
                />
              </span>
              <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ timer.message }}</span>
              <span class="badge badge-ghost badge-sm shrink-0">{{ timerTargetLabel(timer) }}</span>
              <span class="badge badge-ghost badge-sm shrink-0">{{ dispatchModeLabel(timer) }}</span>
            </button>
          </div>

          <div v-else class="py-6 text-center text-sm opacity-50">{{ t("config.timer.empty") }}</div>

          <div v-if="totalPages > 1" class="flex justify-center border-t border-base-300 px-3 py-2">
            <div class="join">
              <button class="btn btn-sm join-item" :disabled="page <= 1" @click="page -= 1">‹</button>
              <button class="btn btn-sm join-item btn-active">{{ page }} / {{ totalPages }}</button>
              <button class="btn btn-sm join-item" :disabled="page >= totalPages" @click="page += 1">›</button>
            </div>
          </div>
        </div>
      </template>
    </ConfigTemplate>

    <TimerCreateCard
      :open="editorOpen"
      mode="edit"
      :timer="editorTimer"
      @close="closeEditor"
      @saved="handleSaved"
      @enabled-change="handleEnabledChange"
      @deleted="handleDeleted"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import type { UnarchivedConversationSummary } from "../../../../types/app";
import { indexConversations, type IndexedConversation } from "../../../shared/components/conversation-picker";
import TimerCreateCard from "../../../chat/components/dialogs/TimerCreateCard.vue";
import { timerIsFinished, type TimerEntry } from "../../../chat/utils/timer-entry";

type TimerFilter = "all" | "active" | "disabled";

const PAGE_SIZE = 5;

const { t } = useI18n();

const timers = ref<TimerEntry[]>([]);
const conversationIndex = ref<IndexedConversation[]>([]);
const listLoading = ref(false);
const message = ref("");
const messageError = ref(false);

const filter = ref<TimerFilter>("all");
const page = ref(1);

const editorOpen = ref(false);
const editorTimer = ref<TimerEntry | null>(null);

const timerFilterTabs = computed(() => [
  { id: "all" as const, label: t("common.all") },
  { id: "active" as const, label: t("config.timer.enabled") },
  { id: "disabled" as const, label: t("config.timer.disabled") },
]);

const timerTemplateGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "timer-list",
    title: t("config.timer.title"),
    rows: [
      {
        key: "timer-list",
        items: [],
      },
    ],
  },
]);

const filteredTimers = computed(() => {
  if (filter.value === "active") return timers.value.filter((item) => item.enabled);
  if (filter.value === "disabled") return timers.value.filter((item) => !item.enabled);
  return timers.value;
});

const totalPages = computed(() => Math.max(1, Math.ceil(filteredTimers.value.length / PAGE_SIZE)));

const pagedTimers = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return filteredTimers.value.slice(start, start + PAGE_SIZE);
});

watch(filter, () => {
  page.value = 1;
});

watch(totalPages, (next) => {
  if (page.value > next) {
    page.value = next;
  }
});

function setMessage(text: string, isError = false) {
  message.value = text;
  messageError.value = isError;
}

async function loadTimers() {
  listLoading.value = true;
  try {
    timers.value = await invokeTauri<TimerEntry[]>("timer.list", { conversationId: null });
  } catch (error) {
    setMessage(`${t("config.timer.listLoadFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    listLoading.value = false;
  }
}

/** 目标会话只存 id；列表里用与选择器同一套「会话 #N」编号，两边才对得上同一个会话 */
async function loadConversationIndex() {
  try {
    const items = await invokeTauri<UnarchivedConversationSummary[]>("conversation.overview.list");
    conversationIndex.value = indexConversations(items);
  } catch {
    conversationIndex.value = [];
  }
}

function timerTargetLabel(timer: TimerEntry) {
  const conversationId = String(timer.conversationId || "").trim();
  if (!conversationId) return t("config.timer.globalTarget");
  const matched = conversationIndex.value.find((item) => item.id === conversationId);
  if (matched) return t("chat.conversationPicker.label", { index: matched.index });
  // 会话已归档或不在列表里时不铺整条 UUID，用短号让人能对上号就行。
  return conversationId.slice(0, 8);
}

function dispatchModeLabel(timer: TimerEntry) {
  return timer.dispatchMode === "delegate"
    ? t("config.timer.dispatchDelegate")
    : t("config.timer.dispatchDirect");
}

function openEditEditor(timer: TimerEntry) {
  editorTimer.value = timer;
  editorOpen.value = true;
}

function closeEditor() {
  editorOpen.value = false;
  editorTimer.value = null;
}

function handleSaved(savedTimer?: TimerEntry) {
  if (savedTimer && editorTimer.value && editorTimer.value.id === savedTimer.id) {
    editorTimer.value = savedTimer;
  }
  setMessage(t("config.timer.saved"));
  void loadTimers();
}

/** 只拨了启用开关：弹窗留着，提示也换成启用/停用的说法 */
function handleEnabledChange(updated: TimerEntry) {
  if (editorTimer.value && editorTimer.value.id === updated.id) {
    editorTimer.value = updated;
  }
  setMessage(updated.enabled ? t("config.timer.resumed") : t("config.timer.paused"));
  void loadTimers();
}

function handleDeleted() {
  editorOpen.value = false;
  editorTimer.value = null;
  setMessage(t("config.timer.deleted"));
  void loadTimers();
}

onMounted(() => {
  void loadTimers();
  void loadConversationIndex();
});
</script>
