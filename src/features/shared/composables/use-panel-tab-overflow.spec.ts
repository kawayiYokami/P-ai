import { describe, expect, it } from "vitest";
import { ref } from "vue";
import { usePanelTabOverflow } from "./use-panel-tab-overflow";
import type { PanelTabStripItem } from "../components/panel-tab-strip.types";

function mockTab(key: string, label = `Tab ${key}`): PanelTabStripItem {
  return { key, label };
}

describe("usePanelTabOverflow", () => {
  it("shows all tabs when width is sufficient", () => {
    const tabs = ref([mockTab("1"), mockTab("2"), mockTab("3")]);
    const activeKey = ref("1");
    const containerWidth = ref(500);

    const { visibleTabs, overflowTabs, hasOverflow } = usePanelTabOverflow({
      tabs,
      activeKey,
      containerWidth,
      minTabWidth: 100,
      overflowButtonWidth: 36,
      gap: 4,
    });

    expect(hasOverflow.value).toBe(false);
    expect(visibleTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["1", "2", "3"]);
    expect(overflowTabs.value).toEqual([]);
  });

  it("splits tabs into visible and overflow when width is constrained", () => {
    const tabs = ref([mockTab("1"), mockTab("2"), mockTab("3"), mockTab("4")]);
    const activeKey = ref("1");
    // 250px 可用: 250 - 36 - 4 = 210px -> 210 / (100 + 4) = 2 个
    const containerWidth = ref(250);

    const { visibleTabs, overflowTabs, hasOverflow } = usePanelTabOverflow({
      tabs,
      activeKey,
      containerWidth,
      minTabWidth: 100,
      overflowButtonWidth: 36,
      gap: 4,
    });

    expect(hasOverflow.value).toBe(true);
    expect(visibleTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["1", "2"]);
    expect(overflowTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["3", "4"]);
  });

  it("keeps active tab visible even if it is positioned later in the list", () => {
    const tabs = ref([mockTab("1"), mockTab("2"), mockTab("3"), mockTab("4")]);
    const activeKey = ref("4"); // 激活第 4 项
    const containerWidth = ref(250); // 能容纳 2 个

    const { visibleTabs, overflowTabs, hasOverflow } = usePanelTabOverflow({
      tabs,
      activeKey,
      containerWidth,
      minTabWidth: 100,
      overflowButtonWidth: 36,
      gap: 4,
    });

    expect(hasOverflow.value).toBe(true);
    // 可见区保证保留 activeTab "4"
    expect(visibleTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["1", "4"]);
    // 未在可见区的剩余项进 overflow
    expect(overflowTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["2", "3"]);
  });

  it("handles narrow container by guaranteeing at least 1 tab visible", () => {
    const tabs = ref([mockTab("1"), mockTab("2"), mockTab("3")]);
    const activeKey = ref("2");
    const containerWidth = ref(80); // 极窄视口

    const { visibleTabs, overflowTabs, hasOverflow } = usePanelTabOverflow({
      tabs,
      activeKey,
      containerWidth,
      minTabWidth: 100,
      overflowButtonWidth: 36,
      gap: 4,
    });

    expect(hasOverflow.value).toBe(true);
    expect(visibleTabs.value.length).toBe(1);
    expect(visibleTabs.value[0].key).toBe("2");
    expect(overflowTabs.value.map((t: PanelTabStripItem) => t.key)).toEqual(["1", "3"]);
  });
});
