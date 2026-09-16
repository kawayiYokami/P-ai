// 会话悬浮操作区元素的磨砂底座外观：圆钮、胶囊与预览卡共用同一套底座
// 聊天窗（工具栏对话菜单、SessionControlItems 工作区/运行监控、时间线圆钮、思维链预览条）与 Demo 页共用同一份
// 统一毛玻璃：半透明底座色 + 背景模糊，非交互面（如覆盖层卡片）直接用 FROST_GLASS，交互元素叠 FROST_SURFACE
export const FROST_GLASS =
  "border border-base-300/60 bg-base-100/70 backdrop-blur-md backdrop-saturate-150";

export const FROST_SURFACE =
  `${FROST_GLASS} text-base-content/85 shadow-sm hover:bg-base-100/85 active:scale-[0.98] transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-1`;

const FROST_BASE = `btn btn-sm ${FROST_SURFACE}`;

// 圆形元素：对话菜单、时间线
export const SESSION_FLOAT_FROST_CIRCLE = `${FROST_BASE} btn-circle shrink-0`;

// 胶囊元素：工作区、运行监控；不写 shrink-0，窄容器里可收缩截断
export const SESSION_FLOAT_FROST_PILL = `${FROST_BASE} shrink min-w-0 font-normal disabled:cursor-not-allowed disabled:opacity-50`;

// 预览卡元素：思维链预览条等
export const SESSION_FLOAT_FROST_CARD = `w-fit min-w-0 max-w-full cursor-pointer rounded-2xl px-3 py-2 text-left text-xs ${FROST_SURFACE}`;
