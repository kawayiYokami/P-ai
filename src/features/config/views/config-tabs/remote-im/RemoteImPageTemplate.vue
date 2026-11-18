<!-- 联系人页中间模板：只负责区块间距与空/错态，别的一概不管。
     滚动与内容内边距由 `SettingsPageShell`（→ `SettingsStickyLayout` 的 scroller + `p-4 max-w-5xl`）
     提供，所以这里**不套** `OverlayScrollArea`、也**不加** `p-4` / `max-w-5xl`：
     否则内边距会翻倍、页面里会多一层滚动容器。
     `errorText` / `emptyText` 二选一：给了就不渲染默认槽（空态是邀请下一步动作，不是一行「暂无数据」）。 -->
<template>
  <div class="space-y-3">
    <div
      v-if="errorText"
      class="rounded-box border border-error/20 bg-error/10 px-4 py-3 text-xs text-error break-all whitespace-pre-wrap"
      style="overflow-wrap: anywhere"
    >
      {{ errorText }}
    </div>
    <div
      v-else-if="emptyText"
      class="rounded-box border border-dashed border-base-300 py-12 text-center text-xs opacity-60 italic"
    >
      {{ emptyText }}
    </div>
    <slot v-else />
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    /** 出错时就地显示，替掉内容区 */
    errorText?: string;
    /** 空态提示，替掉内容区 */
    emptyText?: string;
  }>(),
  { errorText: "", emptyText: "" },
);
</script>
