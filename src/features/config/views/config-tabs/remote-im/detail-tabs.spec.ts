// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { computed, ref } from "vue";
import { buildDetailTabs, useDetailTabs } from "./detail-tabs";

const labels = { channelLabel: "渠道设置", contactsLabel: "联系人设置" };

describe("buildDetailTabs 的出现条件", () => {
  it("详情态且有渠道：两个 tab，顺序为渠道设置 → 联系人设置", () => {
    const tabs = buildDetailTabs({ inDetailMode: true, hasChannel: true, channelDirty: false, ...labels });
    expect(tabs.map((tab) => tab.key)).toEqual(["channel", "contacts"]);
    expect(tabs.map((tab) => tab.label)).toEqual(["渠道设置", "联系人设置"]);
  });

  it("列表态：不渲染 tab（否则页头会挤掉 #left 的渠道搜索框）", () => {
    expect(buildDetailTabs({ inDetailMode: false, hasChannel: true, channelDirty: false, ...labels })).toEqual([]);
  });

  it("详情态但渠道已被删掉：一个 tab 都不出（P2-1：不能留两个点了没反应的 tab）", () => {
    expect(buildDetailTabs({ inDetailMode: true, hasChannel: false, channelDirty: false, ...labels })).toEqual([]);
  });

  it("渠道有未保存改动时，渠道设置标签带小圆点", () => {
    const tabs = buildDetailTabs({ inDetailMode: true, hasChannel: true, channelDirty: true, ...labels });
    expect(tabs[0].badge).toBe("●");
    expect(tabs[1].badge).toBeUndefined();
  });
});

describe("useDetailTabs 的状态", () => {
  function createState(initial: { inDetailMode: boolean; hasChannel: boolean }) {
    const inDetailMode = ref(initial.inDetailMode);
    const hasChannel = ref(initial.hasChannel);
    const channelDirty = ref(false);
    const state = useDetailTabs({
      inDetailMode,
      hasChannel: computed(() => hasChannel.value),
      channelDirty: computed(() => channelDirty.value),
      channelLabel: computed(() => labels.channelLabel),
      contactsLabel: computed(() => labels.contactsLabel),
    });
    return { ...state, inDetailMode, hasChannel, channelDirty };
  }

  it("切到联系人面后 resetTab 回到渠道面（换渠道 / 返回列表都靠它）", () => {
    const state = createState({ inDetailMode: true, hasChannel: true });
    expect(state.detailTab.value).toBe("channel");

    state.selectTab("contacts");
    expect(state.detailTab.value).toBe("contacts");

    state.resetTab();
    expect(state.detailTab.value).toBe("channel");
  });

  it("未知 key 一律回落渠道面，不会把详情页留成空面", () => {
    const state = createState({ inDetailMode: true, hasChannel: true });
    state.selectTab("contacts");
    state.selectTab("whatever");
    expect(state.detailTab.value).toBe("channel");
  });

  it("渠道被删掉后 tabs 变空，但内部 tab 状态仍可复位", () => {
    const state = createState({ inDetailMode: true, hasChannel: true });
    state.selectTab("contacts");
    state.hasChannel.value = false;
    expect(state.tabs.value).toEqual([]);

    state.resetTab();
    expect(state.detailTab.value).toBe("channel");
  });
});
