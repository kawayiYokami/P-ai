import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { UnderlineTabItem } from "../../../../shared/components/UnderlineTabs.vue";

export type DetailTabKey = "channel" | "contacts";

export type DetailTabsOptions = {
  /** 是否在详情态 */
  inDetailMode: boolean;
  /** 详情态是否真有可展示的渠道（删掉最后一个渠道后就没有了） */
  hasChannel: boolean;
  /** 渠道是否有未保存改动，给「渠道设置」标签挂个小圆点 */
  channelDirty: boolean;
  channelLabel: string;
  contactsLabel: string;
};

/**
 * 详情页的两个 tab。
 *
 * 关键：`hasChannel` 必须参与判断。只剩 `inDetailMode` 时，删掉最后一个渠道会出现
 * 「主体已回列表、页头还挂两个点了没反应的 tab」，而且 `SettingsPageShell` 在有 tabs 时
 * 用 `UnderlineTabs` 占掉左侧、不再渲染 `#left`，渠道搜索框会跟着消失。
 */
export function buildDetailTabs(options: DetailTabsOptions): UnderlineTabItem[] {
  if (!options.inDetailMode || !options.hasChannel) return [];
  return [
    {
      key: "channel",
      label: options.channelLabel,
      badge: options.channelDirty ? "●" : "",
    },
    { key: "contacts", label: options.contactsLabel },
  ];
}

/** 详情页 tab 状态：换渠道、返回列表都必须回到第一个面 */
export function useDetailTabs(params: {
  inDetailMode: Ref<boolean>;
  hasChannel: ComputedRef<boolean>;
  channelDirty: ComputedRef<boolean>;
  channelLabel: ComputedRef<string>;
  contactsLabel: ComputedRef<string>;
}) {
  const detailTab = ref<DetailTabKey>("channel");

  const tabs = computed<UnderlineTabItem[]>(() =>
    buildDetailTabs({
      inDetailMode: params.inDetailMode.value,
      hasChannel: params.hasChannel.value,
      channelDirty: params.channelDirty.value,
      channelLabel: params.channelLabel.value,
      contactsLabel: params.contactsLabel.value,
    }),
  );

  function selectTab(key: string) {
    detailTab.value = key === "contacts" ? "contacts" : "channel";
  }

  /** 回到渠道设置面（换渠道 / 返回列表时调用） */
  function resetTab() {
    detailTab.value = "channel";
  }

  return { detailTab, tabs, selectTab, resetTab };
}
