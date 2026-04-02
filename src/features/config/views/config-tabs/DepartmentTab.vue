<template>
  <div class="grid gap-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <div class="flex w-full flex-col gap-3">
          <div class="flex items-center justify-between"><span class="text-sm">{{ t("config.department.title") }}</span></div>
          <div class="flex gap-1">
            <select :value="selectedDepartmentId" class="select select-bordered select-sm flex-1" @change="switchSelectedDepartment(($event.target as HTMLSelectElement).value)">
              <option v-for="department in sortedDepartments" :key="department.id" :value="department.id">
                {{ department.name }}{{ department.isBuiltInAssistant ? `（${t("config.department.assistantBadge")}）` : (department.source === "private_workspace" ? `（${t("config.department.privateWorkspaceBadge")}）` : "") }}
              </option>
            </select>
            <button class="btn btn-sm btn-square bg-base-200" :title="t('config.department.add')" :disabled="savingConfig" @click="addDepartment">
              <Plus class="h-3.5 w-3.5" />
            </button>
            <button
              class="btn btn-sm btn-square"
              :class="!selectedDepartment || isNonRemovableDepartment(selectedDepartment) || selectedDepartmentIsPrivateWorkspace ? 'text-base-content/30 bg-base-200 cursor-not-allowed' : 'bg-base-200'"
              :title="t('config.department.remove')"
              :disabled="!selectedDepartment || isNonRemovableDepartment(selectedDepartment) || selectedDepartmentIsPrivateWorkspace || savingConfig"
              @click="removeSelectedDepartment"
            >
              <Trash2 class="h-3.5 w-3.5" />
            </button>
            <button
              class="btn btn-sm btn-square"
              :class="departmentDirty ? 'btn-primary' : 'bg-base-200'"
              :disabled="!selectedDepartment || selectedDepartmentIsPrivateWorkspace || !!departmentValidationMessage || !departmentDirty || savingConfig"
              :title="savingConfig ? t('config.api.saving') : departmentDirty ? t('common.save') : t('status.configSaved')"
              @click="saveDepartments"
            >
              <Save v-if="!savingConfig" class="h-3.5 w-3.5" />
              <span v-else class="loading loading-spinner loading-sm"></span>
            </button>
          </div>
          <div class="text-sm opacity-60">{{ t("config.department.hint") }}</div>
        </div>
      </div>
    </div>
    <div v-if="selectedDepartment" class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
        <div v-if="departmentValidationMessage" class="border-b border-warning/30 bg-warning/10 px-4 py-3 text-sm text-warning-content">
          {{ departmentValidationMessage }}
        </div>
        <div class="flex items-center justify-between gap-2 px-4 py-3 border-b border-base-300">
          <div class="flex items-center gap-2">
            <div class="font-medium">{{ selectedDepartment.name }}</div>
            <span v-if="selectedDepartmentIsPrivateWorkspace" class="badge badge-soft badge-secondary">{{ t("config.department.privateWorkspaceBadge") }}</span>
          </div>
          <button
            class="btn btn-sm btn-ghost"
            :disabled="isNonRemovableDepartment(selectedDepartment) || selectedDepartmentIsPrivateWorkspace || savingConfig"
            @click="removeSelectedDepartment"
          >
            {{ t("config.department.remove") }}
          </button>
        </div>

        <div class="divide-y divide-base-300">
          <!-- 名称 -->
          <div class="px-4 py-4">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.department.name") }}</div>
            <input
              v-model.trim="selectedDepartment.name"
              class="input input-bordered input-sm w-full"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.namePlaceholder')"
            />
            <div v-if="selectedDepartmentNameEmpty" class="text-xs text-error mt-2 opacity-80">
              {{ t("config.department.emptyName") }}
            </div>
            <div v-if="selectedDepartmentNameDuplicated" class="text-xs text-error mt-2 opacity-80">
              {{ t("config.department.duplicateName") }}
            </div>
          </div>

          <!-- 任命 -->
          <div class="px-4 py-4">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.department.assignee") }}</div>
            <select
              class="select select-bordered select-sm w-full"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :value="selectedDepartment.agentIds[0] || ''"
              @change="selectDepartmentAssignee(($event.target as HTMLSelectElement).value)"
            >
              <option value="">{{ t("config.department.assigneePlaceholder") }}</option>
              <option v-for="persona in personas" :key="persona.id" :value="persona.id">
                {{ persona.name }}
              </option>
            </select>
            <div
              v-if="selectedDepartment.isBuiltInAssistant && selectedDepartment.agentIds[0] === assistantDepartmentAgentId && selectedDepartment.agentIds[0]"
              class="text-xs opacity-50 mt-2"
            >
              {{ t("config.department.currentAssistant") }}
            </div>
          </div>

          <!-- 驱动模型 -->
          <div class="px-4 py-4">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.department.model") }}</div>
            <div class="grid gap-3">
              <div
                v-for="(apiId, idx) in selectedDepartmentApiConfigIds"
                :key="`${selectedDepartment.id}-api-${idx}`"
                class="flex items-center gap-2"
              >
                <select
                  class="select select-bordered select-sm flex-1"
                  :disabled="selectedDepartmentIsPrivateWorkspace"
                  :value="apiId"
                  @change="updateDepartmentApiConfigAt(idx, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t("config.memory.notConfigured") }}</option>
                  <option v-for="api in availableDepartmentApiConfigsForIndex(idx)" :key="api.id" :value="api.id">{{ api.name }}</option>
                </select>
                <div class="join">
                  <button class="btn btn-sm btn-square join-item opacity-60 hover:opacity-100" :disabled="selectedDepartmentIsPrivateWorkspace || idx <= 0" :title="t('config.department.moveUp')" @click="moveDepartmentApiConfig(idx, -1)">↑</button>
                  <button class="btn btn-sm btn-square join-item opacity-60 hover:opacity-100" :disabled="selectedDepartmentIsPrivateWorkspace || idx >= selectedDepartmentApiConfigIds.length - 1" :title="t('config.department.moveDown')" @click="moveDepartmentApiConfig(idx, 1)">↓</button>
                  <button class="btn btn-sm btn-square join-item opacity-60 hover:opacity-100" :disabled="selectedDepartmentIsPrivateWorkspace || selectedDepartmentApiConfigIds.length <= 1" :title="t('config.department.removeModel')" @click="removeDepartmentApiConfigAt(idx)">×</button>
                </div>
              </div>
              <button
                class="btn btn-sm"
                :disabled="selectedDepartmentIsPrivateWorkspace || remainingDepartmentApiConfigs.length <= 0"
                @click="addDepartmentApiConfig"
              >
                {{ t("config.department.addModel") }}
              </button>
            </div>
            <div class="text-[11px] opacity-50 mt-2">{{ t("config.department.modelFallbackHint") }}</div>
            <div class="text-[11px] opacity-40 mt-1">{{ t("config.department.allowedModelsNote") }}</div>
          </div>

          <!-- 概述 -->
          <div class="px-4 py-4">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.department.summary") }}</div>
            <textarea
              v-model="selectedDepartment.summary"
              class="textarea textarea-bordered textarea-sm w-full min-h-20"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.summaryPlaceholder')"
            />
          </div>

          <!-- 办事指南 -->
          <div class="px-4 py-4">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.department.guide") }}</div>
            <textarea
              v-model="selectedDepartment.guide"
              class="textarea textarea-bordered textarea-sm w-full min-h-28"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.guidePlaceholder')"
            />
            <div class="text-[11px] opacity-40 mt-2">{{ t("config.department.guideHint") }}</div>
          </div>
        </div>
      </div>
    <div v-else class="border border-base-300 rounded-box bg-base-100 p-12 text-center">
      <div class="text-sm opacity-40">{{ t("config.department.selectHint") }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Plus, Save, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, DepartmentConfig, PersonaProfile } from "../../../../types/app";
