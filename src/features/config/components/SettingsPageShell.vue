<template>
  <SettingsStickyLayout :content-class="contentClass" :header-class="headerClass">
    <template #header>
      <slot name="breadcrumb">
        <SettingsBreadcrumb :items="breadcrumb" />
      </slot>

      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex min-w-0 flex-1 items-center">
          <UnderlineTabs
            v-if="tabs.length"
            :tabs="tabs"
            :active-key="activeTab"
            @update:active-key="selectTab"
          />
          <slot v-else name="left" />
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <slot name="actions" />
        </div>
      </div>
    </template>

    <div class="ecall-tab-stage" :style="slideStyle">
      <Transition name="ecall-tab-slide">
        <div :key="panelKey" class="ecall-tab-panel">
          <slot />
        </div>
      </Transition>
    </div>
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import SettingsStickyLayout from "./SettingsStickyLayout.vue";
import SettingsBreadcrumb, { type SettingsBreadcrumbItem } from "./SettingsBreadcrumb.vue";
import UnderlineTabs, { type UnderlineTabItem } from "./UnderlineTabs.vue";

const props = withDefaults(defineProps<{
  /** 面包屑项，末项为当前层，前面带 onClick 的项可点返回 */
  breadcrumb?: SettingsBreadcrumbItem[];
  /** 头部左侧的标签页；给了就渲染滑动下划线 tab，不给则用 left 插槽（如搜索框） */
  tabs?: UnderlineTabItem[];
  /** 当前标签页 key，配合 tabs 使用 */
  tab?: string;
  /** 传给 SettingsStickyLayout 的头部内层类 */
  headerClass?: string;
  /** 传给 SettingsStickyLayout 的内容内层类 */
  contentClass?: string;
}>(), {
  breadcrumb: () => [],
  tabs: () => [],
  tab: "",
  headerClass: "pb-0",
  contentClass: undefined,
});

const emit = defineEmits<{
  (e: "update:tab", key: string): void;
}>();

const activeTab = computed(() => props.tab);
const panelKey = computed(() => (props.tabs.length > 0 ? props.tab : ""));

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
