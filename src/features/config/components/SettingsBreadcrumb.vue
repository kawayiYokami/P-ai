<template>
  <!-- 结构照 daisyUI 官方 breadcrumbs：div.breadcrumbs > ul > li，
       图标与文字在同一个 <a>/<span> 里，分隔符由 `li + li` 自己给。 -->
  <div v-if="visible" class="breadcrumbs text-sm">
    <ul>
      <li
        v-for="(item, index) in items"
        :key="`${index}-${item.label}`"
      >
        <a
          v-if="item.onClick"
          :title="item.title || item.label"
          @click="item.onClick()"
        >
          <AppIcon v-if="index === 0" name="settings" class="h-4 w-4 stroke-current" />
          {{ item.label }}
        </a>
        <span v-else class="pointer-events-none inline-flex items-center gap-2">
          <AppIcon v-if="index === 0" name="settings" class="h-4 w-4 stroke-current" />
          {{ item.label }}
        </span>
        <span
          v-for="(badge, badgeIndex) in itemBadges(item)"
          :key="badgeIndex"
          class="badge badge-xs"
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
