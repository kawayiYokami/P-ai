<template>
  <CardShell
    variant="wide"
    tone="primary"
    :icon="ClipboardList"
    :label="t('chat.homePanel.planLabel')"
    interactive
    @select="emit('open', plan.path)"
  >
    <template #trailing>
      <span
        v-if="taskProgress"
        class="shrink-0 text-xs font-mono font-medium px-1.5 py-0.5 rounded-full"
        :class="taskProgress.isDone ? 'bg-success/15 text-success' : 'bg-primary/15 text-primary'"
      >
        {{ taskProgress.done }}/{{ taskProgress.total }}
      </span>
    </template>
    <div class="flex min-w-0 flex-col gap-1.5">
      <!-- 计划标题行 -->
      <div class="flex items-center justify-between gap-1 min-w-0">
        <span class="truncate text-xs font-semibold text-base-content/90" :title="outline.title">
          {{ outline.title }}
        </span>
        <ArrowUpRight class="size-3 shrink-0 text-base-content/35" aria-hidden="true" />
      </div>

      <!-- 加载中状态 -->
      <div v-if="loading && !outline.items.length" class="flex items-center gap-1.5 py-1 text-xs text-base-content/40">
        <span class="loading loading-spinner loading-xs"></span>
        <span>{{ t("chat.homePanel.planLoading") }}</span>
      </div>

      <!-- 读取失败 -->
      <div v-else-if="errorText" class="truncate text-2xs text-error/80" :title="errorText">
        {{ t("chat.messageItem.readPlanFailed") }}
      </div>

      <!-- 条目列表 -->
      <ul v-else-if="visibleItems.length" class="menu menu-xs w-full gap-0.5 p-0">
        <li v-for="(item, idx) in visibleItems" :key="`${idx}-${item.text}`">
          <div class="min-w-0 gap-1.5 py-0.5 px-1 font-normal" :title="item.text">
            <!-- checklist item -->
            <template v-if="item.kind === 'checkbox'">
              <Check
                v-if="item.status === 'completed'"
                class="size-3 shrink-0 text-success"
                aria-hidden="true"
              />
              <span
                v-else
                class="ecall-plan-bullet-empty shrink-0 border border-base-content/30 rounded-xs"
                aria-hidden="true"
              ></span>
            </template>
            <!-- 编号项 -->
            <span
              v-else-if="item.kind === 'numbered'"
              class="ecall-home-num shrink-0 text-2xs font-mono text-base-content/50"
            >
              {{ idx + 1 }}.
            </span>
            <!-- 标题或圆点项 -->
            <span
              v-else
              class="ecall-plan-dot shrink-0"
              :class="item.kind === 'heading' ? 'bg-primary/70' : 'bg-base-content/30'"
            ></span>

            <!-- 文字 -->
            <span
              class="min-w-0 flex-1 truncate text-xs"
              :class="item.status === 'completed' ? 'line-through text-base-content/40' : 'text-base-content/80'"
            >
              {{ item.text }}
            </span>
          </div>
        </li>
      </ul>

      <!-- 无条目但已加载完：展示路径 -->
      <div
        v-else-if="!loading && !visibleItems.length"
        class="truncate text-2xs text-base-content/40 font-mono"
        :title="plan.path"
      >
        {{ displayPath }}
      </div>
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUpRight, Check, ClipboardList } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import { invokeTauri } from "../../../../services/tauri-api";
import { parsePlanMarkdown, type ParsedPlanItem } from "../../utils/plan-card-parser";

const MAX_VISIBLE_ITEMS = 4;

export interface LatestPlanSummary {
  path: string;
  markdownContent?: string;
}

const props = withDefaults(defineProps<{
  plan: LatestPlanSummary;
  conversationId?: string;
  /** 可选：直接传入 markdown（例如在测试/演示画廊中） */
  markdownContent?: string;
}>(), {
  conversationId: "",
  markdownContent: "",
});

const emit = defineEmits<{
  (e: "open", path: string): void;
}>();

const { t } = useI18n();

const planMarkdown = ref("");
const loading = ref(false);
const errorText = ref("");

const effectiveMarkdown = computed(() => props.plan?.markdownContent || props.markdownContent || planMarkdown.value);

const outline = computed(() => parsePlanMarkdown(effectiveMarkdown.value, props.plan.path));

const visibleItems = computed<ParsedPlanItem[]>(() => outline.value.items.slice(0, MAX_VISIBLE_ITEMS));

const taskProgress = computed(() => {
  const checkboxes = outline.value.items.filter((item) => item.kind === "checkbox");
  if (!checkboxes.length) return null;
  const done = checkboxes.filter((item) => item.status === "completed").length;
  return {
    done,
    total: checkboxes.length,
    isDone: done === checkboxes.length,
  };
});

const displayPath = computed(() => {
  const raw = String(props.plan.path || "").replace(/\\/g, "/").trim();
  const parts = raw.split("/").filter(Boolean);
  return parts.slice(-2).join("/");
});

// 以「会话 + 计划路径」作为唯一读取依据：同一份计划只读一次，
// 消息内容或 plan 对象引用如何变化都不再重复读取
const loadKey = computed(() => {
  const convId = String(props.conversationId || "").trim();
  const path = String(props.plan?.path || "").trim();
  return `${convId}\u0000${path}`;
});

watch(
  loadKey,
  async (key, _prev, onCleanup) => {
    // 直接传入 markdown（演示画廊 / 测试）时无需读文件
    if (props.plan?.markdownContent || props.markdownContent) {
      planMarkdown.value = "";
      errorText.value = "";
      loading.value = false;
      return;
    }

    const [convId, path] = key.split("\u0000");
    if (!convId || !path) {
      planMarkdown.value = "";
      errorText.value = "";
      loading.value = false;
      return;
    }

    let cancelled = false;
    onCleanup(() => {
      cancelled = true;
    });

    // 切换目标计划时先清空上一份内容，避免旧计划串台
    planMarkdown.value = "";
    errorText.value = "";
    loading.value = true;

    try {
      const content = await invokeTauri<string>("conversation.plan.readFile", {
        conversationId: convId,
        path,
      });
      if (cancelled) return;
      planMarkdown.value = String(content || "");
    } catch (error) {
      if (cancelled) return;
      errorText.value = error instanceof Error ? error.message : String(error || "");
    } finally {
      if (!cancelled) {
        loading.value = false;
      }
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.ecall-home-num {
  font-variant-numeric: tabular-nums;
}

.ecall-plan-dot {
  width: 0.25rem;
  height: 0.25rem;
  border-radius: 9999px;
}

.ecall-plan-bullet-empty {
  width: 0.65rem;
  height: 0.65rem;
}
</style>
