<template>
  <div
    class="navbar min-h-10 h-10 shrink-0 px-2 relative z-40 overflow-visible select-none"
    :class="viewMode === 'chat' ? '' : 'bg-base-200 border-b border-base-300'"
  >
    <div v-if="viewMode !== 'chat'" data-tauri-drag-region class="absolute inset-0 cursor-move" aria-hidden="true"></div>

    <div v-if="viewMode === 'chat'" class="relative z-10 flex min-w-0 flex-none items-center gap-1" @mousedown.stop>
      <div ref="conversationListPopoverRef" class="relative">
        <button
          class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
          :title="t('chat.conversationList')"
          :disabled="sideConversationListVisible"
          @click.stop="toggleConversationList"
        >
          <TextAlignJustify class="h-3.5 w-3.5" />
        </button>
        <div v-if="conversationListOpen" class="absolute left-0 top-full z-50 mt-2">
          <ChatConversationListCard
            :items="conversationItems"
            :active-conversation-id="activeConversationId"
            :user-alias="userAlias"
            :persona-name-map="personaNameMap"
            :persona-avatar-url-map="personaAvatarUrlMap"
            :user-avatar-url="userAvatarUrl"
            @select-conversation="handleConversationListSelect"
          />
        </div>
      </div>

      <button
        class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
        :title="t('chat.newConversation')"
        @click.stop="handleCreateConversation"
      >
        <SquarePen class="h-4 w-4" />
      </button>

      <button
        class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
        :disabled="forcingArchive || chatting"
        :title="forceArchiveTip"
        @click.stop="$emit('force-archive')"
      >
        <Minimize2 class="h-3.5 w-3.5 shrink-0" />
      </button>

      <div
        v-if="chatUsagePercent >= 50"
        class="inline-flex h-8 items-center px-2 text-sm font-semibold"
        :title="forceArchiveTip"
      >
        <span
          :class="[
            chatUsagePercent >= 70
              ? 'text-warning'
              : 'text-base-content',
          ]"
        >{{ chatUsagePercent }}%</span>
      </div>
    </div>

    <div
      v-if="viewMode === 'chat'"
      data-tauri-drag-region
      class="min-w-0 flex-1 self-stretch cursor-move"
      aria-hidden="true"
    ></div>

    <div
      v-if="viewMode === 'chat'"
      class="pointer-events-none absolute left-1/2 top-1/2 z-0 flex max-w-[40%] -translate-x-1/2 -translate-y-1/2 items-center px-2"
      :title="currentPersonaName"
    >
      <span class="truncate text-sm font-semibold text-base-content">{{ currentPersonaName }}</span>
    </div>

    <div v-if="viewMode !== 'chat'" class="pointer-events-none absolute left-1/2 top-1/2 z-0 flex -translate-x-1/2 -translate-y-1/2 items-center px-2">
      <span class="font-semibold text-sm">{{ titleText }}</span>
    </div>

    <div class="relative z-10 ml-auto flex flex-none gap-1" @mousedown.stop>
      <button
        v-if="viewMode === 'chat'"
        class="btn btn-ghost btn-sm"
        :title="t('window.archivesTitle')"
        @click.stop="$emit('open-archives')"
      >
        <History class="h-3.5 w-3.5" />
      </button>
      <button
        v-if="viewMode === 'chat'"
        class="btn btn-ghost btn-sm"
        :title="openConfigTitle"
        @click.stop="$emit('open-config')"
      >
        <Settings class="h-3.5 w-3.5" />
      </button>
      <button
        class="btn btn-ghost btn-sm"
        title="最小化"
        @click.stop="$emit('minimize-window')"
        :disabled="!windowReady"
      >
        <Minus class="h-3.5 w-3.5" />
      </button>
      <button
        class="btn btn-ghost btn-sm"
        :title="maximized ? '还原窗口' : '最大化'"
        @click.stop="$emit('toggle-maximize-window')"
        :disabled="!windowReady"
      >
        <Square class="h-3.5 w-3.5" />
      </button>
      <button
        class="btn btn-sm btn-ghost hover:bg-error"
        :title="closeTitle || 'Close'"
        @click.stop="$emit('close-window')"
        :disabled="!windowReady"
      >
        <X class="h-3.5 w-3.5" />
      </button>
    </div>
  </div>

  <dialog v-if="viewMode === 'chat'" class="modal" :class="{ 'modal-open': createConversationDialogOpen }">
    <div class="modal-box max-w-md">
      <h3 class="text-base font-semibold">{{ t("chat.newConversation") }}</h3>
      <div class="mt-3 flex flex-col gap-3">
        <div class="rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-sm leading-6 text-base-content/80">
          {{ t("chat.unarchivedConversationMemoryReminder") }}
        </div>
        <input
          ref="createConversationInputRef"
          v-model="createConversationTitle"
          type="text"
          class="input input-bordered w-full"
          :placeholder="t('chat.newConversationTopicPlaceholder')"
          @keydown="handleCreateConversationDialogKeydown"
        />
        <select
          v-model="createConversationDepartmentId"
          class="select select-bordered w-full"
        >
          <option
            v-for="department in createConversationDepartmentOptions"
            :key="department.id"
            :value="department.id"
          >
            {{ departmentOptionLabel(department) }}
          </option>
        </select>
        <div v-if="recentConversationTopics.length > 0" class="flex flex-col gap-2">
          <div class="text-xs font-medium opacity-70">{{ t("chat.recentConversationTopics") }}</div>
          <div class="flex flex-wrap gap-2">
            <button
              v-for="topic in recentConversationTopics"
              :key="topic"
              type="button"
              class="btn btn-sm btn-ghost"
              @click="applyRecentConversationTopic(topic)"
            >
              {{ topic }}
            </button>
          </div>
        </div>
      </div>
      <div class="modal-action">
        <button class="btn btn-sm" @click="closeCreateConversationDialog">{{ t("common.cancel") }}</button>
        <button class="btn btn-sm btn-primary" @click="confirmCreateConversation">{{ t("common.confirm") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeCreateConversationDialog">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { History, Minimize2, Minus, Settings, Square, SquarePen, TextAlignJustify, X } from "lucide-vue-next";
import type { ChatConversationOverviewItem } from "../../../types/app";
import ChatConversationListCard from "../../chat/components/ChatConversationListCard.vue";

type ConversationDepartmentOption = {
  id: string;
  name: string;
  ownerName: string;
};

const RECENT_CONVERSATION_TOPICS_STORAGE_KEY = "easy_call.recent_conversation_topics.v1";
const RECENT_CONVERSATION_TOPICS_LIMIT = 7;

const props = defineProps<{
  viewMode: "chat" | "archives" | "config";
  titleText: string;
  chatUsagePercent: number;
  forcingArchive: boolean;
  chatting: boolean;
  currentPersonaName: string;
  sideConversationListVisible: boolean;
  activeConversationId: string;
  conversationItems: ChatConversationOverviewItem[];
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  createConversationDepartmentOptions: ConversationDepartmentOption[];
  defaultCreateConversationDepartmentId: string;
  forceArchiveTip: string;
  maximized: boolean;
  windowReady: boolean;
  openConfigTitle: string;
  closeTitle?: string;
}>();

const emit = defineEmits<{
  (e: "open-config"): void;
  (e: "open-archives"): void;
  (e: "minimize-window"): void;
  (e: "toggle-maximize-window"): void;
  (e: "switch-conversation", conversationId: string): void;
  (e: "create-conversation", input?: { title?: string; departmentId?: string }): void;
  (e: "force-archive"): void;
  (e: "close-window"): void;
}>();

const { t } = useI18n();
const conversationListOpen = ref(false);
const conversationListPopoverRef = ref<HTMLElement | null>(null);
const createConversationInputRef = ref<HTMLInputElement | null>(null);
const recentConversationTopics = ref<string[]>([]);
const createConversationDialogOpen = ref(false);
const createConversationTitle = ref("");
const createConversationDepartmentId = ref("");

function loadRecentConversationTopics() {
  try {
    const raw = window.localStorage.getItem(RECENT_CONVERSATION_TOPICS_STORAGE_KEY);
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
      if (normalized.length >= RECENT_CONVERSATION_TOPICS_LIMIT) break;
    }
    recentConversationTopics.value = normalized;
  } catch {
    recentConversationTopics.value = [];
  }
}

function saveRecentConversationTopics() {
  try {
    window.localStorage.setItem(RECENT_CONVERSATION_TOPICS_STORAGE_KEY, JSON.stringify(recentConversationTopics.value));
  } catch {
    // ignore persistence failures
  }
}

function pushRecentConversationTopic(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  recentConversationTopics.value = [text, ...recentConversationTopics.value.filter((item) => item !== text)].slice(0, RECENT_CONVERSATION_TOPICS_LIMIT);
  saveRecentConversationTopics();
}

function closeConversationList() {
  conversationListOpen.value = false;
}

function toggleConversationList() {
  conversationListOpen.value = !conversationListOpen.value;
}

function handleConversationListSelect(conversationId: string) {
  closeConversationList();
  emit("switch-conversation", String(conversationId || "").trim());
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!conversationListOpen.value) return;
  const target = event.target as Node | null;
  const root = conversationListPopoverRef.value;
  if (root && target && !root.contains(target)) {
    closeConversationList();
  }
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && conversationListOpen.value) {
    closeConversationList();
  }
}

