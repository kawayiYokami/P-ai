/**
 * 队员磁贴的数据形状。
 *
 * 刻意不从 TeamManageMember 继承：磁贴只关心这几个字段，而队伍管理弹窗与首页磁贴墙
 * 是两个落点，不该被彼此的字段拖着改。谁传数据谁做一次映射。
 *
 * 单独放在 `.ts` 而不是 `<script setup>` 里导出：SFC 的 script setup 只承载组件自身，
 * 类型被多处消费时留在里面会让每个消费方都要写一个绕路的导入。
 */
export type TeamMemberTileItem = {
  conversationId: string;
  title: string;
  teamName?: string;
  agentId?: string;
  agentName?: string;
  /** 会话状态六态 key；空串与缺失都归到「未设置」 */
  progressState?: string;
  /** 运行态：调度中/整理上下文等在跑，磁贴末尾挂一个转圈 */
  runtimeState?: string;
  /**
   * 这名队员名下此刻在办的那条议题。有值时磁贴写一句「正在处理 #3 标题」；
   * 没有时只显示他自己的运行时状态（忙碌 / 空闲 / 出错）。
   * 一个队员最多一条在办，所以是单个对象而不是数组。
   */
  activeIssue?: TeamMemberActiveIssue;
  /**
   * 他此刻在不在跑：busy / idle / error，后端按运行态与待处理队列算好下发。
   * 与议题状态是两个维度，磁贴只在没有在办议题时显示它。
   */
  memberState?: string;
  /** 以下三项只喂标题降级链，磁贴自己不显示 */
  summaryTitle?: string;
  updatedAt?: string;
  lastMessageAt?: string;
};

export type TeamMemberActiveIssue = {
  number: number;
  title: string;
  status: string;
};
