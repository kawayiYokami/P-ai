<template>
  <CardShell
    layout="tile"
    class="relative"
    :class="active ? 'ring-2 ring-primary/50' : ''"
    :tone="active ? 'primary' : 'info'"
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
  /** 是否当前打开的文件：主色描边 + 图标换成主色 */
  active?: boolean;
}>(), {
  path: "",
  active: false,
});

const emit = defineEmits<{
  (e: "open"): void;
  (e: "close"): void;
}>();

const { t } = useI18n();
</script>
