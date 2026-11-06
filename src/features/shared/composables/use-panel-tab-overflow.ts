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

    // 每行能放几个：标签宽度固定后，就是容器宽整除「标签宽 + 间距」。
    // 换行不再需要测量——行数只是把这个数乘上去。
    const perRowCapacity = Math.max(1, Math.floor((width + gapVal) / (minTabW + gapVal)));

    // 没有标签或者容器宽度未就绪时，直接全量呈现，不出现更多按钮
    if (total <= 1 || width <= 0) {
      return {
        visibleTabs: allTabs,
        overflowTabs: [] as PanelTabStripItem[],
        hasOverflow: false,
        perRowCapacity: Math.max(1, total),
      };
    }

    // 全部标签在一行内放得下 → 全量呈现，不出现更多按钮
    if (total <= perRowCapacity) {
      return {
        visibleTabs: allTabs,
        overflowTabs: [] as PanelTabStripItem[],
        hasOverflow: false,
        perRowCapacity,
      };
    }

    // 放不下：留一格给更多按钮
    const usableWidth = Math.max(0, width - overflowBtnW - gapVal);
    const perRowWithButton = Math.max(1, Math.floor((usableWidth + gapVal) / (minTabW + gapVal)));
    const capacity = Math.max(1, Math.min(total - 1, perRowWithButton));

    const activeIdx = allTabs.findIndex((t) => t.key === currentActiveKey);

    // 如果激活项在前 capacity 个内，直接取前 capacity 个作为可见项
    if (activeIdx < capacity || activeIdx === -1) {
      return {
        visibleTabs: allTabs.slice(0, capacity),
        overflowTabs: allTabs.slice(capacity),
        hasOverflow: true,
        perRowCapacity,
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
      perRowCapacity,
    };
  });

  const visibleTabs = computed(() => resolved.value.visibleTabs);
  const overflowTabs = computed(() => resolved.value.overflowTabs);
  const hasOverflow = computed(() => resolved.value.hasOverflow);
  /** 每行能放几个标签。固定宽度后它就是「一行容量」，收起态的隐藏数量也用它。 */
  const perRowCapacity = computed(() => resolved.value.perRowCapacity);

  const activeInOverflow = computed(() => {
    const currentActiveKey = toValue(options.activeKey) || "";
    return overflowTabs.value.some((t: PanelTabStripItem) => t.key === currentActiveKey);
  });

  return {
    visibleTabs,
    overflowTabs,
    hasOverflow,
    activeInOverflow,
    perRowCapacity,
  };
}