import { validateDepartmentConfig } from "../../utils/department-validation";

const props = defineProps<{
  config: AppConfig;
  apiConfigs: ApiConfigItem[];
  personas: PersonaProfile[];
  assistantDepartmentAgentId: string;
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  setStatusAction: (text: string) => void;
}>();

const emit = defineEmits<{
  (e: "update:assistantDepartmentAssigneeId", value: string): void;
}>();

const { t } = useI18n();
const selectedDepartmentId = ref("assistant-department");
const NON_REMOVABLE_DEPARTMENT_IDS = new Set(["assistant-department", "deputy-department", "front-desk-department"]);

function isNonRemovableDepartment(department: DepartmentConfig | null | undefined) {
  if (!department) return false;
  const id = String(department.id || "").trim();
  return NON_REMOVABLE_DEPARTMENT_IDS.has(id) || !!department.isBuiltInAssistant;
}

const sortedDepartments = computed(() =>
  [...(props.config.departments || [])].sort((a, b) => {
    const rank = (id: string) => id === "assistant-department" ? 0 : id === "deputy-department" ? 1 : id === "front-desk-department" ? 2 : 3;
    const aRank = rank(String(a.id || "").trim());
    const bRank = rank(String(b.id || "").trim());
    return aRank - bRank || a.orderIndex - b.orderIndex;
  }),
);

