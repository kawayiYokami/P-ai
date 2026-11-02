<template>
  <CardShell
    layout="tile"
    class="relative"
    :icon="Files"
    :label="label"
    :title="path"
    interactive
    @select="emit('open')"
  >
    <button
      type="button"
      class="absolute right-2 top-2 grid size-5 place-items-center rounded-md text-base-content/35 transition-colors hover:bg-base-content/10 hover:text-base-content/80"
      :title="t('fileReader.close')"
      :aria-label="`${t('fileReader.close')} ${label}`"
      @click.stop="emit('close')"
      @keydown.stop
    >
      <X class="size-3.5" aria-hidden="true" />
    </button>
  </CardShell>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Files, X } from "@lucide/vue";
import CardShell from "./CardShell.vue";

withDefaults(defineProps<{
  /** 文件名，磁贴标题 */
  label: string;
  /** 完整路径，仅用于悬停提示 */
  path?: string;
}>(), {
  path: "",
});

const emit = defineEmits<{
  (e: "open"): void;
  (e: "close"): void;
}>();

const { t } = useI18n();
</script>
