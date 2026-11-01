<template>
  <CardShell
    layout="tile"
    tone="secondary"
    :icon="MessageSquareMore"
    :label="t('chat.sideChat.title')"
    interactive
    @select="emit('open')"
  />
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { MessageSquareMore } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();
</script>
