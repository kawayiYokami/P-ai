<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-3">
      <div class="ecall-kanban-demo-flow w-full max-w-[586px]">
        <HomeKanbanCard
          :issues="issues"
          :avatar-url-map="avatarMap"
          leader-name="Claude"
          :leader-avatar-url="avatarMap['leader']"
        />
        <div class="ecall-kanban-demo-filler">环境卡占位</div>
        <div class="ecall-kanban-demo-filler">计划卡占位</div>
        <div class="ecall-kanban-demo-filler">工作区卡占位</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import HomeKanbanCard from "../../chat/components/chat-home/HomeKanbanCard.vue";
import type { KanbanIssue } from "../../chat/composables/use-kanban-board";

function svgDataUrl(svg: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const avatarMap: Record<string, string> = {
  leader: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><g fill="#FFFFFF" transform="translate(10, 10)"><path d="M10 2.5a1.5 1.5 0 0 0-1.5 1.5v3.2A7.5 7.5 0 0 0 4 10a7.5 7.5 0 0 0 7.5 7.5 7.5 7.5 0 0 0 6-3v1.5a1.5 1.5 0 0 0 3 0V4a1.5 1.5 0 0 0-1.5-1.5H10z" opacity="0.95"/><circle cx="10" cy="10" r="3" fill="#FDE68A"/></g></svg>`),
  小吉祥草王: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#10B981"/><path d="M20 9c-4 5-3 10 0 14 3-4 4-9 0-14zM14 17c5-2 9 0 11 3-5 2-9 0-11-3zM26 17c-5-2-9 0-11 3 5 2 9 0 11-3z" fill="#ECFDF5"/><circle cx="20" cy="18" r="3" fill="#A7F3D0"/></svg>`),
  莫娜: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#6366F1"/><path d="M20 8l2.5 8.5L31 19l-8.5 2.5L20 30l-2.5-8.5L9 19l8.5-2.5z" fill="#EEF2FF"/><circle cx="20" cy="19" r="2.5" fill="#C7D2FE"/></svg>`),
  欧科塔维亚: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#06B6D4"/><circle cx="20" cy="20" r="9" fill="none" stroke="#E0F2FE" stroke-width="2.5"/><path d="M16 16v8l7-4z" fill="#E0F2FE"/></svg>`),
  桑多涅: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#EC4899"/><circle cx="20" cy="20" r="6" fill="#FCE7F3"/><path d="M14 16c3 2 3 6 0 8 3-1 6-1 8-3-2-2-5-2-8-5zM26 16c-3 2-3 6 0 8-3-1-6-1-8-3 2-2 5-2 8-5z" fill="#FFF1F2"/></svg>`),
  钟离: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><rect x="13" y="13" width="14" height="14" rx="2" fill="none" stroke="#FEF3C7" stroke-width="2.5"/><rect x="17" y="17" width="6" height="6" fill="#FEF3C7"/></svg>`),
  小黑塔: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#8B5CF6"/><circle cx="20" cy="15" r="4.5" fill="#EDE9FE"/><path d="M13 27c0-4 3.5-6 7-6s7 2 7 6z" fill="#EDE9FE"/></svg>`),
};

const issues: KanbanIssue[] = [
  { number: 3, title: "发布流程要不要加一步回归检查", status: "in_review", assignee: "小吉祥草王", note: "三条回归都跑过了，等确认口径", updatedAt: "2026-11-01T11:20:00+08:00" },
  { number: 5, title: "看板磁贴尺寸定稿", status: "in_review", assignee: "莫娜", updatedAt: "2026-11-01T11:35:00+08:00", note: "" },
  { number: 11, title: "提问工具的回答要不要带时间戳", status: "in_review", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:46:00+08:00", note: "" },
  { number: 12, title: "队伍名册改角色后的文案", status: "in_review", assignee: "leader", updatedAt: "2026-11-01T11:47:00+08:00", note: "" },
  { number: 7, title: "队员 3 号的 worktree 有未解决冲突", status: "blocked", assignee: "桑多涅", note: "两边都改了 ChatView.vue，需要你定谁先合", updatedAt: "2026-11-01T10:05:00+08:00" },
  { number: 13, title: "摘要工具的回滚逻辑要重做", status: "blocked", assignee: "钟离", note: "等确认快照失败时回滚到哪一版", updatedAt: "2026-11-01T10:30:00+08:00" },
  { number: 8, title: "提问工具要不要支持多选", status: "discussing", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:40:00+08:00", note: "" },
  { number: 9, title: "会话行待定问题数的视觉样式", status: "discussing", assignee: "桑多涅", updatedAt: "2026-11-01T11:42:00+08:00", note: "" },
  { number: 10, title: "议题编号不回收的边界测试", status: "discussing", assignee: "莫娜", updatedAt: "2026-11-01T11:44:00+08:00", note: "" },
  { number: 14, title: "看板页要不要支持拖拽改状态", status: "discussing", assignee: "莫娜", updatedAt: "2026-11-01T11:48:00+08:00", note: "" },
  { number: 1, title: "模型继承链改成父会话优先", status: "in_progress", assignee: "小黑塔", updatedAt: "2026-11-01T11:10:00+08:00", note: "" },
  { number: 2, title: "专家模型改成被动跟随", status: "todo", assignee: "小黑塔", note: "" },
  { number: 4, title: "重跑队长提示词行为测试", status: "todo", assignee: "观测员", note: "" },
  { number: 6, title: "队伍名册改用角色标识", status: "done", assignee: "桑多涅", note: "已确认由界面通过" },
];
</script>

<style scoped>
/* 复刻 ChatHomePanel 的 .ecall-home-flow：列宽 8.75rem、间距 1rem、
   行高下限 6.5rem，磁贴靠 1 / -1 与 span 3 撑成 3×3。 */
.ecall-kanban-demo-flow {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(8.75rem, 1fr));
  grid-auto-rows: minmax(6.5rem, auto);
  gap: 1rem;
  grid-auto-flow: dense;
}

.ecall-kanban-demo-filler {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed color-mix(in oklab, var(--color-base-content) 20%, transparent);
  border-radius: 1rem;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 40%, transparent);
}
</style>
