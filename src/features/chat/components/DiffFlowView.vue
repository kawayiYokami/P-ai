<template>
  <div ref="rootEl" class="diff-flow-root flex h-full min-h-0">
    <OverlayScrollArea
      class="min-h-0 min-w-0 flex-1"
      variant="code-dark"
      scroller-class="diff-flow-scroller h-full"
    >
      <div class="diff-flow-list">
        <section
          v-for="item in items"
          :key="item.key"
          class="diff-flow-item text-xs"
        >
          <!-- 不套卡片：整块本来就在同一个容器里，这里只留一行文件标识；点这行折叠/展开该段 -->
          <div
            role="button"
            tabindex="0"
            :aria-expanded="!isCollapsed(item.key)"
            :title="`${itemTitle(item)}（点击${isCollapsed(item.key) ? '展开' : '收起'}，右键折叠/展开全部）`"
            class="flex cursor-pointer select-none items-center gap-2 border-b border-base-content/5 bg-base-200 px-3 py-2 font-mono text-xs transition-colors hover:bg-base-300"
            @click="toggleCollapsed(item.key)"
            @contextmenu.prevent="toggleAll"
            @keydown.enter.prevent="toggleCollapsed(item.key)"
            @keydown.space.prevent="toggleCollapsed(item.key)"
          >
            <AppIcon
              name="chevron-right"
              class="size-3 shrink-0 text-base-content/45 transition-transform duration-200 ease-out"
              :class="{ 'rotate-90': !isCollapsed(item.key) }"
            />
            <img
              :src="resolveFileTreeIcon(item.name, false)"
              class="size-3.5 shrink-0"
              alt=""
              aria-hidden="true"
            />
            <span class="flex min-w-0 flex-1 items-baseline gap-1.5 whitespace-nowrap" :title="item.path">
              <span class="shrink-0 text-base-content">{{ item.name }}</span>
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[0.625rem] text-base-content/45">{{ item.dir }}</span>
            </span>
            <span class="shrink-0 text-success">+{{ item.added }}</span>
            <span class="shrink-0 text-error">-{{ item.removed }}</span>
            <!-- 打开文件：跳到文件阅读面板并定位到这个文件 -->
            <button
              type="button"
              class="shrink-0 rounded-sm p-0.5 text-base-content/40 transition-colors hover:text-base-content"
              title="打开文件"
              @click.stop="emit('openFile', item.sourcePath || item.path)"
            >
              <AppIcon name="external-link" class="size-3" />
            </button>
          </div>
          <!-- 折叠走 grid 行轨道过渡，跟 CollapsibleGroup 同一口径，连点不会吞状态 -->
          <div class="diff-flow-shell" :class="{ 'is-collapsed': isCollapsed(item.key) }">
            <div class="diff-flow-inner">
              <TerminalApprovalPatchSample
                :lines="item.lines"
                :diff-only="true"
                hide-header
                embedded
              />
            </div>
          </div>
        </section>
      </div>
      <slot />
    </OverlayScrollArea>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import TerminalApprovalPatchSample from "../../shell/components/TerminalApprovalPatchSample.vue";
import { resolveFileTreeIcon } from "../../file-reader/file-tree-icons";
import type { DiffFlowItem } from "./diff-flow-types";

const props = defineProps<{
  items: DiffFlowItem[];
}>();

const emit = defineEmits<{
  (e: "openFile", path: string): void;
}>();

const rootEl = ref<HTMLElement | null>(null);
/** 折叠状态按项 key 记，换批次时整体清空，免得上一个批次收起的项把新批次串住 */
const collapsedKeys = ref<Record<string, boolean>>({});

function isCollapsed(key: string) {
  return !!collapsedKeys.value[key];
}

function toggleCollapsed(key: string) {
  collapsedKeys.value = { ...collapsedKeys.value, [key]: !collapsedKeys.value[key] };
}

/** 右键任一折叠标题：还有展开的就全部折起，全折了再右键就全部展开 */
function toggleAll() {
  if (allCollapsed.value) {
    collapsedKeys.value = {};
    return;
  }
  collapsedKeys.value = Object.fromEntries(props.items.map((item) => [item.key, true]));
}

/** 供宿主在页码旁放一个整体折叠开关 */
const allCollapsed = computed(
  () => props.items.length > 0 && props.items.every((item) => collapsedKeys.value[item.key]),
);

defineExpose({ toggleAll, allCollapsed });

/** 拆成「文件名 + 目录」：文件名在前必须完整，目录在后可以截断 */
const items = computed(() =>
  props.items.map((item) => {
    const path = String(item.path || "").replace(/\\/g, "/");
    const cut = path.lastIndexOf("/");
    return {
      ...item,
      dir: cut >= 0 ? path.slice(0, cut) : "",
      name: cut >= 0 ? path.slice(cut + 1) : path,
    };
  }),
);

function itemTitle(item: DiffFlowItem & { name: string }) {
  return `${item.name}  +${item.added} -${item.removed}`;
}

watch(
  () => props.items,
  () => {
    // 换批次后列表换了内容：折叠状态作废
    collapsedKeys.value = {};
  },
);
</script>

<style scoped>
/* 字号跟随外层 text-xs，不在这里写死：补丁组件默认是 0.75rem，外层降档时它得跟着走 */
.diff-flow-item :deep(.approval-patch-sample) {
  font-size: inherit;
}

/* 折叠：走 grid 行轨道过渡，与 CollapsibleGroup 同一口径，连点只改目标值不吞状态 */
.diff-flow-shell {
  display: grid;
  grid-template-rows: 1fr;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  transition: grid-template-rows 180ms cubic-bezier(0.22, 1, 0.36, 1);
}

.diff-flow-shell.is-collapsed {
  grid-template-rows: 0fr;
}

.diff-flow-inner {
  min-height: 0;
  min-width: 0;
  overflow: clip;
}

@media (prefers-reduced-motion: reduce) {
  .diff-flow-shell {
    transition: none;
  }
}
</style>
