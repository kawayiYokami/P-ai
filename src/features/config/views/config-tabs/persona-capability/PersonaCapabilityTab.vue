<template>
  <!-- 1. 委托模型：任务怎么分派给下一个人格 -->
  <div v-if="showAgentOnlySettings" class="space-y-2">
    <ConfigCard :title="t('config.persona.delegateModelSettings')" flush>
      <div class="divide-y divide-base-200/60">
        <!-- 三选一 -->
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-3.5 sm:p-4 min-w-0">
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium text-base-content">{{ t("config.persona.delegateModelTitle") }}</div>
            <div class="mt-0.5 text-xs text-base-content/50 leading-relaxed">{{ t("config.persona.delegateModelHint") }}</div>
          </div>
          <SegmentedControl
            :model-value="delegateModelSelection"
            :options="delegateModelOptions"
            :disabled="saving"
            :full-width="false"
            size="sm"
            class="shrink-0 self-start sm:self-auto"
            @change="setDelegateModelOption"
          />
        </div>

        <!-- 自定义模型选择 -->
        <div v-if="delegateModelSelection === 'custom'" class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-3.5 sm:p-4 min-w-0">
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium text-base-content">{{ t("config.persona.delegateCustomModel") }}</div>
            <div class="mt-0.5 text-xs text-base-content/50 leading-relaxed">{{ t("config.persona.delegateCustomModelHint") }}</div>
          </div>
          <ApiConfigPicker
            class="min-w-0 sm:w-72 shrink-0"
            variant="field"
            :model-value="delegateCustomModelId"
            :api-configs="apiConfigs"
            :disabled="saving"
            @update:model-value="setDelegateCustomModel"
          />
        </div>
      </div>
    </ConfigCard>
  </div>

  <!-- 2. 随身技能与上下文注入顺序 -->
  <PersonaInjectionTable
    :persona="persona"
    :skills="skills"
  />

  <!-- 3. 私有记忆：存在哪、怎么回忆、怎么搬走 -->
  <div v-if="showAgentOnlySettings" class="space-y-2">
    <ConfigCard :title="t('config.persona.memorySettings')" flush>
      <div class="divide-y divide-base-200/60">
        <!-- 私有记忆开关 -->
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-3.5 sm:p-4 min-w-0">
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium text-base-content">{{ t("config.persona.privateMemory") }}</div>
            <div class="mt-0.5 text-xs text-base-content/50 leading-relaxed">{{ t("config.persona.privateMemoryHint") }}</div>
          </div>
          <SegmentedControl
            :model-value="!!persona.privateMemoryEnabled"
            :options="privateMemoryModeOptions"
            :disabled="privateMemoryCounting || privateMemorySwitching"
            :full-width="false"
            size="sm"
            class="shrink-0 self-start sm:self-auto"
            @change="setPrivateMemoryMode"
          />
        </div>

        <!-- 回忆方式 -->
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-3.5 sm:p-4 min-w-0">
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium text-base-content">{{ t("config.persona.memoryRecallMode") }}</div>
            <div class="mt-0.5 text-xs text-base-content/50 leading-relaxed">{{ memoryRecallModeHint }}</div>
          </div>
          <SegmentedControl
            :model-value="selectedPersonaMemoryRecallMode"
            :options="memoryRecallModeOptions"
            :disabled="memoryRecallModeSwitching"
            :full-width="false"
            size="sm"
            class="shrink-0 self-start sm:self-auto"
            @change="setMemoryRecallMode"
          />
        </div>

        <!-- 导入私有记忆 -->
        <div class="flex items-center justify-between gap-3 p-3.5 sm:p-4">
          <div class="min-w-0">
            <div class="text-sm font-medium text-base-content">{{ t("config.persona.import") }}</div>
          </div>
          <button
            type="button"
            class="btn btn-sm min-h-[2rem] btn-ghost gap-1.5"
            @click="triggerPersonaMemoryImport"
          >
            <AppIcon name="upload" class="h-4 w-4" />
            <span>{{ t("config.persona.import") }}</span>
          </button>
        </div>
      </div>
    </ConfigCard>

    <div v-if="privateMemoryError" class="text-xs text-error px-1">
      {{ privateMemoryError }}
    </div>
  </div>

  <input
    ref="personaMemoryImportInput"
    type="file"
    accept=".json,application/json"
    class="hidden"
    @change="onPersonaMemoryImportFile"
  />

  <!-- 关闭私有记忆前的导出确认 -->
  <dialog ref="privateMemoryDialog" class="modal">
    <div class="modal-box max-w-md">
      <h3 class="text-sm font-semibold mb-2">{{ t('config.persona.closePrivateMemoryConfirm') }}</h3>
      <div v-if="privateMemoryCounting" class="flex items-center gap-2 text-sm">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{{ t('config.persona.countingMemory') }}</span>
      </div>
      <div v-else class="text-sm whitespace-pre-wrap leading-relaxed">{{ privateMemoryDialogMessage }}</div>
      <div v-if="!privateMemoryCounting && privateMemoryCount > 0" class="mt-3 rounded-box border border-warning/40 bg-warning/10 p-2 text-sm">
        <div class="font-medium">{{ t('config.persona.mustExportFirst') }}</div>
        <div class="opacity-70 mt-1">{{ t('config.persona.exportedConfirmUnlock') }}</div>
      </div>
      <div v-if="!privateMemoryCounting && privateMemoryCount > 0" class="mt-3">
        <button
          class="btn btn-sm min-h-[2.25rem] btn-warning"
          :disabled="privateMemoryExporting || privateMemoryExported"
          @click="exportPrivateMemoriesBeforeDisable"
        >
          {{ privateMemoryExported ? t('config.persona.exported') : (privateMemoryExporting ? t('config.persona.exporting') : t('config.persona.exportPrivateMemory')) }}
        </button>
      </div>
      <div class="modal-action">
        <button class="btn btn-sm min-h-[2.25rem]" :disabled="privateMemoryCounting || privateMemoryExporting || privateMemorySwitching" @click="cancelDisablePrivateMemory">{{ t('common.cancel') }}</button>
        <button
          class="btn btn-sm min-h-[2.25rem] btn-primary"
          :disabled="privateMemoryCounting || privateMemoryExporting || privateMemorySwitching || (privateMemoryCount > 0 && !privateMemoryExported)"
          @click="confirmDisablePrivateMemory"
        >
          {{ t('common.confirm') }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="cancelDisablePrivateMemory">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../../shared/components/AppIcon.vue";
import type { ApiConfigItem, MemoryRecallMode, PersonaProfile, SkillSummaryItem } from "../../../../../types/app";
import {
  exportTransportAgentPrivateMemories,
  invokeTauri,
} from "../../../../../services/tauri-api";
import ApiConfigPicker from "../../../components/ApiConfigPicker.vue";
import ConfigCard from "../../../components/ConfigCard.vue";
import SegmentedControl from "../../../components/SegmentedControl.vue";
import PersonaInjectionTable from "./PersonaInjectionTable.vue";

/**
 * 能力页：这个智能体拿什么模型干活、带什么准则与技能、记得什么。
 *
 * 改的都是 `persona` 这个对象本身（与父组件共用同一引用），
 * 所以父组件的 `personaDirty` 照旧能感知到，不需要额外的变更上报。
 */
const props = withDefaults(defineProps<{
  persona: PersonaProfile;
  skills?: SkillSummaryItem[];
  apiConfigs?: ApiConfigItem[];
  saving?: boolean;
}>(), {
  skills: () => [],
  apiConfigs: () => [],
  saving: false,
});

const emit = defineEmits<{
  (e: "importPersonaMemories", value: { agentId: string; file: File }): void;
}>();

const { t } = useI18n();

// 用户人格与系统人格没有「委托给谁」「记不记私有记忆」这些问题。
const showAgentOnlySettings = computed(() => !props.persona.isBuiltInUser && !props.persona.isBuiltInSystem);

// ==================== 委托模型配置 ====================
// 存储映射：跟随会话 = ["role:session"]，快速 = ["role:quick"]，自定义 = [具体 endpoint]
// 专家模型已改为隐形兜底，不再作为委托模型的可选项；存量 role:expert 按「跟随会话」解释。
const DELEGATE_MODEL_ROLE_SESSION = "role:session";
const DELEGATE_MODEL_ROLE_EXPERT = "role:expert";
const DELEGATE_MODEL_ROLE_QUICK = "role:quick";

type DelegateModelOption = "follow_session" | "quick" | "custom";

const delegateModelOptions = computed(() => [
  { value: "follow_session" as DelegateModelOption, label: t("config.persona.delegateModelFollowSession") },
  { value: "quick" as DelegateModelOption, label: t("config.persona.delegateModelQuick") },
  { value: "custom" as DelegateModelOption, label: t("config.persona.delegateModelCustom") },
]);

function delegateModelRawId(): string {
  const ids = Array.isArray(props.persona?.apiConfigIds) ? props.persona.apiConfigIds : [];
  return String(ids[0] || "").trim();
}

function delegateModelOptionFromStorage(): DelegateModelOption {
  const rawId = delegateModelRawId();
  // 未配置时默认跟随会话
  if (!rawId) return "follow_session";
  if (rawId === DELEGATE_MODEL_ROLE_SESSION) return "follow_session";
  // 存量 role:expert 与后端语义一致地按跟随会话展示，下次保存即固化
  if (rawId === DELEGATE_MODEL_ROLE_EXPERT) return "follow_session";
  if (rawId === DELEGATE_MODEL_ROLE_QUICK) return "quick";
  return "custom";
}

// UI 当前选中项：与存储解耦，避免「点自定义无模型时被存储值顶回」
const delegateModelSelection = ref<DelegateModelOption>(delegateModelOptionFromStorage());

watch(
  () => props.persona?.id,
  () => {
    delegateModelSelection.value = delegateModelOptionFromStorage();
  },
);

const delegateCustomModelId = computed(() => {
  const rawId = delegateModelRawId();
  if (!rawId || rawId.startsWith("role:")) return "";
  return rawId;
});

function writeDelegateModel(rawId: string) {
  const persona = props.persona;
  if (!persona) return;
  persona.apiConfigIds = rawId ? [rawId] : [];
}

function setDelegateModelOption(option: DelegateModelOption) {
  delegateModelSelection.value = option;
  switch (option) {
    case "follow_session":
      // 跟随会话：显式写 role:session 哨兵，与存储映射注释保持一致
      writeDelegateModel(DELEGATE_MODEL_ROLE_SESSION);
      break;
    case "quick":
      writeDelegateModel(DELEGATE_MODEL_ROLE_QUICK);
      break;
    case "custom": {
      // 进入自定义：保留已有自定义模型；没有则只切 UI 选项，不写回专家
      const current = delegateCustomModelId.value;
      if (current) {
        writeDelegateModel(current);
      }
      break;
    }
  }
}

function setDelegateCustomModel(modelId: string) {
  const trimmed = String(modelId || "").trim();
  if (trimmed) {
    writeDelegateModel(trimmed);
  }
}

// ==================== 私有记忆 ====================
const personaMemoryImportInput = ref<HTMLInputElement | null>(null);
const privateMemoryDialog = ref<HTMLDialogElement | null>(null);
const privateMemoryCounting = ref(false);
const privateMemorySwitching = ref(false);
const memoryRecallModeSwitching = ref(false);
const privateMemoryExporting = ref(false);
const privateMemoryDialogMessage = ref("");
const privateMemoryError = ref("");
const privateMemoryCount = ref(0);
const privateMemoryExported = ref(false);
const pendingDisableAgentId = ref("");

const privateMemoryModeOptions = computed(() => [
  { value: false, label: t("config.persona.global") },
  { value: true, label: t("config.persona.private") },
]);

const memoryRecallModeOptions = computed(() => [
  { value: "auto" as MemoryRecallMode, label: t("config.persona.memoryRecallAuto") },
  { value: "manual" as MemoryRecallMode, label: t("config.persona.memoryRecallManual") },
  { value: "off" as MemoryRecallMode, label: t("config.persona.memoryRecallOff") },
]);

const selectedPersonaMemoryRecallMode = computed(() =>
  normalizeMemoryRecallMode(props.persona?.memoryRecallMode),
);

const memoryRecallModeHint = computed(() => {
  if (selectedPersonaMemoryRecallMode.value === "manual") {
    return t("config.persona.memoryRecallModeHintManual");
  }
  if (selectedPersonaMemoryRecallMode.value === "off") {
    return t("config.persona.memoryRecallModeHintOff");
  }
  return t("config.persona.memoryRecallModeHintAuto");
});

function normalizeMemoryRecallMode(value: unknown): MemoryRecallMode {
  const raw = String(value || "").trim();
  if (raw === "manual" || raw === "off") return raw;
  return "auto";
}

function triggerPersonaMemoryImport() {
  if (!personaMemoryImportInput.value) return;
  personaMemoryImportInput.value.value = "";
  personaMemoryImportInput.value.click();
}

function onPersonaMemoryImportFile(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  const agentId = props.persona?.id;
  if (!agentId) return;
  emit("importPersonaMemories", { agentId, file });
}

async function setPrivateMemoryMode(enabled: boolean) {
  const agentId = props.persona?.id;
  if (!agentId) return;
  const current = !!props.persona?.privateMemoryEnabled;
  if (current === enabled) return;
  privateMemoryError.value = "";
  if (enabled) {
    privateMemorySwitching.value = true;
    try {
      await invokeTauri("set_agent_private_memory_enabled", {
        input: { agentId, enabled: true },
      });
      props.persona.privateMemoryEnabled = true;
    } catch (error) {
      privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
    } finally {
      privateMemorySwitching.value = false;
    }
    return;
  }
  pendingDisableAgentId.value = agentId;
  privateMemoryDialogMessage.value = "";
  privateMemoryCount.value = 0;
  privateMemoryExported.value = false;
  privateMemoryCounting.value = true;
  privateMemoryDialog.value?.showModal();
  try {
    const result = await invokeTauri<{ count: number }>("get_agent_private_memory_count", {
      input: { agentId },
    });
    const count = Math.max(0, Number(result.count || 0));
    privateMemoryCount.value = count;
    privateMemoryDialogMessage.value = count <= 0
      ? t('config.persona.noPrivateMemorySafe')
      : `${t('config.persona.hasPrivateMemory', { count: count.toLocaleString() })}\n\n${t('config.persona.mustExportFirstHint')}`;
  } catch {
    privateMemoryCount.value = 0;
    privateMemoryDialogMessage.value = t('config.persona.countFailedButCanClose');
  } finally {
    privateMemoryCounting.value = false;
  }
}

async function setMemoryRecallMode(mode: MemoryRecallMode) {
  const agentId = props.persona?.id;
  if (!agentId) return;
  const nextMode = normalizeMemoryRecallMode(mode);
  const current = normalizeMemoryRecallMode(props.persona?.memoryRecallMode);
  if (current === nextMode) return;
  privateMemoryError.value = "";
  memoryRecallModeSwitching.value = true;
  try {
    const result = await invokeTauri<{ agentId: string; mode: MemoryRecallMode }>("set_agent_memory_recall_mode", {
      input: { agentId, mode: nextMode },
    });
    props.persona.memoryRecallMode = normalizeMemoryRecallMode(result.mode);
  } catch (error) {
    privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
  } finally {
    memoryRecallModeSwitching.value = false;
  }
}

function cancelDisablePrivateMemory() {
  pendingDisableAgentId.value = "";
  privateMemoryCount.value = 0;
  privateMemoryExported.value = false;
  privateMemoryExporting.value = false;
  privateMemoryDialog.value?.close();
}

async function exportPrivateMemoriesBeforeDisable() {
  const agentId = pendingDisableAgentId.value;
  if (!agentId || privateMemoryCount.value <= 0) return;
  privateMemoryError.value = "";
  privateMemoryExporting.value = true;
  try {
    const result = await exportTransportAgentPrivateMemories<{ count: number; path: string }>({ agentId });
    privateMemoryExported.value = true;
    privateMemoryDialogMessage.value = `${t('config.persona.exportSuccess', { count: result.count.toLocaleString() })}\n${t('config.persona.exportSuccessPathHint', { path: result.path })}`;
  } catch (error) {
    privateMemoryExported.value = false;
    privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
  } finally {
    privateMemoryExporting.value = false;
  }
}

async function confirmDisablePrivateMemory() {
  const agentId = pendingDisableAgentId.value;
  if (!agentId) {
    privateMemoryDialog.value?.close();
    return;
  }
  privateMemoryError.value = "";
  privateMemorySwitching.value = true;
  try {
    await invokeTauri("disable_agent_private_memory", {
      input: { agentId },
    });
    const persona = props.persona;
    if (persona && !persona.isBuiltInUser && !persona.isBuiltInSystem) {
      persona.privateMemoryEnabled = false;
    }
    pendingDisableAgentId.value = "";
    privateMemoryCount.value = 0;
    privateMemoryExported.value = false;
    privateMemoryDialog.value?.close();
  } catch (error) {
    privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
  } finally {
    privateMemorySwitching.value = false;
  }
}
</script>
