// 会话悬浮操作区元素的磨砂外观：圆钮与胶囊共用同一套底座，只差轮廓
// 聊天窗（工具栏对话菜单、SessionControlItems 工作区/运行监控、时间线圆钮）与 Demo 页共用同一份，改外观只改这里
const FROST_BASE =
  "btn btn-sm border border-base-300/50 bg-base-100/55 text-base-content/80 shadow-sm backdrop-blur-md backdrop-saturate-150 hover:bg-base-100/75";

// 圆形元素：对话菜单、时间线
export const SESSION_FLOAT_FROST_CIRCLE = `${FROST_BASE} btn-circle shrink-0`;

// 胶囊元素：工作区、运行监控；不写 shrink-0，窄容器里可收缩截断
export const SESSION_FLOAT_FROST_PILL = `${FROST_BASE} shrink min-w-0 disabled:cursor-not-allowed disabled:opacity-50`;
