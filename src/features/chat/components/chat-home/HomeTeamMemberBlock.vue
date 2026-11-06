<template>
  <section
    class="ecall-home-team-block flex h-full min-w-0 flex-col rounded-box border border-base-content/10 bg-base-100 p-3"
    :style="{ gridColumn: `span ${colSpan}`, gridRow: `span ${rowSpan}` }"
  >
    <header class="mb-2 flex shrink-0 items-center gap-2">
      <span class="min-w-0 flex-1 truncate text-xs font-semibold text-base-content/80">
        {{ t("chat.teamMember.blockTitle") }}
      </span>
      <button
        type="button"
        class="btn btn-ghost btn-xs h-6 shrink-0 gap-1 px-1.5 text-xs text-base-content/50 transition-colors hover:bg-base-content/10 hover:text-base-content"
        @click="emit('openTeamManagement')"
      >
        <span>{{ t("chat.teamMember.more") }}</span>
        <AppIcon name="arrow-up-right" class="size-3" aria-hidden="true" />
      </button>
    </header>

    <OverlayScrollArea v-if="orderedMembers.length" class="flex-1 min-h-0 w-full" scroller-class="ecall-home-team-scroller ecall-fade-bottom">
      <TeamMemberTileGrid
        :members="orderedMembers"
        :avatar-url-map="avatarUrlMap"
        @select-member="emit('selectMember', $event)"
      />
    </OverlayScrollArea>
    <div v-else class="flex flex-1 items-center justify-center px-2 text-center text-xs text-base-content/50">
      {{ t("chat.teamMember.empty") }}
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import TeamMemberTileGrid from "../TeamMemberTileGrid.vue";
import { teamMemberWorkRank, type TeamMemberTileItem } from "../team-member-tile";

/**
 * 首页「队员状态表」整块：标题行 + 全体队员 + 内部滚动。
 *
 * 全部队员都上表，不按工作状态筛：这一块回答的是「我的队伍里都有谁」，
 * 谁暂时闲着不该从列表里消失。排序仍走 teamMemberWorkRank，
 * 调度中（0 忙碌且有议题、1 仅忙碌）自然排在最前，出错与空闲跟在后。
 *
 * 占几行几列由宿主 ChatHomePanel 算好通过 colSpan / rowSpan 下发，
 * 这一块不按人数自己涨高；超出可用高度后在内部平滑滚动，
 * 底部由全局 `.ecall-fade-bottom` 渐隐，不在组件里另写渐变。
 *
 * 排序：按 teamMemberWorkRank 分级，同状态内保持稳定招募顺序。
 */
const props = withDefaults(defineProps<{
  members?: TeamMemberTileItem[];
  avatarUrlMap?: Record<string, string>;
  /** 占几列：宿主算好下发 */
  colSpan?: number;
  /** 占几行：宿主算好下发 */
  rowSpan?: number;
}>(), {
  members: () => [],
  avatarUrlMap: () => ({}),
  colSpan: 2,
  rowSpan: 1,
});

const orderedMembers = computed(() => {
  const indexed = props.members.map((member, index) => ({ member, index }));
  indexed.sort((a, b) =>
    teamMemberWorkRank(a.member) - teamMemberWorkRank(b.member)
    || a.index - b.index,
  );
  return indexed.map((entry) => entry.member);
});

const emit = defineEmits<{
  (e: "selectMember", conversationId: string): void;
  /** 右上角「更多」：打开队员页，由上层接到队伍管理入口。 */
  (e: "openTeamManagement"): void;
}>();

const { t } = useI18n();
</script>

<style scoped>
.ecall-home-team-block {
  /* 行高是量出来的：py-1.5（上下各 6px）+ 头像 h-5（20px，与文字行高同高）= 32px。
     两个值在这里定义，磁贴组件只读不写，避免宿主与组件各改一半。 */
  --ecall-team-member-row: 32px;
  --ecall-team-member-gap: 0.25rem;
  transition: all 0.24s cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* 滚动区自适应吃满卡片剩余高度；超出的队员在里面滚。
   右侧留一点内边距，浮动滚动条不出现在内容正上方 */
:deep(.ecall-home-team-scroller) {
  height: 100%;
  padding-right: 0.25rem;
}
</style>
