<template>
  <!--
    头像生成失败时弹出：告诉用户是快速模型不可用，并就地让他换一个。
    招募人格在聊天窗口弹、保存人格在配置窗口弹，卡片本身不关心宿主。
  -->
  <dialog class="modal" :class="{ 'modal-open': open }" :data-theme="theme || undefined">
    <div class="modal-box max-w-md">
      <h3 class="text-base font-semibold">{{ t("quickModelUnavailable.title") }}</h3>

      <p class="mt-2 text-sm text-base-content/70">{{ reasonText }}</p>

      <div class="mt-4">
        <div class="mb-1 text-caption text-base-content/60">{{ t("quickModelUnavailable.pickLabel") }}</div>
        <ApiConfigPicker
          class="w-full"
          :model-value="modelValue"
          :api-configs="apiConfigs"
          :placeholder="t('quickModelUnavailable.notConfigured')"
          @update:model-value="onSelect"
        />
      </div>

      <p class="mt-3 text-caption text-base-content/60">{{ t("quickModelUnavailable.disableHint") }}</p>

      <div class="modal-action">
        <button type="button" class="btn btn-sm btn-ghost" @click="emit('close')">
          {{ t("quickModelUnavailable.close") }}
        </button>
      </div>
    </div>
  </dialog>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem } from "../../../types/app";
import ApiConfigPicker from "./ApiConfigPicker.vue";
import { classifyQuickModelError } from "../../shared/utils/quick-model-error-hint";

const props = withDefaults(defineProps<{
  open: boolean;
  /** 后端原始错误，只用来提炼一句原因，不整段展示。 */
  error?: string;
  apiConfigs: ApiConfigItem[];
  modelValue?: string;
  /** 主题跟随窗口；不传时用 document 根主题。 */
  theme?: string;
}>(), {
  error: "",
  modelValue: "",
  theme: "",
});

const emit = defineEmits<{
  (event: "select", modelId: string): void;
  (event: "close"): void;
}>();

const { t } = useI18n();

const reasonText = computed(() => {
  const hint = classifyQuickModelError(props.error);
  switch (hint.kind) {
    case "server":
      return hint.status
        ? t("quickModelUnavailable.reason.serverWithCode", { code: hint.status })
        : t("quickModelUnavailable.reason.server");
    case "auth":
      return t("quickModelUnavailable.reason.auth");
    case "timeout":
      return t("quickModelUnavailable.reason.timeout");
    case "notFound":
      return t("quickModelUnavailable.reason.notFound");
    default:
      return hint.detail
        ? t("quickModelUnavailable.reason.unknownWithDetail", { detail: hint.detail })
        : t("quickModelUnavailable.reason.unknown");
  }
});

function onSelect(modelId: string) {
  const next = String(modelId || "").trim();
  // 选到「不配置」等于关掉快速请求，不该被当成一次重试。
  if (!next) return;
  emit("select", next);
}
</script>
