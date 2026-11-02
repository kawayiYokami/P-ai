<template>
  <section
    class="ecall-home-team-block flex h-full min-w-0 flex-col rounded-box border border-base-content/10 bg-base-100 p-3"
    :class="[isMultiRow ? 'ecall-home-team-row-2' : 'ecall-home-team-row-1']"
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
        <ArrowUpRight class="size-3" aria-hidden="true" />
      </button>
    </header>

    <OverlayScrollArea v-if="workingMembers.length" class="flex-1 min-h-0 w-full" scroller-class="ecall-home-team-scroller">
      <TeamMemberTileGrid
        :members="workingMembers"
        :avatar-url-map="avatarUrlMap"
        @select-member="emit('selectMember', $event)"
      />
    </OverlayScrollArea>
    <div v-else class="flex flex-1 items-center justify-center px-2 text-center text-xs text-base-content/50">
      {{ t("chat.teamMember.emptyWorking") }}
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUpRight } from "@lucide/vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import TeamMemberTileGrid from "../TeamMemberTileGrid.vue";
import { teamMemberWorkRank, type TeamMemberTileItem } from "../team-member-tile";

/**
 * 首页「队员状态表」整块：标题行 + 工作中的队员 + 自适应高度与超出滚动。
 *
 * 只放正在干活的人：判定直接用 teamMemberWorkRank，0/1/2
 * （忙碌且有在办议题、仅忙碌、有在办议题）算工作中；
 * 3（运行出错）与 4（空闲）不进磁贴——这里回答的是「现在谁在替我干活」，
 * 空闲与出错在队员页里看。判定不另写一套，免得两处口径分叉。
 *
 * 高度自适应：
 * - 工作中 0~2 人时保持标准 1 格高（span 1）；
 * - 3 人及以上自适应扩展为 2 格高（最多 2 格高）；
 * - 超过 2 格可容纳高度后在内部平滑滚动，不无休止占高破坏网格。
 * - 全部空闲时卡片不隐藏，留在原地显示一句空态，避免首页网格跳动。
 *
 * 排序：仍按 teamMemberWorkRank 分级，同状态内保持稳定招募顺序。
 */
const props = withDefaults(defineProps<{
  members?: TeamMemberTileItem[];
  avatarUrlMap?: Record<string, string>;
}>(), {
  members: () => [],
  avatarUrlMap: () => ({}),
});

/** rank 小于它就上磁贴：0 忙碌且有议题、1 仅忙碌、2 仅有在办议题。 */
const WORKING_RANK_LIMIT = 3;

const workingMembers = computed(() => {
  const indexed = props.members
    .map((member, index) => ({ member, index }))
    .filter((entry) => teamMemberWorkRank(entry.member) < WORKING_RANK_LIMIT);
  indexed.sort((a, b) =>
    teamMemberWorkRank(a.member) - teamMemberWorkRank(b.member)
    || a.index - b.index,
  );
  return indexed.map((entry) => entry.member);
});

/**
 * 工作中的队员少于 3 个（0、1、2 个）时保持标准单行高（1格高）；
 * 达到 3 个及以上时自适应展开为 2 格高（最多 2 格高），容纳更多队员。
 */
const isMultiRow = computed(() => workingMembers.value.length >= 3);

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
  grid-column: span 2;
  transition: all 0.24s cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* 1~2 名队员（或空态）：标准单行卡（跨 1 行高），与环境卡片等单行卡平齐 */
.ecall-home-team-block.ecall-home-team-row-1 {
  grid-row: span 1;
}

/* 3 名及以上队员：跨两行高（最多 2 格高），容纳更多队员，超出部分在卡片内平滑滚动 */
.ecall-home-team-block.ecall-home-team-row-2 {
  grid-row: span 2;
}

/* 滚动区自适应吃满卡片剩余高度；超出的队员在里面滚。
   右侧留一点内边距，浮动滚动条不出现在内容正上方 */
:deep(.ecall-home-team-scroller) {
  height: 100%;
  padding-right: 0.25rem;
}

/* 磁贴墙塌成单列时（右栏 320 宽），跨两格会挤出隐式列，退回单列 */
@container (max-width: 340px) {
  .ecall-home-team-block {
    grid-column: span 1;
  }
}
</style>
