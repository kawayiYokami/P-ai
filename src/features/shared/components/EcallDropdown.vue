<template>
  <div ref="rootRef" class="relative" :class="rootClass">
    <slot name="trigger" :open="isOpen" :toggle="toggle" :close="close" :openDropdown="openDropdown" />
    <!-- 非 Teleport：absolute 跟随 trigger，完全在文档流内 -->
    <Transition :name="transitionName">
      <div
        v-if="isOpen && !teleport"
        ref="inlinePanelRef"
        class="absolute z-30 overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
        :class="[panelClass, direction === 'up' ? 'bottom-full mb-2' : 'top-full mt-2']"
        :style="inlinePanelStyle"
      >
        <slot :close="close" :open="isOpen" />
      </div>
    </Transition>

    <!-- Teleport：fixed 脱离 overflow 裁剪，按视口定位；对话框已改为 :open 非 top layer，teleport 到 body 即可在最前；不使用 disabled 切换，避免关闭瞬间固定定位面板被移回原位导致瞬移到右边 -->
    <Teleport v-if="teleport && isOpen" :to="teleportTo">
      <Transition :name="transitionName" appear>
        <div
          v-if="isOpen"
          ref="teleportedPanelRef"
          class="fixed z-[1300] overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
          :class="panelClass"
          :style="teleportedStyle"
          :data-theme="teleportTheme"
          @click.stop
        >
          <slot :close="close" :open="isOpen" />
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = withDefaults(defineProps<{
  modelValue: boolean;
  disabled?: boolean;
  teleport?: boolean;
  teleportTo?: string;
  matchTriggerWidth?: boolean;
  panelClass?: string;
  rootClass?: string;
  maxHeight?: number;
  placement?: "auto" | "top" | "bottom";
}>(), {
  disabled: false,
  teleport: false,
  teleportTo: "body",
  matchTriggerWidth: true,
  panelClass: "",
  rootClass: "",
  maxHeight: undefined,
  placement: "auto",
});

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "open"): void;
  (e: "close"): void;
}>();

const rootRef = ref<HTMLElement | null>(null);
const inlinePanelRef = ref<HTMLElement | null>(null);
const teleportedPanelRef = ref<HTMLElement | null>(null);

const direction = ref<"up" | "down">("down");
const teleportedStyle = ref<Record<string, string>>({});
const inlinePanelStyle = ref<Record<string, string>>({});

const isOpen = computed(() => Boolean(props.modelValue));
const transitionName = computed(() => direction.value === "up" ? "ecall-dropdown-up" : "ecall-dropdown");

const teleportTheme = computed(() => {
  if (typeof document === "undefined") return "light";
  return document.documentElement.getAttribute("data-theme") || "light";
});

function openDropdown() {
  if (props.disabled) return;
  if (isOpen.value) return;
  emit("update:modelValue", true);
  emit("open");
}

function close() {
  if (!isOpen.value) return;
  emit("update:modelValue", false);
  emit("close");
}

function toggle() {
  if (props.disabled) return;
  if (isOpen.value) close();
  else openDropdown();
}

const VIEWPORT_MARGIN = 12;
const GAP = 8;

async function refreshPosition() {
  if (!isOpen.value) return;
  const root = rootRef.value;
  if (!root) return;
  await nextTick();
  const rect = root.getBoundingClientRect();
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  const spaceAbove = Math.max(0, rect.top - VIEWPORT_MARGIN - GAP);
  const spaceBelow = Math.max(0, viewportHeight - rect.bottom - VIEWPORT_MARGIN - GAP);

  let nextDirection: "up" | "down" = "down";
  if (props.placement === "top") nextDirection = "up";
  else if (props.placement === "bottom") nextDirection = "down";
  else {
    // auto：下方放不下且上方更大时向上
    const needHeight = props.maxHeight ?? 240;
    if (spaceBelow < needHeight && spaceAbove > spaceBelow) nextDirection = "up";
  }
  direction.value = nextDirection;

  const availableHeight = nextDirection === "up" ? spaceAbove : spaceBelow;
  const cappedMaxHeight = props.maxHeight ?? Math.min(320, Math.max(120, availableHeight));
  const finalMaxHeight = Math.max(96, Math.min(cappedMaxHeight, availableHeight || cappedMaxHeight));

  if (props.teleport) {
    const triggerWidth = props.matchTriggerWidth ? Math.round(rect.width) : undefined;
    // 未锁定触发宽度时，面板宽度由 panel-class 决定（可自适应内容）；
    // 此时按面板实测宽度收敛左右边界，避免撑开后溢出视口
    const panelWidth =
      triggerWidth ?? Math.round(teleportedPanelRef.value?.getBoundingClientRect().width ?? 0);
    let left = Math.round(rect.left);
    if (panelWidth > 0) {
      if (left + panelWidth > viewportWidth - VIEWPORT_MARGIN) {
        left = Math.max(VIEWPORT_MARGIN, viewportWidth - panelWidth - VIEWPORT_MARGIN);
      }
      if (left < VIEWPORT_MARGIN) left = VIEWPORT_MARGIN;
    }
    const style: Record<string, string> = {
      maxHeight: `${Math.round(finalMaxHeight)}px`,
    };
    if (triggerWidth !== undefined) style.width = `${triggerWidth}px`;
    style.left = `${Math.round(left)}px`;
    style.right = "auto";
    if (nextDirection === "up") {
      const bottom = Math.round(viewportHeight - rect.top + GAP);
      style.bottom = `${bottom}px`;
      style.top = "auto";
    } else {
      const top = Math.round(rect.bottom + GAP);
      style.top = `${top}px`;
      style.bottom = "auto";
    }
    teleportedStyle.value = style;
  } else {
    inlinePanelStyle.value = {
      maxHeight: `${Math.round(finalMaxHeight)}px`,
    };
  }
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!isOpen.value) return;
  const target = event.target as Node | null;
  if (!target) return;
  if (rootRef.value?.contains(target)) return;
  if (props.teleport && teleportedPanelRef.value?.contains(target)) return;
  if (!props.teleport && inlinePanelRef.value?.contains(target)) return;
  // 点击到 Teleport 面板内部已在上面 return，不会关闭；此处统一关闭
  close();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && isOpen.value) {
    close();
  }
}

watch(() => isOpen.value, async (open) => {
  if (open) {
    await nextTick();
    await refreshPosition();
  }
});

watch(() => [props.teleport, props.teleportTo, props.maxHeight, props.placement] as const, () => {
  if (isOpen.value) void refreshPosition();
});

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  document.addEventListener("keydown", handleKeydown);
  window.addEventListener("resize", refreshPosition, { passive: true });
  window.addEventListener("scroll", refreshPosition, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  document.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("resize", refreshPosition);
  window.removeEventListener("scroll", refreshPosition, true);
});

defineExpose({ refreshPosition, direction, isOpen });
</script>
