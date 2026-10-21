<template>
  <dialog
    ref="dialogRef"
    class="modal"
    :open="open"
    @close="onDialogClose"
    @cancel.prevent="onDialogClose"
    @keydown.esc.prevent="onDialogClose"
  >
    <div class="modal-box flex max-h-[calc(100dvh-4rem)] w-full max-w-xl flex-col overflow-hidden p-0">
      <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 px-4 py-3">
        <div class="text-sm font-medium text-base-content/85">{{ t("chat.teamManage.title") }}</div>
        <button class="btn btn-sm btn-ghost" type="button" @click="emit('close')">
          {{ t("common.close") }}
        </button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <label class="flex cursor-pointer items-center justify-between gap-3">
          <span class="text-xs font-medium text-base-content/80">{{ t("chat.teamManage.recruitSwitch") }}</span>
          <input
            type="checkbox"
            class="toggle toggle-primary toggle-sm"
            :checked="Boolean(teamEnabled)"
            :disabled="saving"
            @change="onSwitchChange"
          />
        </label>
        <div class="mt-2 rounded-field bg-base-200/70 px-3 py-2 text-xs text-base-content/55">
          {{ t("chat.teamManage.recruitOffHint") }}
        </div>
        <div class="mt-4">
          <div class="mb-2 text-xs font-medium text-base-content/70">
            {{ t("chat.teamManage.memberList", { count: members.length }) }}
          </div>
          <div v-if="members.length === 0" class="rounded-field border border-dashed border-base-content/15 px-3 py-4 text-center text-xs text-base-content/45">
            {{ t("chat.teamManage.empty") }}
          </div>
          <div v-else class="flex flex-col gap-1.5">
            <div
              v-for="member in members"
              :key="member.conversationId"
              class="flex items-center gap-2 rounded-field border border-base-content/10 bg-base-100 px-3 py-2"
            >
              <span class="badge badge-sm badge-primary">{{ member.teamNumber }} 号</span>
              <span class="min-w-0 flex-1 truncate text-xs font-medium text-base-content/85">{{ member.teamName || member.title }}</span>
              <span v-if="member.agentName" class="max-w-40 truncate text-xs text-base-content/45">{{ member.agentName }}</span>
            </div>
          </div>
        </div>
        <div class="mt-4">
          <div class="mb-2 flex items-center justify-between gap-2">
            <span class="text-xs font-medium text-base-content/70">{{ t("chat.teamManage.recruitTitle") }}</span>
            <button
              class="btn btn-sm btn-primary shrink-0"
              type="button"
              :disabled="selectedAgentIds.length === 0 || recruiting"
              @click="onRecruit"
            >
              {{ recruiting ? t("common.saving") : t("chat.teamManage.recruitAction", { count: selectedAgentIds.length }) }}
            </button>
          </div>
          <div v-if="recruitableOptions.length === 0" class="rounded-field border border-dashed border-base-content/15 px-3 py-4 text-center text-xs text-base-content/45">
            {{ t("chat.teamManage.noRecruitable") }}
          </div>
          <div v-else class="flex max-w-full flex-wrap items-stretch justify-start gap-3">
            <div
              v-for="option in recruitableOptions"
              :key="option.agentId"
              class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-base-300 bg-base-100/60 px-1 py-2.5 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-base-100 hover:shadow-lg"
              :class="isSelected(option.agentId) ? 'border-primary/60 bg-primary/10' : ''"
            >
              <button
                type="button"
                class="flex w-full flex-col items-center gap-1.5"
                @click="toggleSelect(option.agentId)"
              >
                <div class="avatar">
                  <div
                    class="h-14 w-14 rounded-full transition-shadow"
                    :class="isSelected(option.agentId) ? 'ring-2 ring-primary' : ''"
                  >
                    <img
                      v-if="resolveAvatarUrl(option.agentId)"
                      :src="resolveAvatarUrl(option.agentId)"
                      :alt="option.agentName"
                      class="h-14 w-14 rounded-full object-cover"
                    />
                    <div
                      v-else
                      class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/80 text-lg font-semibold text-primary-content"
                    >
                      {{ agentInitials(option.agentName) }}
                    </div>
                  </div>
                </div>
                <span class="max-w-full truncate text-center text-xs leading-tight text-base-content/80">
                  {{ option.agentName }}
                </span>
              </button>
            </div>
          </div>
        </div>
        <div v-if="errorMessage" class="mt-3 rounded-field bg-error/10 px-3 py-2 text-xs text-error">
          {{ errorMessage }}
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { AgentPersonaOption } from "../../../shared/agent-persona-options";

