import type { AppIconName } from "../icons/icon-map";

export type PanelTabStripItem = {
  key: string;
  label: string;
  title?: string;
  badge?: string;
  /** 会话状态圆点的填充类；不给则不渲染圆点 */
  statusDotClass?: string;
  /** 状态圆点的提示文案（含证据原文），悬停可见 */
  statusTitle?: string;
  /** 会话正在跑：标签右侧挂一个转圈，标题再长也不挤它 */
  busy?: boolean;
  iconSrc?: string;
  /** 图标语义名，查 APP_ICON_MAP */
  icon?: AppIconName;
  closeable?: boolean;
  disabled?: boolean;
};

export type PanelTabStripContextMenuItem = {
  label: string;
  onClick: (key: string) => void;
};

export type PanelTabStripVariant = "default" | "file";

/**
 * 没有标签的标题栏（磁贴页）：传这个常量，别在模板里写 `:tabs="[]"`——
 * 每次渲染新建空数组会让子组件的 props 判定为变化，整条标题栏跟着重算。
 *
 * 放在 types 文件而不是组件里：`<script setup>` 不允许运行时 export。
 */
export const EMPTY_PANEL_TABS: PanelTabStripItem[] = Object.freeze([]) as unknown as PanelTabStripItem[];

/** 没有右键菜单项的标题栏（非文件页）用这个，理由同上。 */
export const EMPTY_PANEL_CONTEXT_MENU: PanelTabStripContextMenuItem[] =
  Object.freeze([]) as unknown as PanelTabStripContextMenuItem[];
