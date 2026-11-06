<template>
  <CardShell
    layout="tile"
    :col-span="colSpan"
    :row-span="rowSpan"
    tone="secondary"
    icon="message-square-more"
    :label="t('chat.sideChat.title')"
    interactive
    @select="emit('open')"
  />
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import CardShell from "./CardShell.vue";

withDefaults(defineProps<{
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
}>(), {
  colSpan: 1,
  rowSpan: 1,
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();
</script>
