/**
 * 平铺 diff 视图（DiffFlowView）的数据形状。
 *
 * 一项 = 一次改动段，不按文件聚合：同一文件改了几次就平铺几项，顺序与工具调用一致。
 * 这样点导航的序号和实际改动顺序对得上，不会把两次不同时间的修改揉成一块。
 */

export type DiffFlowItem = {
  key: string;
  /** 用于展示的路径，由调用方决定压缩口径（例如去掉工作区绝对前缀） */
  path: string;
  added: number;
  removed: number;
  /** 这一次改动段的原始 diff 行；多个 `@@` 段首尾相接，渲染器按 hunk 头自行接续行号 */
  lines: string[];
  /** 打开文件用的原始路径：`path` 可能被调用方压成省略形式，不能拿来读文件 */
  sourcePath?: string;
};
