<template>
  <CardShell
    variant="wide"
    tone="warning"
    :icon="GitBranch"
    :label="t('chat.homePanel.envLabel')"
    :interactive="false"
  >
    <div class="ecall-env-rows">
      <!-- 变更：改动概况，点击打开变更集合 -->
      <button
        type="button"
        class="ecall-env-row"
        :title="changeTitle"
        @click="emit('openChanges')"
      >
        <span class="ecall-env-icon ecall-env-icon-warning"><FileDiff class="size-3.5" /></span>
        <span class="ecall-env-label">{{ t("chat.homePanel.changesLabel") }}</span>
        <span class="ecall-env-value" :class="changeCount > 0 ? 'text-warning' : 'opacity-45'">{{ changeText }}</span>
      </button>

      <!-- 分支：点击就地展开下拉切换 -->
      <EcallDropdown
        v-model="branchPickerOpen"
        teleport
        :disabled="!repoRoot"
        root-class="min-w-0"
        panel-class="w-full"
      >
        <template #trigger="{ toggle, open }">
          <button
            type="button"
            class="ecall-env-row"
            :title="branchTitle"
            :disabled="!repoRoot"
            @click="toggle"
          >
            <span class="ecall-env-icon ecall-env-icon-primary"><GitBranch class="size-3.5" /></span>
            <span class="ecall-env-label">{{ t("chat.homePanel.branchLabel") }}</span>
            <span class="ecall-env-value min-w-0 truncate">{{ branchDisplay }}</span>
            <ChevronDown
              class="size-3 shrink-0 opacity-45 transition-transform"
              :class="{ 'rotate-180': open }"
            />
          </button>
        </template>
        <template #default>
          <OverlayScrollArea class="p-1" scroller-class="max-h-64 overscroll-contain">
            <GitBranchPicker
              :repo-root="repoRoot"
              :current-branch="branch"
              @switched="onBranchSwitched"
              @error="(message) => emit('error', message)"
            />
          </OverlayScrollArea>
        </template>
      </EcallDropdown>

      <!-- 最新提交：最新一条提交主题，点击打开提交历史 -->
      <button
        type="button"
        class="ecall-env-row"
        :title="latestCommitTitle"
        :disabled="!latestCommit"
        @click="emit('openCommits')"
      >
        <span class="ecall-env-icon ecall-env-icon-info"><History class="size-3.5" /></span>
        <span class="ecall-env-label">{{ t("chat.homePanel.latestCommitLabel") }}</span>
        <span class="ecall-env-value min-w-0 truncate">{{ latestCommitSubject || t("chat.homePanel.noCommits") }}</span>
      </button>
    </div>
  </CardShell>
</template>

<script setup lang="ts">
/**
 * 会话首页「环境」卡：把原来的 Git 变更卡与提交卡合并成一张，共三行。
 *
 * 行与行为：
 * - 变更：改动概况，点击打开变更集合
 * - 分支：当前分支，点击就地展开下拉切换
 * - 最新提交：最新一条提交主题，点击打开提交历史
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, FileDiff, GitBranch, History } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import EcallDropdown from "../../../shared/components/EcallDropdown.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import GitBranchPicker from "../../../file-reader/components/GitBranchPicker.vue";

const props = withDefaults(defineProps<{
  repoRoot?: string;
  branch?: string;
  changeCount?: number;
  /** 最近提交；只取第一条作为「最新提交」 */
  commits?: Array<{ hash: string; message: string }>;
}>(), {
  repoRoot: "",
  branch: "",
  changeCount: 0,
  commits: () => [],
});

const emit = defineEmits<{
  (e: "openChanges"): void;
  (e: "openCommits"): void;
  (e: "branchSwitched"): void;
  (e: "error", message: string): void;
}>();

const { t } = useI18n();

const branchPickerOpen = ref(false);

const changeCount = computed(() => Number(props.changeCount || 0));
const changeText = computed(() =>
  changeCount.value > 0 ? t("chat.homePanel.changeCount", { n: changeCount.value }) : t("chat.homePanel.cleanWorktree"),
);
const changeTitle = computed(() => t("chat.homePanel.openChangesTitle"));

const branchDisplay = computed(() => String(props.branch || "").trim() || t("gitPanel.detachedHead"));
const branchTitle = computed(() => t("chat.homePanel.switchBranchTitle", { name: branchDisplay.value }));

const latestCommit = computed(() => props.commits[0] || null);
const latestCommitSubject = computed(() => {
  const message = String(latestCommit.value?.message || "");
  return (message.split(/\r?\n/)[0] || "").trim();
});
const latestCommitTitle = computed(() => (latestCommit.value ? String(latestCommit.value.message || "") : ""));

function onBranchSwitched() {
  branchPickerOpen.value = false;
  // 会话内自己切的分支不算「在别处切过」：交给上层按仓库目录同步记录，避免下次发送时拦自己
  emit("branchSwitched");
}
</script>

<style scoped>
.ecall-env-rows {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

/* 行：图标 + 名称 + 右侧值，整行可点 */
.ecall-env-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  min-width: 0;
  padding: 0.25rem 0.375rem;
  border-radius: 0.5rem;
  font-size: 0.75rem;
  text-align: left;
  transition: background-color 120ms ease;
}

button.ecall-env-row:hover:not(:disabled) {
  background-color: var(--color-base-200);
}

.ecall-env-row:disabled {
  cursor: default;
}

.ecall-env-icon {
  display: grid;
  place-items: center;
  width: 1.25rem;
  height: 1.25rem;
  flex-shrink: 0;
  border-radius: 0.375rem;
}

.ecall-env-icon-warning {
  background-color: color-mix(in oklab, var(--color-warning) 12%, transparent);
  color: var(--color-warning);
}

.ecall-env-icon-primary {
  background-color: color-mix(in oklab, var(--color-primary) 12%, transparent);
  color: var(--color-primary);
}

.ecall-env-icon-info {
  background-color: color-mix(in oklab, var(--color-info) 12%, transparent);
  color: var(--color-info);
}

.ecall-env-label {
  flex-shrink: 0;
  color: color-mix(in oklab, var(--color-base-content) 80%, transparent);
}

.ecall-env-value {
  flex: 1;
  text-align: right;
  color: color-mix(in oklab, var(--color-base-content) 55%, transparent);
  font-variant-numeric: tabular-nums;
}
</style>
