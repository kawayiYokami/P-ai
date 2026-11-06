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
