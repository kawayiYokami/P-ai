<template>
  <CardShell
    layout="tile"
    tone="secondary"
    :icon="Users"
    :label="t('chat.teamManage.title')"
    interactive
    @select="emit('open')"
  >
    <span class="text-xs text-base-content/55">
      {{ summaryText }}
    </span>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Users } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const props = withDefaults(defineProps<{
  memberCount?: number;
  recruitEnabled?: boolean;
}>(), {
  memberCount: 0,
  recruitEnabled: false,
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();

const summaryText = computed(() => {
  const count = Math.max(0, Number(props.memberCount || 0));
  if (!props.recruitEnabled && count === 0) return String(t("chat.teamManage.recruitOff"));
  if (count === 0) return String(t("chat.teamManage.empty"));
  return String(t("chat.teamManage.memberCount", { count }));
});
</script>
