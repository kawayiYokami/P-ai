<template>
  <section
    class="ecall-home-team-block flex h-full min-w-0 flex-col gap-2 rounded-box border border-base-content/10 bg-base-100 p-3.5"
    :class="[isMultiRow ? 'ecall-home-team-row-2' : 'ecall-home-team-row-1']"
  >
    <div class="text-sm font-medium text-base-content/85">{{ t("chat.teamMember.title") }}</div>
    <OverlayScrollArea v-if="orderedMembers.length" class="flex-1 min-h-0 w-full" scroller-class="ecall-home-team-scroller">
      <TeamMemberTileGrid
        :members="orderedMembers"
        :avatar-url-map="avatarUrlMap"
        @select-member="emit('selectMember', $event)"
      />
    </OverlayScrollArea>
    <div v-else class="text-xs text-base-content/50">{{ t("chat.teamMember.empty") }}</div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import TeamMemberTileGrid from "../TeamMemberTileGrid.vue";
import { teamMemberWorkRank, type TeamMemberTileItem } from "../team-member-tile";

/**
 * 首页「队员状态表」整块：标题 + 队员列表 + 自适应高度与超出滚动。
 *
 * 高度自适应：
 * - 0~2 名队员时保持标准 1 格高（span 1）；
 * - 3 名及以上队员自适应扩展为 2 格高（最多 2 格高），容纳更多队员；
 * - 超过 2 格可容纳高度后在内部平滑滚动，不无休止占高破坏网格。
 *
 * 排序：
 * - 工作中（调度忙碌、有在办议题）排在最前；
 * - 运行出错排中间；
 * - 空闲排在最后；
 * - 同状态内保持稳定招募顺序。
 */
const props = withDefaults(defineProps<{
  members?: TeamMemberTileItem[];
  avatarUrlMap?: Record<string, string>;
}>(), {
  members: () => [],
  avatarUrlMap: () => ({}),
});

/**
 * 按工作状态排序：工作中（忙碌/在办议题）优先排前，空闲排最后。
 * 同状态内保持招募顺序。
 */
const orderedMembers = computed(() => {
  const indexed = props.members.map((member, index) => ({ member, index }));
  indexed.sort((a, b) =>
    teamMemberWorkRank(a.member) - teamMemberWorkRank(b.member)
    || a.index - b.index,
  );
  return indexed.map((entry) => entry.member);
});

/**
 * 队员少于 3 个（0、1、2 个）时保持标准单行高（1格高）；
 * 队员达到 3 个及以上时自适应展开为 2 格高（最多 2 格高），容纳更多队员。
 */
const isMultiRow = computed(() => orderedMembers.value.length >= 3);

const emit = defineEmits<{
  (e: "selectMember", conversationId: string): void;
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
