<template>
  <CardShell
    variant="wide"
    tone="warning"
    :icon="GitBranch"
    :label="branch || t('chat.homePanel.gitChanges')"
    interactive
    @select="emit('openChanges')"
  >
    <template #trailing>
      <span class="shrink-0 text-xs text-base-content/45">{{ summary }}</span>
    </template>
    <ul v-if="changes.length" class="menu menu-xs w-full gap-0.5 p-0">
      <li v-for="change in visibleChanges" :key="change.path">
        <button
          type="button"
          class="min-w-0 gap-2 font-normal hover:bg-base-200/80 rounded"
          :title="change.path"
          @click.stop="handleOpenFile(change.path)"
        >
          <span class="ecall-home-status w-2.5" :class="statusClass(change.status)">{{ statusLabel(change.status) }}</span>
          <span class="min-w-0 flex-1 truncate text-left text-base-content/80">{{ baseName(change.path) }}</span>
        </button>
      </li>
    </ul>
    <div v-else class="flex flex-1 items-center justify-center text-xs text-base-content/40">
      {{ workspaceRootPath ? t("chat.homePanel.noChanges") : t("chat.homePanel.noWorkspace") }}
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { GitBranch } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const MAX_VISIBLE = 4;

const props = withDefaults(defineProps<{
  workspaceRootPath?: string;
  branch?: string;
  changes?: Array<{ path: string; status: string }>;
  changeCount?: number;
}>(), {
  workspaceRootPath: "",
  branch: "",
  changes: () => [],
  changeCount: 0,
});

const emit = defineEmits<{
  (e: "openChanges"): void;
  (e: "openFile", path: string): void;
}>();

function handleOpenFile(path: string) {
  const root = String(props.workspaceRootPath || "").replace(/[\\/]+$/, "");
  const rel = String(path || "").replace(/^[\\/]+/, "");
  const fullPath = root ? `${root}/${rel}` : rel;
  emit("openFile", fullPath);
}

const { t } = useI18n();

const visibleChanges = computed(() => props.changes.slice(0, MAX_VISIBLE));
const summary = computed(() => {
  if (!props.workspaceRootPath) return "";
  if (!Number(props.changeCount || 0)) return t("chat.homePanel.cleanWorktree");
  return t("chat.homePanel.changeCount", { n: Number(props.changeCount || 0) });
});

/** Git 状态码转单字母，与 Git 面板的语义保持一致。 */
function statusLabel(status: string): string {
  const code = String(status || "").trim().charAt(0).toUpperCase();
  if (code === "A" || code === "?") return "A";
  if (code === "D") return "D";
  if (code === "R") return "R";
  return "M";
}

function statusClass(status: string): string {
  const label = statusLabel(status);
  if (label === "A") return "text-success";
  if (label === "D") return "text-error";
  if (label === "R") return "text-info";
  return "text-warning";
}

function baseName(path: string): string {
  const normalized = String(path || "").replace(/\\/g, "/");
  return normalized.split("/").filter(Boolean).pop() || normalized;
}
</script>

<style scoped>
.ecall-home-status {
  flex-shrink: 0;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
</style>
