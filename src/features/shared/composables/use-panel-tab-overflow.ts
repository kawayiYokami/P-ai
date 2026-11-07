import { computed, toValue, type MaybeRefOrGetter } from "vue";
import type { PanelTabStripItem } from "../components/panel-tab-strip.types";

export type UsePanelTabOverflowOptions = {
  tabs: MaybeRefOrGetter<PanelTabStripItem[]>;
  activeKey: MaybeRefOrGetter<string>;
  containerWidth: MaybeRefOrGetter<number>;
  minTabWidth?: MaybeRefOrGetter<number>;
  maxTabWidth?: MaybeRefOrGetter<number>;
  overflowButtonWidth?: MaybeRefOrGetter<number>;
  gap?: MaybeRefOrGetter<number>;
};

export function usePanelTabOverflow(options: UsePanelTabOverflowOptions) {
  const resolved = computed(() => {
    const allTabs = toValue(options.tabs) || [];
    const total = allTabs.length;
    const width = toValue(options.containerWidth) || 0;
    const minTabW = toValue(options.minTabWidth) ?? 96;
    const overflowBtnW = toValue(options.overflowButtonWidth) ?? 32;
    const gapVal = toValue(options.gap) ?? 4;
    const currentActiveKey = toValue(options.activeKey) || "";

    // 没有标签或者容器宽度未就绪时，直接全量呈现，不出现更多按钮
    if (total <= 1 || width <= 0) {
      return {
        visibleTabs: allTabs,
        overflowTabs: [] as PanelTabStripItem[],
        hasOverflow: false,
      };
    }

    // 判断不带更多按钮时能否全部容纳
    const requiredWidthNoOverflow = total * minTabW + (total - 1) * gapVal;
    if (width >= requiredWidthNoOverflow) {
      return {
        visibleTabs: allTabs,
        overflowTabs: [] as PanelTabStripItem[],
        hasOverflow: false,
      };
    }

    // 无法全部容纳，必须留出更多按钮空间
    const usableWidth = Math.max(0, width - overflowBtnW - gapVal);
    const capacity = Math.max(1, Math.min(total - 1, Math.floor((usableWidth + gapVal) / (minTabW + gapVal))));

    const activeIdx = allTabs.findIndex((t) => t.key === currentActiveKey);

    // 如果激活项在前 capacity 个内，直接取前 capacity 个作为可见项
    if (activeIdx < capacity || activeIdx === -1) {
      return {
        visibleTabs: allTabs.slice(0, capacity),
        overflowTabs: allTabs.slice(capacity),
        hasOverflow: true,
      };
    }

    // 激活项排在后面，优先将其保留在可见区末尾，避免当前激活页在标签栏上消失
    const prefixCount = Math.max(0, capacity - 1);
    const prefixTabs = allTabs.slice(0, prefixCount);
    const activeTab = allTabs[activeIdx];
    const visibleTabs = [...prefixTabs, activeTab];

    const visibleKeySet = new Set(visibleTabs.map((t) => t.key));
    const overflowTabs = allTabs.filter((t) => !visibleKeySet.has(t.key));

    return {
      visibleTabs,
      overflowTabs,
      hasOverflow: true,
    };
  });

  const visibleTabs = computed(() => resolved.value.visibleTabs);
  const overflowTabs = computed(() => resolved.value.overflowTabs);
  const hasOverflow = computed(() => resolved.value.hasOverflow);

  const activeInOverflow = computed(() => {
    const currentActiveKey = toValue(options.activeKey) || "";
    return overflowTabs.value.some((t: PanelTabStripItem) => t.key === currentActiveKey);
  });

  return {
    visibleTabs,
    overflowTabs,
    hasOverflow,
    activeInOverflow,
  };
}
