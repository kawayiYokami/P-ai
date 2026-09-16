<template>
  <ConfigCard :title="title" flush>
    <template #actions>
      <slot name="actions" />
      <SegmentedControl
        v-if="!readonly"
        :model-value="mode"
        :options="modeOptions"
        :full-width="false"
        size="sm"
        @change="onModeChange"
      />
    </template>

    <OverlayScrollArea
      v-if="displayMode === 'preview'"
      scroller-class="h-full bg-base-100 p-4 text-xs leading-relaxed"
      :style="{ height: props.height }"
    >
      <InlineMarkdownText v-if="hasContent" :text="props.modelValue" />
      <div v-else class="py-8 text-center text-xs opacity-50">{{ props.placeholder }}</div>
    </OverlayScrollArea>

    <textarea
      v-else
      ref="textareaRef"
      :value="props.modelValue"
      class="textarea w-full rounded-none border-0 bg-base-100 p-4 font-mono text-xs leading-relaxed select-text focus:outline-none resize-y"
      :placeholder="props.placeholder"
      :style="{ height: props.height }"
      @input="onInput"
    ></textarea>
  </ConfigCard>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import ConfigCard from "./ConfigCard.vue";
import SegmentedControl from "./SegmentedControl.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import InlineMarkdownText from "../../chat/markdown/InlineMarkdownText.vue";

type MarkdownEditorMode = "preview" | "edit";

const props = withDefaults(defineProps<{
  modelValue: string;
  title?: string;
  placeholder?: string;
  readonly?: boolean;
  height?: string;
  defaultMode?: MarkdownEditorMode;
}>(), {
  title: undefined,
  placeholder: "",
  readonly: false,
  height: "18rem",
  defaultMode: "preview",
});

const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
}>();

const { t } = useI18n();
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const mode = ref<MarkdownEditorMode>(props.defaultMode);

const displayMode = computed<MarkdownEditorMode>(() =>
  props.readonly ? "preview" : mode.value,
);

const hasContent = computed(() => String(props.modelValue || "").trim().length > 0);

const modeOptions = computed(() => [
  { value: "preview", label: t("common.preview") },
  { value: "edit", label: t("common.edit") },
]);

function onModeChange(value: string | number | boolean) {
  if (value === "preview" || value === "edit") {
    mode.value = value;
  }
}

function onInput(event: Event) {
  const target = event.target as HTMLTextAreaElement | null;
  emit("update:modelValue", target?.value ?? "");
}
</script>
