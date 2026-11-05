<template>
  <section :style="leadStyle" class="min-w-0">
    <div
      role="button"
      tabindex="0"
      :draggable="draggable"
      class="group/section relative sticky top-0 z-20 mx-1 flex min-h-9 select-none items-center gap-2 rounded-lg px-2.5 py-1 text-left text-sm text-base-content/80 transition-colors hover:text-base-content"
      :class="{ 'ecall-frost-solid': headOpaque }"
      :title="title"
      @click="toggle"
      @contextmenu.stop.prevent="collapseAll"
      @keydown.enter.prevent="toggle"
      @keydown.space.prevent="toggle"
      @dragstart="onDragStart"
      @dragover="onDragOver"
      @drop="onDrop"
      @dragend="onDragEnd"
    >
      <div
        v-if="dropIndicator === 'before'"
        class="pointer-events-none absolute left-2 right-2 top-0 h-[3px] -translate-y-1/2 rounded-full bg-neutral shadow-[0_0_0_1px_color-mix(in_oklab,var(--color-neutral)_28%,transparent)]"
        aria-hidden="true"
      ></div>
      <div
        v-if="dropIndicator === 'after'"
        class="pointer-events-none absolute left-2 right-2 bottom-0 translate-y-1/2 rounded-full bg-neutral h-[3px] shadow-[0_0_0_1px_color-mix(in_oklab,var(--color-neutral)_28%,transparent)]"
        aria-hidden="true"
      ></div>
      <span v-if="avatarUrl" class="avatar shrink-0">
        <span class="flex h-8 w-8 items-center justify-center overflow-hidden rounded-full bg-base-100">
          <img :src="avatarUrl" :alt="title" class="h-full w-full object-cover" />
        </span>
      </span>
      <ChevronRight
        v-else-if="icon === 'chevron'"
        class="h-4 w-4 shrink-0 transition-transform duration-200 ease-out"
        :class="collapsed ? '' : 'rotate-90'"
      />
      <component
        v-else
        :is="collapsed ? Folder : FolderOpen"
        class="h-4 w-4 shrink-0"
      />
      <span class="min-w-0 truncate">{{ title }}</span>
      <span v-if="count !== undefined" class="shrink-0 tabular-nums">{{ count }}</span>
      <ChevronRight
        v-if="avatarUrl"
        class="ml-auto h-4 w-4 shrink-0 transition-transform duration-200 ease-out"
        :class="collapsed ? '' : 'rotate-90'"
      />
      <slot name="actions" />
    </div>
    <!--
      展开 / 收起走 grid 行轨道过渡（与 daisyUI collapse 同款），由 CSS 负责动画：
      快速连点只改变目标值，浏览器从当前计算值接着过渡，不会吞点击、也不会留下中间态。
    -->
    <div
      ref="shellRef"
      class="collapsible-group-shell"
      :class="{ 'is-collapsed': collapsed }"
      @transitionend="onShellTransitionEnd"
    >
      <div class="collapsible-group-inner">
        <slot />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { ChevronRight, Folder, FolderOpen } from "@lucide/vue";

const props = withDefaults(defineProps<{
  title: string;
  count?: number;
  /** 是否收起；true 表示收起，内容区高度归零 */
  modelValue: boolean;
  /** 折叠指示图标：chevron=箭头（默认），folder=文件夹开合 */
  icon?: "chevron" | "folder";
  /** 分组头像地址；传了就优先显示头像，替代图标 */
  avatarUrl?: string | null;
  draggable?: boolean;
  dropIndicator?: "before" | "after" | null;
  /** 折叠头是否用不透明底色盖住下方滚过内容；只给坐在透色底上的侧栏用 */
  headOpaque?: boolean;
}>(), {
  icon: "chevron",
});

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  "collapse-all": [];
  "after-enter": [];
  "after-leave": [];
  "dragstart": [event: DragEvent];
  "dragover": [event: DragEvent];
  "drop": [event: DragEvent];
  "dragend": [event: DragEvent];
}>();

const collapsed = computed(() => !!props.modelValue);

const shellRef = ref<HTMLElement | null>(null);
let settleTimer: number | undefined;

/** 把前导图标/头像的宽度透传给插槽内的会话行，保证文字左缘对齐（头像 2rem、图标 1rem） */
const leadStyle = computed(() => ({ "--ecall-section-lead": props.avatarUrl ? "2rem" : "1rem" }));

function toggle() {
  // 无动画守卫：动画期间的点击同样生效，状态与点击次数始终一致
  emit("update:modelValue", !props.modelValue);
}

function clearSettleTimer() {
  if (settleTimer === undefined) return;
  window.clearTimeout(settleTimer);
  settleTimer = undefined;
}

/** 过渡收尾：通知外部重新测量布局 */
function notifySettled() {
  clearSettleTimer();
  if (props.modelValue) {
    emit("after-leave");
  } else {
    emit("after-enter");
  }
}

function onShellTransitionEnd(event: TransitionEvent) {
  if (event.target !== shellRef.value || event.propertyName !== "grid-template-rows") return;
  notifySettled();
}

watch(() => props.modelValue, () => {
  // 兜底：内容高度为 0、或系统关闭动效时 transitionend 不触发
  clearSettleTimer();
  settleTimer = window.setTimeout(notifySettled, 260);
});

onBeforeUnmount(clearSettleTimer);

function collapseAll(event: MouseEvent) {
  const target = event.target;
  if (target instanceof HTMLElement && target.closest("button,a,input,textarea,select")) return;
  emit("collapse-all");
}

function onDragStart(event: DragEvent) {
  if (!props.draggable) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  emit("dragstart", event);
}

function onDragOver(event: DragEvent) {
  if (!props.draggable) return;
  emit("dragover", event);
}

function onDrop(event: DragEvent) {
  if (!props.draggable) return;
  emit("drop", event);
}

function onDragEnd(event: DragEvent) {
  if (!props.draggable) return;
  emit("dragend", event);
}
</script>

<style scoped>
.collapsible-group-shell {
  display: grid;
  grid-template-rows: 1fr;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  transition: grid-template-rows 180ms cubic-bezier(0.22, 1, 0.36, 1);
}

.collapsible-group-shell.is-collapsed {
  grid-template-rows: 0fr;
}

.collapsible-group-inner {
  min-height: 0;
  min-width: 0;
  overflow: clip;
  visibility: visible;
  transition: visibility 180ms;
}

.collapsible-group-shell.is-collapsed .collapsible-group-inner {
  visibility: hidden;
}

@media (prefers-reduced-motion: reduce) {
  .collapsible-group-shell {
    transition: none;
  }
}
</style>
