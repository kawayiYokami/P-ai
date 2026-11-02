<template>
  <div class="ecall-home-flow">
    <slot />
  </div>
</template>

<script setup lang="ts">
/**
 * 首页磁贴墙的网格容器。
 *
 * 为什么单独成组件：demo 页面要看磁贴区块的跨列与伸缩，必须复现同一套列宽、行高、
 * 间距与窄档规则。以前 demo 里手抄了一份 `demo-home-flow`，参数一改就两边不一致，
 * 所以统一收在这里，真实首页与 demo 都渲染这一个组件。
 */
</script>

<style scoped>
/* 固定网格：列宽固定、宽卡跨两格，同一行的卡片自动等高 */
.ecall-home-flow {
  --ecall-home-tile: 8.75rem;
  /* 行高由宿主统一决定：环境卡四行（标题 + 变更/分支/最新提交）与队员状态表一行 3 人都在这一档内不裁切。
     10.25rem = 内容卡所需高度；磁贴内容再多也在卡内滚动，不许撑高这一行。 */
  --ecall-home-row: 10.25rem;
  /* 卡片间距与圆角（--radius-box: 1rem）对齐 */
  --ecall-home-gap: 1rem;
  container-type: inline-size;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--ecall-home-tile), 1fr));
  /* 行高是硬的：单元多高只由这里决定，内容撑不开它。
     卡片占几行几列由宿主算好传进磁贴，磁贴自己改不了尺寸；
     内容超出就在卡片内部滚动。 */
  grid-auto-rows: var(--ecall-home-row);
  grid-auto-flow: dense;
  gap: var(--ecall-home-gap);
  align-content: start;
}

/* 窄容器下（如侧边栏单列时），所有卡统一压成单列，防止横向撑裂。
   网格容器是尺寸语言的宿主，磁贴自己压不算。 */
@container (max-width: 320px) {
  :deep(.ecall-home-flow > *) {
    grid-column: span 1 !important;
  }
}

.ecall-home-flow > * {
  animation: ecall-home-card-enter 220ms cubic-bezier(0.2, 0, 0, 1);
}

@keyframes ecall-home-card-enter {
  from {
    opacity: 0;
    transform: scale(0.94);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ecall-home-flow > * {
    animation: none;
  }
}
</style>