const selectedDepartment = computed(
  () => props.config.departments.find((item) => item.id === selectedDepartmentId.value) ?? sortedDepartments.value[0] ?? null,
);
const selectedDepartmentIsPrivateWorkspace = computed(
  () => selectedDepartment.value?.source === "private_workspace",
);
const textDepartmentApiConfigs = computed(() =>
  props.apiConfigs.filter((api) => !!api.enableText && ["openai", "openai_responses", "gemini", "anthropic"].includes(api.requestFormat)),
);
const selectedDepartmentApiConfigIds = computed(() =>
  currentDepartmentApiConfigIdsForEditor(selectedDepartment.value),
);
const remainingDepartmentApiConfigs = computed(() => {
  const selectedIds = new Set(selectedDepartmentApiConfigIds.value);
  return textDepartmentApiConfigs.value.filter((api) => !selectedIds.has(api.id));
});
const departmentNameCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const department of props.config.departments || []) {
    const key = String(department.name || "").trim().toLocaleLowerCase();
    if (!key) continue;
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  return counts;
});
const selectedDepartmentNameDuplicated = computed(() => {
  const key = String(selectedDepartment.value?.name || "").trim().toLocaleLowerCase();
  if (!key) return false;
  return (departmentNameCounts.value.get(key) || 0) > 1;
});
const selectedDepartmentNameEmpty = computed(() => !String(selectedDepartment.value?.name || "").trim());
const hasDuplicateDepartmentName = computed(() =>
  Array.from(departmentNameCounts.value.values()).some((count) => count > 1),
);
const hasEmptyDepartmentName = computed(() =>
  (props.config.departments || []).some((department) => !String(department.name || "").trim()),
);
const departmentValidationMessage = computed(() =>
  validateDepartmentConfig(props.config, props.apiConfigs, (key, params) => t(key, params ?? {})),
);
const departmentSnapshot = computed(() => JSON.stringify(
  (props.config.departments || []).map((item) => ({
    id: item.id,
    name: item.name,
    summary: item.summary,
    guide: item.guide,
    apiConfigId: item.apiConfigId,
    apiConfigIds: [...(item.apiConfigIds || [])],
    agentIds: [...(item.agentIds || [])],
    orderIndex: item.orderIndex,
  })),
));
const lastSavedDepartmentSnapshot = ref(departmentSnapshot.value);
const departmentDirty = computed(() => departmentSnapshot.value !== lastSavedDepartmentSnapshot.value);

watch(
  () => sortedDepartments.value.map((item) => item.id).join("|"),
  () => {
    if (!sortedDepartments.value.some((item) => item.id === selectedDepartmentId.value)) {
      selectedDepartmentId.value = sortedDepartments.value[0]?.id || "assistant-department";
    }
  },
  { immediate: true },
);

function syncAssistantDepartmentState() {
  const assistant = props.config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
  if (!assistant) return;
  const nextAssistantId = assistant.agentIds[0];
  if (nextAssistantId && nextAssistantId !== props.assistantDepartmentAgentId) {
    emit("update:assistantDepartmentAssigneeId", nextAssistantId);
  }
  const assistantPrimaryApiId = String(assistant.apiConfigIds?.[0] || assistant.apiConfigId || "").trim();
  if (props.config.assistantDepartmentApiConfigId !== assistantPrimaryApiId) {
    props.config.assistantDepartmentApiConfigId = assistantPrimaryApiId;
  }
}

function addDepartment() {
  const now = new Date().toISOString();
  const id = `department-${Date.now()}`;
  const name = nextDepartmentName();
  props.config.departments.push({
    id,
    name,
    summary: "",
    guide: "",
    apiConfigId: "",
    apiConfigIds: [],
    agentIds: [],
    createdAt: now,
    updatedAt: now,
    orderIndex: props.config.departments.length + 1,
    isBuiltInAssistant: false,
    source: "main_config",
    scope: "global",
  });
  selectedDepartmentId.value = id;
}

