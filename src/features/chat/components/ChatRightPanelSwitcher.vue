<template>
  <button
    type="button"
    class="btn btn-ghost btn-sm btn-square shrink-0"
    :title="t('chat.homePanel.title')"
    :aria-label="t('chat.homePanel.title')"
    @click.stop="emit('selectHome')"
  >
    <AppIcon name="layout-grid" class="size-4 shrink-0 opacity-70" aria-hidden="true" />
  </button>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";
import { useI18n } from "vue-i18n";

const emit = defineEmits<{
  (e: "selectHome"): void;
}>();

const { t } = useI18n();
</script>
