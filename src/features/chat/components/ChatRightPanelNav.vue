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
      <AppIcon name="layout-grid" class="size-4 shrink-0 opacity-70" aria-hidden="true" />
    </button>
  </div>
</template>

<script setup lang="ts">
/**
 * 右面板 tab 条左侧的导航条：后退 / 前进 / 回预览。
 * 四条 tab 条（文件页、工具审查、侧边会话、队伍成员）共用这一个组件，
 * 后退在文件页和之前的队伍对话之间来回切时才不会断。
 */
import AppIcon from "../../shared/components/AppIcon.vue";
import { useI18n } from "vue-i18n";

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
</script>
