<template>
  <!-- 外观交给 daisyUI 的 `breadcrumbs`：字重、颜色、分隔符、hover 下划线一律不在这里覆盖，
       两级用同一套样式（之前逐项设 text-sm/text-base/font-semibold，才出现"字体都不一样"）。
       只留三条：`text-base` 整层统一字号（与标题栏行里其它控件同级，不逐项设置）；
       `p-0` 去掉库给的上下 8px，让这一层能放进 40px 的标题栏行；`min-w-0` 允许截断。 -->
  <div v-if="visible" class="breadcrumbs min-w-0 p-0 text-base">
    <ul class="flex flex-wrap items-center">
      <li
        v-for="(item, index) in items"
        :key="`${index}-${item.label}`"
        class="flex min-w-0 items-center gap-1.5"
      >
        <a
          v-if="item.onClick"
          :title="item.title || item.label"
          @click="item.onClick()"
        >
          {{ item.label }}
        </a>
        <!-- 当前层不进 <a>、也不包元素：daisyUI 的 `li > *` 会给子元素加 cursor:pointer 与 hover 下划线，
             纯文本节点选不中，所以它天然不可点、无下划线，字也跟随父级。 -->
        <template v-else>{{ item.label }}</template>
        <span
          v-for="(badge, badgeIndex) in itemBadges(item)"
          :key="badgeIndex"
          class="badge badge-xs shrink-0"
          :class="[badge.class || 'badge-warning', badge.dotClass ? 'flex items-center gap-1.5' : '']"
        >
          <span v-if="badge.dotClass" class="size-2 shrink-0 rounded-full" :class="badge.dotClass"></span>
          <span>{{ badge.text }}</span>
        </span>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

export type SettingsBreadcrumbBadge = {
  text: string;
  /** 胶囊类名，默认 badge-warning */
  class?: string;
  /** 状态圆点类名，给了就在文字前加一枚圆点 */
  dotClass?: string;
};

export type SettingsBreadcrumbItem = {
  /** 面包屑文字 */
  label: string;
  /** 悬停提示，省略时用 label */
  title?: string;
  /** 给了就是可点返回的前序层级，不给就是当前层 */
  onClick?: () => void;
  /** 当前层尾部的单枚状态胶囊文字 */
  badge?: string;
  /** 单枚状态胶囊的类名，默认 badge-warning */
  badgeClass?: string;
  /** 当前层尾部的多枚状态胶囊，给了就忽略 badge */
  badges?: SettingsBreadcrumbBadge[];
};

const props = defineProps<{
  items: SettingsBreadcrumbItem[];
}>();

/**
 * 只要有一项就渲染。
 *
 * 标题栏那一层永远由面包屑占位：哪怕停在顶层、只有一个不可返回的层级
 * （如「供应商」「欢迎」），也必须显示当前页名——否则那一行是空的，
 * 后面的内容就会顶到窗口上沿。
 */
const visible = computed(() => props.items.length > 0);

function itemBadges(item: SettingsBreadcrumbItem): SettingsBreadcrumbBadge[] {
  if (item.badges && item.badges.length > 0) return item.badges;
  if (item.badge) return [{ text: item.badge, class: item.badgeClass }];
  return [];
}
</script>
