import type { Component } from "vue";

export type PanelTabStripItem = {
  key: string;
  label: string;
  title?: string;
  badge?: string;
  /** 会话状态圆点的填充类；不给则不渲染圆点 */
  statusDotClass?: string;
  /** 状态圆点的提示文案（含证据原文），悬停可见 */
  statusTitle?: string;
  iconSrc?: string;
  icon?: Component;
  closeable?: boolean;
  disabled?: boolean;
};

export type PanelTabStripContextMenuItem = {
  label: string;
  onClick: (key: string) => void;
};

export type PanelTabStripVariant = "default" | "file";