function nextDepartmentName() {
  const base = t("config.department.newName");
  let index = props.config.departments.filter((item) => !isNonRemovableDepartment(item)).length + 1;
  while (true) {
    const name = `${base} ${index}`;
    const exists = props.config.departments.some(
      (item) => String(item.name || "").trim().toLocaleLowerCase() === name.trim().toLocaleLowerCase(),
    );
    if (!exists) return name;
    index += 1;
  }
}

function removeSelectedDepartment() {
  const target = selectedDepartment.value;
  if (!target || isNonRemovableDepartment(target)) return;
  const idx = props.config.departments.findIndex((item) => item.id === target.id);
  if (idx >= 0) {
    props.config.departments.splice(idx, 1);
  }
}

function selectDepartmentAssignee(agentId: string) {
  const target = selectedDepartment.value;
  if (!target) return;
  const newAgentIds = agentId ? [agentId] : [];
  const currentAgentId = target.agentIds[0] || "";
  if (currentAgentId === (newAgentIds[0] || "")) return;
  target.agentIds = newAgentIds;
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}

function currentDepartmentApiConfigIds(target: DepartmentConfig | null | undefined) {
  if (!target) return [];
  const ids = Array.isArray(target.apiConfigIds) && target.apiConfigIds.length > 0
    ? target.apiConfigIds
    : [target.apiConfigId || ""];
  return ids.map((id) => String(id || "").trim()).filter(Boolean);
}

function currentDepartmentApiConfigIdsForEditor(target: DepartmentConfig | null | undefined) {
  const ids = currentDepartmentApiConfigIds(target);
  return ids.length > 0 ? Array.from(new Set(ids)) : [""];
}

function availableDepartmentApiConfigsForIndex(index: number) {
  const currentIds = currentDepartmentApiConfigIds(selectedDepartment.value);
  const currentId = currentIds[index];
  return textDepartmentApiConfigs.value.filter((api) => api.id === currentId || !currentIds.includes(api.id));
}

function updateDepartmentApiConfigAt(index: number, apiId: string) {
  const target = selectedDepartment.value;
  if (!target) return;
  const next = currentDepartmentApiConfigIds(target);
  const trimmedApiId = String(apiId || "").trim();
  if ((next[index] || "") === trimmedApiId) return;
  if (!trimmedApiId) {
    next.splice(index, 1);
  } else {
    next[index] = trimmedApiId;
  }
  target.apiConfigIds = Array.from(new Set(next.filter(Boolean)));
  target.apiConfigId = target.apiConfigIds[0] || "";
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}

function addDepartmentApiConfig() {
  const target = selectedDepartment.value;
  if (!target) return;
  const nextApi = remainingDepartmentApiConfigs.value[0];
  if (!nextApi) return;
  const next = currentDepartmentApiConfigIds(target);
  next.push(nextApi.id);
  target.apiConfigIds = next;
  target.apiConfigId = next[0] || "";
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}

function removeDepartmentApiConfigAt(index: number) {
  const target = selectedDepartment.value;
  if (!target) return;
  const next = currentDepartmentApiConfigIds(target);
  next.splice(index, 1);
  target.apiConfigIds = next;
  target.apiConfigId = target.apiConfigIds[0] || "";
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}

function moveDepartmentApiConfig(index: number, delta: number) {
  const target = selectedDepartment.value;
  if (!target) return;
  const next = currentDepartmentApiConfigIds(target);
  const swapIndex = index + delta;
  if (swapIndex < 0 || swapIndex >= next.length) return;
  const [item] = next.splice(index, 1);
  next.splice(swapIndex, 0, item);
  target.apiConfigIds = next;
  target.apiConfigId = next[0] || "";
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}

function switchSelectedDepartment(nextId: string) {
  const trimmedId = String(nextId || "").trim();
  if (!trimmedId || trimmedId === selectedDepartmentId.value) return;
  if (departmentDirty.value) {
    const currentName = String(selectedDepartment.value?.name || selectedDepartmentId.value || "").trim() || t("config.department.title");
    props.setStatusAction(t("status.departmentUnsavedSwitchHint", { name: currentName }));
  }
  selectedDepartmentId.value = trimmedId;
}

async function saveDepartments() {
  syncAssistantDepartmentState();
  if (departmentValidationMessage.value) return;
  const saved = await Promise.resolve(props.saveConfigAction());
  if (saved) {
    lastSavedDepartmentSnapshot.value = departmentSnapshot.value;
  }
}
</script>
