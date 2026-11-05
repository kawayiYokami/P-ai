<template>
  <div v-if="visible" class="breadcrumbs mb-2.5 min-w-0 p-0 text-xl sm:mb-3">
    <ul class="flex flex-wrap items-center">
      <li
        v-for="(item, index) in items"
        :key="`${index}-${item.label}`"
        class="flex min-w-0 items-center gap-2 py-1"
      >
        <a
          v-if="item.onClick"
          class="cursor-pointer py-1 text-base-content/50 transition-colors hover:text-base-content"
          :title="item.title || item.label"
          @click="item.onClick()"
        >
          {{ item.label }}
        </a>
        <span v-else class="min-w-0 max-w-[14rem] truncate py-1 font-semibold text-base-content sm:max-w-xs md:max-w-md">
          {{ item.label }}
        </span>
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
 * 顶层页只有一项、没有可返回的上级，它渲染出来的大标题和顶栏/左侧导航里
 * 已经显示的页名重复，占一整行还压着下面的 tab 栏。这种只有一项的面包屑不渲染；
 * 多层级（进详情、可返回）照常显示，否则会丢掉返回入口。
 * 末项如果挂了状态胶囊，信息无处可去，仍然渲染。
 */
const visible = computed(() => {
  if (props.items.length === 0) return false;
  if (props.items.length > 1) return true;
  const only = props.items[0];
  return Boolean(only.onClick || only.badge || (only.badges && only.badges.length > 0));
});

function itemBadges(item: SettingsBreadcrumbItem): SettingsBreadcrumbBadge[] {
  if (item.badges && item.badges.length > 0) return item.badges;
  if (item.badge) return [{ text: item.badge, class: item.badgeClass }];
  return [];
}
</script>
