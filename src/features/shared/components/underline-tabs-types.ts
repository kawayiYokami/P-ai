export type UnderlineTabItem = {
  key: string;
  label: string;
  badge?: string | number;
  disabled?: boolean;
  /** 选中时的文字色，默认 text-base-content */
  accentClass?: string;
  /** 选中时下划线的色，默认 bg-base-content */
  indicatorClass?: string;
};
