<template>
  <SettingsStickyLayout :no-scroll="noScroll" :content-class="contentClass" :header-class="resolvedHeaderClass">
    <template #header>
      <!-- w-full 必须有：这一行在 SettingsStickyLayout 里是 flex 子项，默认宽度只按内容撑，
           不写满就撑不到 1280，三列全挤在左边。 -->
      <div class="grid w-full grid-cols-[1fr_auto_1fr] items-center gap-3">
        <div class="flex min-w-0 items-center">
          <!-- 左列：返回按钮（iOS 风格，主题色，带上一级名字）；无标签页的页面额外用左侧插槽 -->
          <button
            v-if="backItem"
            type="button"
            class="btn btn-ghost btn-sm min-w-0 shrink-0 gap-0.5 pl-1 pr-1.5 text-primary hover:bg-primary/10"
            :title="backItem.title || backItem.label"
            @click="backItem.onClick?.()"
          >
            <AppIcon name="chevron-left" class="h-4 w-4 shrink-0" />
            <span class="min-w-0 max-w-40 truncate">{{ backItem.label }}</span>
          </button>
          <slot v-if="!tabs.length" name="left" />
        </div>
        <!-- 中列：标签页居中 -->
        <div class="flex min-w-0 items-center justify-center">
          <UnderlineTabs
            v-if="tabs.length"
            :tabs="tabs"
            :active-key="activeTab"
            @update:active-key="selectTab"
          />
        </div>
        <!-- 右列：操作按钮贴右 -->
        <div class="flex min-w-0 items-center justify-end gap-2">
          <slot name="actions" />
        </div>
      </div>
    </template>

    <div
      v-if="tabs.length"
      class="ecall-tab-stage"
      :class="{ 'h-full min-h-0 flex-1': noScroll }"
      :style="slideStyle"
    >
      <Transition name="ecall-tab-slide">
        <div
          :key="panelKey"
          class="ecall-tab-panel"
          :class="{ 'h-full min-h-0 flex flex-col flex-1': noScroll }"
        >
          <slot />
        </div>
      </Transition>
    </div>
    <div v-else-if="noScroll" class="h-full min-h-0 flex flex-col flex-1 overflow-hidden">
      <slot />
    </div>
    <slot v-else />
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import SettingsStickyLayout from "./SettingsStickyLayout.vue";
import type { SettingsBreadcrumbItem } from "./settings-breadcrumb-types";
import { useSettingsBreadcrumbContext } from "../composables/use-settings-breadcrumb";
import UnderlineTabs from "../../shared/components/UnderlineTabs.vue";
import type { UnderlineTabItem } from "../../shared/components/underline-tabs-types";
import AppIcon from "../../shared/components/AppIcon.vue";

const props = withDefaults(defineProps<{
  /** 面包屑项，末项为当前层，前面带 onClick 的项可点返回 */
  breadcrumb?: SettingsBreadcrumbItem[];
  /** 头部左侧的标签页；给了就渲染滑动下划线 tab，不给则用 left 插槽（如搜索框） */
  tabs?: UnderlineTabItem[];
  /** 当前标签页 key，配合 tabs 使用 */
  tab?: string;
  /**
   * 传给 SettingsStickyLayout 的头部内层类。
   *
   * **一般不用传**：有下划线 tab 时用 `pb-0` 让下划线贴合分界线，没有 tab（头部只剩面包屑）
   * 时保留 `p-4` 的底部留白。页面自己传值多半是多余的覆盖，会让同一套设置页头部高低不一。
   */
  headerClass?: string;
  /** 传给 SettingsStickyLayout 的内容内层类 */
  contentClass?: string;
  /** 是否禁用外部滚动（用于内部独立滚动的工作台视图） */
  noScroll?: boolean;
}>(), {
  breadcrumb: () => [],
  tabs: () => [],
  tab: "",
  headerClass: undefined,
  contentClass: undefined,
  noScroll: false,
});

const emit = defineEmits<{
  (e: "update:tab", key: string): void;
}>();

const activeTab = computed(() => props.tab);

/** 最近一个可点层级，返回按钮就回到它；停在顶层时没有可返回项，按钮不渲染。 */
const backItem = computed(() => {
  for (let index = props.breadcrumb.length - 1; index >= 0; index -= 1) {
    const item = props.breadcrumb[index];
    if (item?.onClick) return item;
  }
  return null;
});

// 面包屑由设置页外壳统一渲染（那一层永远在）：这里只把本级层级上报过去。
const breadcrumbContext = useSettingsBreadcrumbContext();
watch(
  () => props.breadcrumb,
  (items) => breadcrumbContext?.set(items ?? []),
  { immediate: true, deep: true },
);
onBeforeUnmount(() => breadcrumbContext?.reset());
const panelKey = computed(() => (props.tabs.length > 0 ? props.tab : ""));

/**
 * 头部内层类：不再按"有没有 tab"改内边距——那会让有 tab 的页 40px、没 tab 的页 56px。
 * 行高由 `SettingsStickyLayout` 固定，页面只在确有特例时显式传值。
 */
const resolvedHeaderClass = computed(() => props.headerClass ?? "");

// 方向按 tabs 声明顺序判定，往右切的内容从右侧进、旧内容从左侧出
const direction = ref<1 | -1>(1);

watch(
  () => props.tab,
  (next, prev) => {
    const from = props.tabs.findIndex((tab) => tab.key === prev);
    const to = props.tabs.findIndex((tab) => tab.key === next);
    if (from >= 0 && to >= 0 && from !== to) {
      direction.value = to > from ? 1 : -1;
    }
  },
);

const slideStyle = computed(() => ({
  "--ecall-tab-in-x": `${direction.value * 24}px`,
  "--ecall-tab-out-x": `${-direction.value * 24}px`,
}));

function selectTab(key: string) {
  emit("update:tab", key);
}
</script>
