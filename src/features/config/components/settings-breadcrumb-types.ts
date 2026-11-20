export type SettingsBreadcrumbBadge = {
  text: string;
  /** 胶囊类名，默认 badge-warning */
  class?: string;
  /** 状态圆点类名，给了就在文字前加一枚圆点 */
  dotClass?: string;
};

export type SettingsBreadcrumbItem = {
  /** 面包屑文字 */
  label: string;
  /** 悬停提示，省略时用 label */
  title?: string;
  /** 给了就是可点返回的前序层级，不给就是当前层 */
  onClick?: () => void;
  /** 当前层尾部的单枚状态胶囊文字 */
  badge?: string;
  /** 单枚状态胶囊的类名，默认 badge-warning */
  badgeClass?: string;
  /** 当前层尾部的多枚状态胶囊，给了就忽略 badge */
  badges?: SettingsBreadcrumbBadge[];
};
