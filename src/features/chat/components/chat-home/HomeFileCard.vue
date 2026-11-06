<template>
  <CardShell
    layout="tile"
    class="relative"
    :col-span="colSpan"
    :row-span="rowSpan"
    icon="files"
    :icon-src="iconSrc"
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
      <AppIcon name="close" class="size-3.5" aria-hidden="true" />
    </button>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import CardShell from "./CardShell.vue";
import { resolveFileTreeIcon } from "../../../file-reader/file-tree-icons";

const props = withDefaults(defineProps<{
  /** 文件名，磁贴标题 */
  label: string;
  /** 完整路径，仅用于悬停提示 */
  path?: string;
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
}>(), {
  path: "",
  colSpan: 1,
  rowSpan: 1,
});

/** 按扩展名取 VS Code 同款文件图标；取不到时留空，由 CardShell 回落到 Files 组件图标 */
const iconSrc = computed(() => (props.path ? resolveFileTreeIcon(props.path, false) : ""));

const emit = defineEmits<{
  (e: "open"): void;
  (e: "close"): void;
}>();

const { t } = useI18n();
</script>
