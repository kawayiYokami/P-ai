<template>
  <ConfigCard :title="title">
    <div class="grid gap-2 py-3">
      <div v-for="(apiKey, index) in props.modelValue" :key="`api-key-${index}`" class="flex items-center gap-2">
          <button
            v-if="statusForKey(apiKey)"
            type="button"
            class="cursor-pointer shrink-0 leading-none"
            :title="t('config.api.testConnectionReReportTitle')"
            @click="emit('click-status', apiKey)"
          >
            <span v-if="statusForKey(apiKey)?.status === 'success'" class="status status-success"></span>
            <span v-else class="status status-error"></span>
          </button>
          <span v-else class="w-4 shrink-0"></span>
          <input
            :value="apiKey"
            :type="visibleKeys[index] ? 'text' : 'password'"
            class="input input-bordered input-sm flex-1"
            :placeholder="`API Key #${index + 1}`"
            @input="updateKey(index, ($event.target as HTMLInputElement).value)"
          />
          <button
            class="btn btn-sm btn-square bg-base-200"
            type="button"
            :disabled="index === 0"
            :title="t('config.api.pinApiKeyToTop')"
            @click="pinKey(index)"
          >
            <AppIcon name="arrow-up-to-line" class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-square bg-base-200"
            type="button"
            @click="toggleKeyVisibility(index)"
          >
            <AppIcon name="eye-off" v-if="visibleKeys[index]" class="h-3.5 w-3.5" />
            <AppIcon name="eye" v-else class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-square bg-base-200 text-error"
            type="button"
            :disabled="props.modelValue.length <= 1"
            :title="t('common.delete')"
            @click="removeKey(index)"
          >
            <AppIcon name="trash" class="h-3.5 w-3.5" />
          </button>
        </div>
        <div v-if="props.modelValue.length === 0" class="rounded-box border border-dashed border-base-300 px-3 py-3 text-sm opacity-60">
          {{ t("config.api.noApiKey") }}
        </div>
        <button class="btn btn-sm w-full" type="button" @click="addKey">
          <AppIcon name="plus" class="h-3.5 w-3.5" />
          <span>{{ t("config.api.addApiKey") }}</span>
        </button>
      </div>
  </ConfigCard>
</template>

<script setup lang="ts">
import { ref } from "vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import { useI18n } from "vue-i18n";
import ConfigCard from "./ConfigCard.vue";

export type ApiKeyConnectionStatus = {
  status: "success" | "failed";
  latencyMs?: number;
  error?: string;
};

const props = withDefaults(defineProps<{
  title: string;
  modelValue: string[];
  connectionTestKeyStatus?: Record<string, ApiKeyConnectionStatus>;
}>(), {
  connectionTestKeyStatus: undefined,
});

const emit = defineEmits<{
  "update:modelValue": [value: string[]];
  "click-status": [apiKey: string];
}>();

const { t } = useI18n();
const visibleKeys = ref<Record<number, boolean>>({});

function statusForKey(apiKey: string): ApiKeyConnectionStatus | undefined {
  const key = apiKey.trim();
  return key ? props.connectionTestKeyStatus?.[key] : undefined;
}

function updateKey(index: number, value: string) {
  const next = [...props.modelValue];
  next[index] = value;
  emit("update:modelValue", next);
}

function addKey() {
  emit("update:modelValue", [...props.modelValue, ""]);
}

function removeKey(index: number) {
  if (props.modelValue.length <= 1) return;
  const next = props.modelValue.filter((_, currentIndex) => currentIndex !== index);
  const nextVisibleKeys: Record<number, boolean> = {};
  Object.entries(visibleKeys.value).forEach(([currentIndex, visible]) => {
    const numericIndex = Number(currentIndex);
    if (numericIndex === index) return;
    nextVisibleKeys[numericIndex > index ? numericIndex - 1 : numericIndex] = visible;
  });
  visibleKeys.value = nextVisibleKeys;
  emit("update:modelValue", next);
}

function pinKey(index: number) {
  if (index <= 0 || index >= props.modelValue.length) return;
  const next = [...props.modelValue];
  const [key] = next.splice(index, 1);
  next.unshift(key);
  const nextVisibleKeys: Record<number, boolean> = {};
  Object.entries(visibleKeys.value).forEach(([currentIndex, visible]) => {
    const numericIndex = Number(currentIndex);
    const nextIndex = numericIndex === index ? 0 : numericIndex < index ? numericIndex + 1 : numericIndex;
    nextVisibleKeys[nextIndex] = visible;
  });
  visibleKeys.value = nextVisibleKeys;
  emit("update:modelValue", next);
}

function toggleKeyVisibility(index: number) {
  visibleKeys.value = {
    ...visibleKeys.value,
    [index]: !visibleKeys.value[index],
  };
}
</script>
