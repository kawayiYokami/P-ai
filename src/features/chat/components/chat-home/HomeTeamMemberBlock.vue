<template>
  <section class="ecall-home-team-block flex min-w-0 flex-col gap-2 rounded-box border border-base-content/10 bg-base-100 p-3.5">
    <div class="text-sm font-medium text-base-content/85">{{ t("chat.teamMember.title") }}</div>
    <OverlayScrollArea v-if="orderedMembers.length" scroller-class="ecall-home-team-scroller">
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
import { conversationProgressSortRank } from "../../utils/conversation-item-display";
import TeamMemberTileGrid from "../TeamMemberTileGrid.vue";
import type { TeamMemberTileItem } from "../team-member-tile";

/**
 * 首页「队员状态表」整块：标题 + 两行可见的队员列表 + 超出滚动。
 *
 * 自成组件的原因有两个：
 * 1. 它占磁贴墙两列、内部行高与滚动区高度是一组耦合的变量，散在宿主里谁都能改坏；
 * 2. demo 要看这块的真实形态，必须和首页渲染同一个组件，不能各抄一份。
 *
 * 高度跟内容走，不强行占两行格子——占两行会把区块撑到两行格子高，
 * 而紧凑列表两行只有 92px，多出来的空间只能变成底部一大片空白。
 */
const props = withDefaults(defineProps<{
  members?: TeamMemberTileItem[];
  avatarUrlMap?: Record<string, string>;
}>(), {
  members: () => [],
  avatarUrlMap: () => ({}),
});

/**
 * 按进度排序：名次直接取六态的定义顺序（规划中 → 待办 → 进行中 → 待审核 → 阻塞 → 完成），
 * 未设置排最后。同状态内保持传入顺序（也就是招募顺序），所以排序不是随机的重排。
 */
const orderedMembers = computed(() => {
  const indexed = props.members.map((member, index) => ({ member, index }));
  indexed.sort((a, b) =>
    conversationProgressSortRank(a.member.progressState) - conversationProgressSortRank(b.member.progressState)
    || a.index - b.index,
  );
  return indexed.map((entry) => entry.member);
});

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
}

/* 滚动区固定在两行的高度，超出的队员在里面滚，不把首页撑长。
   右侧留一点内边距，浮动滚动条不出现在内容正上方 */
:deep(.ecall-home-team-scroller) {
  height: calc(var(--ecall-team-member-row) * 2 + var(--ecall-team-member-gap));
  padding-right: 0.25rem;
}

/* 磁贴墙塌成单列时（右栏 320 宽），跨两格会挤出隐式列，退回单列 */
@container (max-width: 340px) {
  .ecall-home-team-block {
    grid-column: span 1;
  }
}
</style>
