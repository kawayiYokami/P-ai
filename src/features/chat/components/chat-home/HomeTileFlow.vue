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
  /* 行高与非看板卡片的内容高度对齐：小卡图标行 + 两行文字大约 6.5rem */
  --ecall-home-row: 6.5rem;
  /* 卡片间距与圆角（--radius-box: 1rem）对齐 */
  --ecall-home-gap: 1rem;
  container-type: inline-size;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--ecall-home-tile), 1fr));
  /* 行高必须显式给：看板磁贴用 grid-row: span 3，行高为 auto 时
     它的高度取决于内容，会反过来改行高，和 dense 一起把网格排乱。
     下限而非固定值：内容多的卡自己撑高，不会被裁。 */
  grid-auto-rows: minmax(var(--ecall-home-row), auto);
  grid-auto-flow: dense;
  gap: var(--ecall-home-gap);
  align-content: start;
}

/* 窄容器下（如侧边栏单列时），宽卡自适应退化为单列，防止横向撑裂 */
@container (max-width: 320px) {
  :deep(.ecall-home-card-wide) {
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
