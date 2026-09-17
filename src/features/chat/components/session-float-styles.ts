// 会话悬浮操作区元素的底座外观：圆钮、胶囊与预览卡共用同一套底座
// 聊天窗（工具栏对话菜单、工作区/运行监控、时间线圆钮、思维链预览条）与 Demo 页共用同一份
// 工作条内的按钮统一走 daisyUI ghost：不画底色与描边，层次只靠文字与悬停底色；
// 悬浮区其他独立面仍用无阴影描边（不透明 base-100 底色 + 边框），非交互面（如覆盖层卡片）用磨砂 FROST_GLASS
const FROST_BORDER = "border border-base-300";

export const FROST_GLASS = `${FROST_BORDER} bg-base-100/70 backdrop-blur-md backdrop-saturate-150`;

// bg-base-100 / hover:bg-base-100 / shadow-none 是必要的显式覆盖：这些元素带 daisyUI 的 btn 类，
// 不钉住的话 btn 自带的底色与阴影会顶回来
export const FROST_SURFACE =
  `${FROST_BORDER} bg-base-100 shadow-none text-base-content/85 hover:border-base-300 hover:bg-base-100 hover:text-base-content active:scale-[0.98] transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-1`;

const FROST_BASE = `btn btn-sm ${FROST_SURFACE}`;

// 圆形元素：对话菜单、时间线
export const SESSION_FLOAT_FROST_CIRCLE = `${FROST_BASE} btn-circle shrink-0`;

// 胶囊元素：工作区、运行监控；不写 shrink-0，窄容器里可收缩截断
const GHOST_BASE = "btn btn-sm btn-ghost";
export const SESSION_GHOST_PILL = `${GHOST_BASE} shrink min-w-0 font-normal disabled:cursor-not-allowed disabled:opacity-50`;

// 预览卡元素：思维链预览条等
export const SESSION_FLOAT_FROST_CARD = `w-fit min-w-0 max-w-full cursor-pointer rounded-2xl px-3 py-2 text-left text-xs ${FROST_SURFACE}`;
