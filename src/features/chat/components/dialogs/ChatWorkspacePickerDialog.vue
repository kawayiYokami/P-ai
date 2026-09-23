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
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <WorkspaceConfigCard
          :main-path="mainPath"
          :secondary-paths="secondaryPaths"
          :access="unifiedAccess"
          :work-mode="workMode"
          :selected-branch="selectedBranch"
          :selected-worktree-path="worktreePath"
          :available-workspaces="availableWorkspaceOptions"
          :hide-add-workspace="hideAddWorkspace"
          :sync-workspace-branch="syncWorkspaceBranch"
          @update:main-path="onMainPathUpdate"
          @update:access="onAccessUpdate"
          @update:work-mode="emit('setWorkMode', $event)"
          @update:branch="emit('setBranch', $event)"
          @update:worktree-path="onWorktreePathUpdate"
          @add-secondary="onAddSecondary"
          @remove-secondary="onRemoveSecondary"
        />
        <div v-if="validationMessage" class="mt-3 rounded-field bg-error/10 px-3 py-2 text-xs text-error">
          {{ validationMessage }}
        </div>
      </div>
      <div class="flex shrink-0 items-center justify-between gap-3 border-t border-base-300 px-4 py-3">
        <label
          class="flex cursor-pointer items-center gap-2 text-xs font-medium"
          :title="t('chat.workspacePickerAutonomousHint')"
        >
          <input
            type="checkbox"
            class="checkbox checkbox-primary checkbox-sm"
            :checked="autonomousMode"
            :disabled="saving"
            @change="onAutonomousModeChange"
          />
          <span>{{ t("chat.workspacePickerAutonomous") }}</span>
        </label>
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
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import WorkspaceConfigCard from "../../../shared/components/WorkspaceConfigCard.vue";
import type { ChatWorkspaceChoice } from "../../composables/use-chat-workspace";
import type { ShellWorkMode } from "../../../../types/app";
import { normalizeWorkspaceAccess } from "../../../../utils/shell-workspaces";

const props = withDefaults(defineProps<{
  open: boolean;
  saving: boolean;
  workspaces: ChatWorkspaceChoice[];
  autonomousMode: boolean;
  workMode: ShellWorkMode;
  worktreePath?: string;
  worktreeExists?: boolean;
  worktreeAvailable?: boolean;
  worktreeCheckMessage?: string;
  validationMessage?: string;
  hideAddWorkspace?: boolean;
  selectedBranch?: string;
  /** 对话框内切换分支成功后按目标目录同步「本会话工作分支」记录 */
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
}>(), {
  hideAddWorkspace: false,
  selectedBranch: "",
  worktreePath: "",
  worktreeExists: false,
  worktreeAvailable: false,
});

const emit = defineEmits<{
  (e: "close"): void;
  (e: "addWorkspace"): void;
  (e: "setMain", workspaceId: string): void;
  (e: "setAccess", workspaceId: string, access: ChatWorkspaceChoice["access"]): void;
  (e: "setAccessUnified", access: ChatWorkspaceChoice["access"]): void;
  (e: "setAutonomousMode", enabled: boolean): void;
  (e: "setWorkMode", mode: ShellWorkMode): void;
  (e: "setBranch", branch: string): void;
  /** 第二级选中工作树后同步给上层（保存时写入 shell_worktree_path；空串=主工作树） */
  (e: "setWorktreePath", worktreePath: string): void;
  (e: "removeWorkspace", workspaceId: string): void;
  (e: "openDir", workspaceId: string): void;
  (e: "save"): void;
  (e: "addSecondary", path: string): void;
  (e: "removeSecondary", path: string): void;
  (e: "updateMainPath", path: string): void;
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);

const mainPath = computed(() => {
  const main = props.workspaces.find((w) => w.level === "main");
  if (main) return String(main.path || "").trim();
  return String(props.workspaces[0]?.path || "").trim();
});

const secondaryPaths = computed(() => {
  return props.workspaces
    .filter((w) => w.level === "secondary")
    .map((w) => String(w.path || "").trim())
    .filter(Boolean);
});

const unifiedAccess = computed<ChatWorkspaceChoice["access"]>(() => {
  const main = props.workspaces.find((w) => w.level === "main");
  const raw = String(main?.access || props.workspaces[0]?.access || "approval").trim();
  return normalizeWorkspaceAccess(raw) as ChatWorkspaceChoice["access"];
});

const availableWorkspaceOptions = computed(() => {
  return props.workspaces.map((w) => ({
    id: w.id,
    name: w.name,
    path: w.path,
    access: w.access,
  }));
});

function onDialogClose() {
  if (props.saving) return;
  emit("close");
}

function onMainPathUpdate(path: string) {
  const normalized = String(path || "").trim();
  if (!normalized) return;
  const matched = props.workspaces.find((w) => w.path.toLowerCase() === normalized.toLowerCase());
  if (matched) {
    emit("setMain", matched.id);
  } else {
    emit("updateMainPath", normalized);
  }
}

function onAccessUpdate(access: ChatWorkspaceChoice["access"]) {
  const normalized = normalizeWorkspaceAccess(String(access || ""));
  emit("setAccessUnified", normalized as ChatWorkspaceChoice["access"]);
  const main = props.workspaces.find((w) => w.level === "main") || props.workspaces[0];
  if (main) emit("setAccess", main.id, normalized as ChatWorkspaceChoice["access"]);
}

function onWorktreePathUpdate(payload: { worktreePath: string; branch: string; isMain: boolean }) {
  const path = String(payload?.worktreePath || "").trim();
  if (!path) return;
  emit("setWorkMode", payload.isMain ? "directory" : "worktree");
  emit("setWorktreePath", payload.isMain ? "" : path);
  if (payload.branch) {
    emit("setBranch", payload.branch);
  }
}

function onAddSecondary(path: string) {
  emit("addSecondary", path);
  emit("addWorkspace");
}

function onRemoveSecondary(path: string) {
  const normalized = String(path || "").trim();
  const matched = props.workspaces.find((w) => w.path.toLowerCase() === normalized.toLowerCase());
  if (matched) emit("removeWorkspace", matched.id);
  emit("removeSecondary", normalized);
}

function onAutonomousModeChange(event: Event) {
  emit("setAutonomousMode", Boolean((event.target as HTMLInputElement | null)?.checked));
}
</script>
