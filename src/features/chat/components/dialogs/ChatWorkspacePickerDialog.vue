<template>
  <div
    v-if="open"
    class="fixed inset-0 z-80 flex items-center justify-center bg-black/30 px-4 py-8"
    @click.self="emit('close')"
  >
    <div class="w-full max-w-2xl rounded-2xl border border-base-300 bg-base-100 shadow-2xl">
      <div class="flex items-start justify-between gap-3 border-b border-base-300 px-4 py-3">
        <div class="min-w-0">
          <div class="text-sm font-semibold">{{ t("chat.workspacePickerTitle") }}</div>
          <div class="mt-1 text-xs opacity-70">{{ t("chat.workspacePickerHint") }}</div>
        </div>
        <label
          class="flex max-w-[14rem] shrink-0 cursor-pointer items-center gap-2 rounded-full bg-base-200 px-3 py-2 text-xs font-medium leading-tight"
          :title="t('chat.workspacePickerAutonomousHint')"
        >
          <span class="whitespace-normal">{{ t("chat.workspacePickerAutonomous") }}</span>
          <input
            type="checkbox"
            class="toggle toggle-primary toggle-sm"
            :checked="autonomousMode"
            :disabled="saving"
            @change="onAutonomousModeChange"
          />
        </label>
      </div>
      <div class="max-h-[65vh] overflow-y-auto">
        <div
          v-if="workspaces.length === 0"
          class="m-4 rounded-box border border-dashed border-base-300 bg-base-200/20 px-4 py-6 text-center text-sm opacity-70"
        >
          {{ t("chat.workspacePickerEmpty") }}
        </div>
        <div v-else class="divide-y divide-base-300">
          <div
            v-for="item in workspaces"
            :key="item.id"
            class="px-3 py-3 text-left"
            :title="item.path"
          >
            <div class="flex items-center gap-3">
              <div class="min-w-0 flex-1 text-left">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="inline-block w-40 truncate font-medium align-middle" :title="item.path">{{ item.name }}</span>
                  <span class="badge" :class="levelClass(item.level)">{{ levelLabel(item.level) }}</span>
                  <span class="badge" :class="accessClass(item.access)">{{ accessLabel(item.access) }}</span>
                </div>
              </div>
              <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
                <button
                  v-if="item.level !== 'system' && item.level !== 'main'"
                  class="btn btn-sm btn-ghost"
                  type="button"
                  :disabled="saving"
                  :title="t('config.tools.setWorkspaceAsMain')"
                  @click="emit('setMain', item.id)"
                >
                  <House class="h-4 w-4" />
                </button>
                <button
                  v-else-if="item.level === 'main'"
                  class="btn btn-sm btn-primary pointer-events-none opacity-100"
                  type="button"
                  aria-disabled="true"
                  tabindex="-1"
                  :title="t('config.tools.currentMainWorkspace')"
                >
                  <House class="h-4 w-4" />
                </button>
                <select
                  v-if="item.level !== 'system'"
                  class="select select-sm select-bordered w-32"
                  :disabled="saving"
                  :value="item.access"
                  @change="onAccessChange(item.id, $event)"
                >
                  <option value="full_access">{{ accessLabel("full_access") }}</option>
                  <option value="approval">{{ accessLabel("approval") }}</option>
                  <option value="read_only">{{ accessLabel("read_only") }}</option>
                </select>
                <button
                  v-if="item.level !== 'system'"
                  class="btn btn-sm btn-ghost text-error"
                  type="button"
                  :disabled="saving"
                  :title="t('config.tools.delete')"
                  @click="emit('removeWorkspace', item.id)"
                >
                  <Trash2 class="h-4 w-4" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="flex items-center justify-between gap-3 border-t border-base-300 px-4 py-3">
        <button class="btn btn-sm" type="button" :disabled="saving" @click="emit('addWorkspace')">
          {{ t("config.tools.addWorkspace") }}
        </button>
        <div class="flex items-center gap-2">
          <button class="btn btn-sm btn-ghost" type="button" :disabled="saving" @click="emit('close')">
          {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-primary" type="button" :disabled="saving" @click="emit('save')">
            {{ saving ? t("common.saving") : t("common.save") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { House, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { ChatWorkspaceChoice } from "../../composables/use-chat-workspace";

defineProps<{
  open: boolean;
  saving: boolean;
  workspaces: ChatWorkspaceChoice[];
  autonomousMode: boolean;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "addWorkspace"): void;
  (e: "setMain", workspaceId: string): void;
  (e: "setAccess", workspaceId: string, access: ChatWorkspaceChoice["access"]): void;
  (e: "setAutonomousMode", enabled: boolean): void;
  (e: "removeWorkspace", workspaceId: string): void;
  (e: "save"): void;
}>();

const { t } = useI18n();

function levelLabel(level: string): string {
  if (level === "system") return t("config.tools.workspaceLevelSystem");
  if (level === "main") return t("config.tools.workspaceLevelMain");
  return t("config.tools.workspaceLevelSecondary");
}

function levelClass(level: string): string {
  if (level === "main") return "badge-primary";
  if (level === "secondary") return "badge-secondary";
  return "badge-ghost";
}

function accessLabel(access: string): string {
  if (access === "approval") return t("config.tools.workspaceAccessApproval");
  if (access === "full_access") return t("config.tools.workspaceAccessFullAccess");
  return t("config.tools.workspaceAccessReadOnly");
}

function accessClass(access: string): string {
  if (access === "approval") return "badge-warning";
  if (access === "full_access") return "badge-success";
  return "badge-ghost";
}

function onAccessChange(workspaceId: string, event: Event) {
  const nextAccess = String((event.target as HTMLSelectElement | null)?.value || "").trim();
  if (nextAccess !== "approval" && nextAccess !== "full_access" && nextAccess !== "read_only") {
    return;
  }
  emit("setAccess", workspaceId, nextAccess);
}

function onAutonomousModeChange(event: Event) {
  emit("setAutonomousMode", Boolean((event.target as HTMLInputElement | null)?.checked));
}
</script>
