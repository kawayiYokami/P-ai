<template>
  <ConfigCard :title="title">
    <div class="grid gap-2 py-3">
      <div class="flex items-center justify-between gap-2">
        <span class="text-xs opacity-60">{{ t("config.api.apiKeyTextareaHint") }}</span>
        <button
          class="btn btn-sm btn-square btn-ghost"
          type="button"
          :title="visible ? t('config.api.hideApiKeys') : t('config.api.showApiKeys')"
          @click="toggleVisibility"
        >
          <AppIcon :name="visible ? 'eye-off' : 'eye'" class="h-3.5 w-3.5" />
        </button>
      </div>
      <textarea
        :value="displayText"
        class="textarea textarea-bordered min-h-32 w-full resize-y font-mono text-sm leading-6"
        :class="{ 'api-key-masked': masked }"
        :placeholder="t('config.api.apiKeyTextareaPlaceholder')"
        :readonly="masked && !supportsTextSecurity"
        spellcheck="false"
        autocomplete="off"
        autocapitalize="off"
        rows="6"
        @input="onInput"
        @blur="normalizeText"
      ></textarea>
      <div v-if="statusEntries.length" class="flex flex-wrap items-center gap-1.5">
        <button
          v-for="entry in statusEntries"
          :key="entry.key"
          type="button"
          class="cursor-pointer leading-none"
          :title="t('config.api.testConnectionReReportTitle')"
          @click="emit('click-status', entry.key)"
        >
          <span v-if="entry.status.status === 'success'" class="status status-success"></span>
          <span v-else class="status status-error"></span>
        </button>
      </div>
      <div v-if="props.modelValue.length === 0" class="rounded-box border border-dashed border-base-300 px-3 py-3 text-sm opacity-60">
        {{ t("config.api.noApiKey") }}
      </div>
    </div>
  </ConfigCard>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
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

// textarea 没有 password 类型，用 CSS 把文本画成圆点；不支持该 CSS 的浏览器退回只读掩码视图。
const supportsTextSecurity = typeof CSS !== "undefined"
  && typeof CSS.supports === "function"
  && CSS.supports("-webkit-text-security", "disc");

const visible = ref(false);
const masked = computed(() => !visible.value);
const draftText = ref(props.modelValue.join("\n"));

function splitKeys(text: string): string[] {
  const seen = new Set<string>();
  const keys: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const key = line.trim();
    if (!key || seen.has(key)) continue;
    seen.add(key);
    keys.push(key);
  }
  return keys;
}

function sameKeys(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((key, index) => key === right[index]);
}

function commitText() {
  const next = splitKeys(draftText.value);
  if (!sameKeys(next, props.modelValue)) emit("update:modelValue", next);
}

function onInput(event: Event) {
  draftText.value = (event.target as HTMLTextAreaElement).value;
  commitText();
}

// 失焦时把编辑区本身也规范成一行一个，避免界面留着空行和重复行、保存后却对不上。
function normalizeText() {
  const normalized = splitKeys(draftText.value).join("\n");
  if (draftText.value !== normalized) draftText.value = normalized;
  commitText();
}

function toggleVisibility() {
  visible.value = !visible.value;
}

const displayText = computed(() => {
  if (!masked.value || supportsTextSecurity || !draftText.value) return draftText.value;
  return splitKeys(draftText.value).map((key) => "•".repeat(Math.min(key.length, 40))).join("\n");
});

const statusEntries = computed(() => {
  const statuses = props.connectionTestKeyStatus;
  if (!statuses) return [];
  return splitKeys(draftText.value)
    .map((key) => ({ key, status: statuses[key] }))
    .filter((entry): entry is { key: string; status: ApiKeyConnectionStatus } => !!entry.status);
});

watch(() => props.modelValue, (value) => {
  if (!sameKeys(splitKeys(draftText.value), value)) draftText.value = value.join("\n");
});
</script>

<style scoped>
.api-key-masked {
  -webkit-text-security: disc;
}
</style>
