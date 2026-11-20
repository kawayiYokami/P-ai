<template>
  <!-- 结构照 daisyUI 官方 `breadcrumbs`：div.breadcrumbs > ul > li。外观（字号、分隔符、
       层级颜色）一律不在这里逐项覆盖，只做两处产品化调整：
       1. 第一层是一枚设置图标，当作「设置」这个根目录，后面接层级，分隔符由 daisyUI 的
          `li + li` 自己给，所以图标必须是一个 <li>，不能放在 ul 外面；
       2. 可点层级去掉 daisyUI 默认的 hover 下划线，换成 hover 底色（负 margin 抵消内边距的位移）。 -->
  <div v-if="visible" class="breadcrumbs min-w-0 p-0 text-sm">
    <ul class="flex flex-wrap items-center">
      <li class="flex min-w-0 items-center">
        <!-- pointer-events-none：daisyUI 的 `li > *` 会给子元素加 cursor:pointer，
             图标不可点，让鼠标事件穿过去，光标就不会变成手型。 -->
        <AppIcon name="settings" class="pointer-events-none h-4 w-4 shrink-0 opacity-60" />
      </li>
      <li
        v-for="(item, index) in items"
        :key="`${index}-${item.label}`"
        class="flex min-w-0 items-center gap-1.5"
      >
        <a
          v-if="item.onClick"
          class="-mx-1.5 rounded px-1.5 py-0.5 no-underline! hover:bg-base-200 hover:no-underline!"
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
import AppIcon from "../../shared/components/AppIcon.vue";
import type { SettingsBreadcrumbBadge, SettingsBreadcrumbItem } from "./settings-breadcrumb-types";

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