function handleCreateConversation() {
  closeConversationList();
  createConversationTitle.value = "";
  createConversationDepartmentId.value =
    String(props.defaultCreateConversationDepartmentId || "").trim()
    || String(props.createConversationDepartmentOptions[0]?.id || "").trim();
  createConversationDialogOpen.value = true;
  nextTick(() => createConversationInputRef.value?.focus());
}

function closeCreateConversationDialog() {
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
  createConversationDepartmentId.value = "";
}

function applyRecentConversationTopic(topic: string) {
  createConversationTitle.value = String(topic || "").trim();
  nextTick(() => createConversationInputRef.value?.focus());
}

function departmentOptionLabel(department: ConversationDepartmentOption): string {
  const departmentName = String(department.name || "").trim();
  const ownerName = String(department.ownerName || "").trim();
  return ownerName ? `${departmentName} / ${ownerName}` : departmentName;
}

function confirmCreateConversation() {
  const title = String(createConversationTitle.value || "").trim();
  const departmentId = String(createConversationDepartmentId.value || "").trim();
  if (title) {
    pushRecentConversationTopic(title);
  }
  createConversationDialogOpen.value = false;
  createConversationTitle.value = "";
  createConversationDepartmentId.value = "";
  emit("create-conversation", {
    title,
    departmentId: departmentId || undefined,
  });
}

function handleCreateConversationDialogKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    confirmCreateConversation();
  }
}

onMounted(() => {
  loadRecentConversationTopics();
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  window.addEventListener("keydown", handleWindowKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>
