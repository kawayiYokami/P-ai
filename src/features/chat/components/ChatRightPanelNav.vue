<template>
  <div class="flex shrink-0 items-center">
    <button
      type="button"
      class="btn btn-ghost btn-sm btn-square"
      :disabled="!canGoBack"
      :title="t('chat.rightPanelNav.back')"
      :aria-label="t('chat.rightPanelNav.back')"
      @click.stop="emit('goBack')"
    >
      <AppIcon name="arrow-left-long" class="size-4 shrink-0 opacity-70" aria-hidden="true" />
    </button>
    <button
      type="button"
      class="btn btn-ghost btn-sm btn-square"
      :disabled="!canGoForward"
      :title="t('chat.rightPanelNav.forward')"
      :aria-label="t('chat.rightPanelNav.forward')"
      @click.stop="emit('goForward')"
    >
      <AppIcon name="arrow-right-long" class="size-4 shrink-0 opacity-70" aria-hidden="true" />
    </button>
    <button
      type="button"
      class="btn btn-ghost btn-sm btn-square"
      :title="t('chat.homePanel.title')"
      :aria-label="t('chat.homePanel.title')"
      @click.stop="emit('selectHome')"
    >
      <!-- 图标跟着磁贴墙的呈现样式走：磁贴态＝块状，网格态＝网格线 -->
      <AppIcon
        :name="homeTileStyle === 'tile' ? 'layout-grid' : 'grid-lines'"
        class="size-4 shrink-0 opacity-70"
        aria-hidden="true"
      />
    </button>
    <!-- 样式下拉：与 home 按钮分体的 ghost 箭头，只负责切换磁贴墙的呈现样式 -->
    <EcallDropdown
      v-model="styleMenuOpen"
      teleport
      :match-trigger-width="false"
      panel-class="w-28 p-0"
      placement="bottom"
    >
      <template #trigger="{ toggle }">
        <button
          type="button"
          class="btn btn-ghost btn-xs btn-square -ml-1"
          :title="t('chat.homePanel.styleSwitch')"
          :aria-label="t('chat.homePanel.styleSwitch')"
          @click.stop="toggle"
        >
          <AppIcon name="chevron-down" class="size-3 shrink-0 opacity-60" aria-hidden="true" />
        </button>
      </template>
      <ul class="menu w-full p-1">
        <li v-for="option in styleOptions" :key="option.value">
          <button
            type="button"
            :class="homeTileStyle === option.value ? 'active font-medium' : ''"
            @click="chooseStyle(option.value)"
          >
            <AppIcon :name="option.icon" class="size-3.5 shrink-0 opacity-70" aria-hidden="true" />
            {{ t(option.labelKey) }}
          </button>
        </li>
      </ul>
    </EcallDropdown>
  </div>
</template>

<script setup lang="ts">
/**
 * 右面板 tab 条左侧的导航条：后退 / 前进 / 回预览，末尾挂一个磁贴墙样式下拉。
 * 四条 tab 条（文件页、工具审查、侧边会话、队伍成员）共用这一个组件，
 * 后退在文件页和之前的队伍对话之间来回切时才不会断。
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import EcallDropdown from "../../shared/components/EcallDropdown.vue";
import type { AppIconName } from "../../shared/icons/icon-map";
import { useHomeTileStyle, type HomeTileStyle } from "../composables/use-home-tile-style";

withDefaults(defineProps<{
  canGoBack?: boolean;
  canGoForward?: boolean;
}>(), {
  canGoBack: false,
  canGoForward: false,
});

const emit = defineEmits<{
  (e: "goBack"): void;
  (e: "goForward"): void;
  (e: "selectHome"): void;
}>();

const { t } = useI18n();
const { homeTileStyle, setHomeTileStyle } = useHomeTileStyle();

const styleMenuOpen = ref(false);

/** 两项的图标与 home 按钮同一套：磁贴＝块状、网格＝网格线 */
const styleOptions: Array<{ value: HomeTileStyle; icon: AppIconName; labelKey: string }> = [
  { value: "tile", icon: "layout-grid", labelKey: "chat.homePanel.tileStyle" },
  { value: "grid", icon: "grid-lines", labelKey: "chat.homePanel.gridStyle" },
];

function chooseStyle(next: HomeTileStyle) {
  setHomeTileStyle(next);
  styleMenuOpen.value = false;
}
</script>
