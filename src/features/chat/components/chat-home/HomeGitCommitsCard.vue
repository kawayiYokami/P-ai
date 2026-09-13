<template>
  <CardShell
    variant="wide"
    tone="warning"
    :icon="History"
    :label="t('chat.homePanel.gitCommits')"
    interactive
    @select="emit('openCommits')"
  >
    <template #trailing>
      <span class="shrink-0 text-xs text-base-content/45">{{ summary }}</span>
    </template>
    <ul v-if="commits.length" class="menu menu-xs w-full gap-0.5 p-0">
      <li v-for="commit in visibleCommits" :key="commit.hash">
        <div class="min-w-0 font-normal" :title="commit.message">
          <span class="min-w-0 flex-1 truncate text-base-content/80">{{ subject(commit.message) }}</span>
        </div>
      </li>
    </ul>
    <div v-else class="flex flex-1 items-center justify-center text-xs text-base-content/40">
      {{ workspaceRootPath ? t("chat.homePanel.noCommits") : t("chat.homePanel.noWorkspace") }}
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { History } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const MAX_VISIBLE = 4;

const props = withDefaults(defineProps<{
  workspaceRootPath?: string;
  branch?: string;
  commits?: Array<{ hash: string; message: string }>;
}>(), {
  workspaceRootPath: "",
  branch: "",
  commits: () => [],
});

const emit = defineEmits<{
  (e: "openCommits"): void;
}>();

const { t } = useI18n();

const visibleCommits = computed(() => props.commits.slice(0, MAX_VISIBLE));
const summary = computed(() => (props.workspaceRootPath ? String(props.branch || "") : ""));

/** 提交信息可能带正文，卡片只显示主题行 */
function subject(message: string): string {
  const firstLine = String(message || "").split(/\r?\n/)[0] || "";
  return firstLine.trim();
}
</script>
