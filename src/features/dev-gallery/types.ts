import type { Component } from "vue";

/**
 * 组件库的一项：只记它是谁、叫什么、内容从哪来。
 * 刻意不含用法、props、代码示例——组件库是看效果的，不是文档站。
 */
export type GalleryEntry = {
  /** 稳定标识，?key= 直达用它，与旧 demo 页的 key 保持一致 */
  key: string;
  /** 英文组件名，取自真实组件文件名 */
  name: string;
  /** 中文说明：它是什么，不讲怎么用 */
  title: string;
  /** `demos/` 下的文件名（不带 `.vue`），由 registry 统一登记 */
  file: string;
  /** 内容懒加载：只有被选中时，那一棵子树才进内存 */
  loader: () => Promise<{ default: Component }>;
  /** 传给内容组件的参数；同一组件要出两种形态时用它区分（例如人格主页 / 人格权限页） */
  props?: Record<string, unknown>;
};

export type GalleryGroup = {
  id: string;
  title: string;
  entries: GalleryEntry[];
};
