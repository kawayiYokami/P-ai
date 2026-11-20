<template>
  <div class="space-y-2">
    <div v-for="(row, index) in rows" :key="index" class="flex items-center gap-2">
      <input
        :value="row.key"
        type="text"
        class="input input-sm input-bordered flex-1 font-mono text-xs"
        :placeholder="keyPlaceholder"
        :disabled="disabled"
        @input="patch(index, 'key', $event)"
      />
      <input
        :value="row.value"
        type="text"
        class="input input-sm input-bordered flex-1 font-mono text-xs"
        :placeholder="valuePlaceholder"
        :disabled="disabled"
        @input="patch(index, 'value', $event)"
      />
      <button
        type="button"
        class="btn btn-ghost btn-xs btn-square"
        :disabled="disabled"
        :title="t('common.delete')"
        @click="remove(index)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";

/** 键值行编辑：环境变量与请求头共用，只负责行内的改键 / 改值 / 删行，新增由父组件放按钮。 */

export type McpKeyValueRow = { key: string; value: string };

const props = withDefaults(defineProps<{
  rows: McpKeyValueRow[];
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  disabled?: boolean;
}>(), {
  keyPlaceholder: "KEY",
  valuePlaceholder: "VALUE",
  disabled: false,
});

const emit = defineEmits<{
  (e: "update", rows: McpKeyValueRow[]): void;
}>();

const { t } = useI18n();

function patch(index: number, field: "key" | "value", event: Event) {
  const value = (event.target as HTMLInputElement).value;
  emit(
    "update",
    props.rows.map((row, i) => (i === index ? { ...row, [field]: value } : row)),
  );
}

function remove(index: number) {
  emit(
    "update",
    props.rows.filter((_, i) => i !== index),
  );
}
</script>