export type TeamManageMember = {
  conversationId: string;
  title: string;
  teamNumber?: number;
  teamName?: string;
  agentId?: string;
  agentName?: string;
};

const props = withDefaults(defineProps<{
  open: boolean;
  teamEnabled?: boolean;
  members?: TeamManageMember[];
  agentOptions?: AgentPersonaOption[];
  /** 当前会话人格：招募列表里排除它，不能招自己 */
  currentAgentId?: string;
  avatarUrlMap?: Record<string, string>;
  saving?: boolean;
  recruiting?: boolean;
  errorMessage?: string;
}>(), {
  teamEnabled: false,
  members: () => [],
  agentOptions: () => [],
  currentAgentId: "",
  avatarUrlMap: () => ({}),
  saving: false,
  recruiting: false,
  errorMessage: "",
});

const emit = defineEmits<{
  (e: "close"): void;
  (e: "setTeamEnabled", enabled: boolean): void;
  (e: "recruit", agentIds: string[]): void;
}>();

const { t } = useI18n();

const dialogRef = ref<HTMLDialogElement | null>(null);
const selectedAgentIds = ref<string[]>([]);

const recruitableOptions = computed(() => {
  const current = String(props.currentAgentId || "").trim();
  return (props.agentOptions || []).filter((option) => {
    const id = String(option.agentId || "").trim();
    if (!id) return false;
    return id !== current;
  });
});

function isSelected(agentId: string): boolean {
  return selectedAgentIds.value.includes(String(agentId || "").trim());
}

function toggleSelect(agentId: string) {
  const id = String(agentId || "").trim();
  if (!id) return;
  const index = selectedAgentIds.value.indexOf(id);
  if (index >= 0) {
    selectedAgentIds.value.splice(index, 1);
  } else {
    selectedAgentIds.value.push(id);
  }
}

function resolveAvatarUrl(agentId: string): string {
  return props.avatarUrlMap?.[String(agentId || "").trim()] || "";
}

function agentInitials(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  const firstTwo = text.slice(0, 2);
  if (/^[A-Za-z]{2}/.test(firstTwo)) {
    return firstTwo.toUpperCase();
  }
  return text.charAt(0).toUpperCase();
}

watch(
  () => props.open,
  (next) => {
    if (next) {
      selectedAgentIds.value = [];
      const dialog = dialogRef.value;
      if (dialog && !dialog.open) dialog.showModal();
    } else {
      const dialog = dialogRef.value;
      if (dialog?.open) dialog.close();
    }
  },
  { immediate: true },
);

// 招募列表随人格变化时，掉已不在列表里的勾选
watch(
  recruitableOptions,
  (next) => {
    const alive = new Set(next.map((option) => String(option.agentId || "").trim()));
    selectedAgentIds.value = selectedAgentIds.value.filter((id) => alive.has(id));
  },
);

function onDialogClose() {
  emit("close");
}

function onSwitchChange(event: Event) {
  const target = event.target as HTMLInputElement | null;
  emit("setTeamEnabled", Boolean(target?.checked));
}

function onRecruit() {
  const ids = selectedAgentIds.value.map((id) => String(id || "").trim()).filter(Boolean);
  if (ids.length === 0) return;
  emit("recruit", ids);
}
</script>
