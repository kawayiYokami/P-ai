<template>
  <div class="flex flex-col gap-6 [&_div]:[transition:background-color_200ms,border-color_200ms,box-shadow_200ms,border-radius_200ms_ease-out]">
    <!-- 操作栏 -->
    <div class="flex items-center justify-between">
      <div class="text-sm opacity-60">{{ t("config.department.hint") }}</div>
      <div class="flex items-center gap-2">
        <button class="btn btn-sm btn-ghost" :disabled="savingConfig" @click="addDepartment">{{ t("config.department.add") }}</button>
      </div>
    </div>

    <div class="grid gap-6 lg:grid-cols-[280px_minmax(0,1fr)]">
      <!-- 部门列表 -->
      <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
        <div class="flex items-center gap-2 px-4 py-3 border-b border-base-300">
          <div class="font-medium">{{ t("config.department.title") }}<span v-if="sortedDepartments.length" class="opacity-60">（{{ sortedDepartments.length }}）</span></div>
        </div>
        <div class="divide-y divide-base-300">
          <button
            v-for="department in pagedDepartments"
            :key="department.id"
            class="w-full text-left px-4 py-3 hover:bg-base-200/40 transition-colors"
            :class="selectedDepartmentId === department.id ? 'bg-base-200/60' : ''"
            @click="selectedDepartmentId = department.id"
          >
            <div class="flex items-center gap-2">
              <div class="font-medium text-sm">{{ department.name }}</div>
              <span v-if="department.isBuiltInAssistant" class="badge badge-soft badge-primary">{{ t("config.department.assistantBadge") }}</span>
              <span v-else-if="department.source === 'private_workspace'" class="badge badge-soft badge-secondary">{{ t("config.department.privateWorkspaceBadge") }}</span>
            </div>
            <div class="text-[11px] opacity-50 mt-1 line-clamp-2">
              {{ department.summary || t("config.department.emptySummary") }}
            </div>
          </button>
        </div>
        <!-- 分页 -->
        <div v-if="totalPages > 1" class="flex justify-center border-t border-base-300 px-4 py-3">
          <div class="join">
            <button class="btn btn-xs join-item" :disabled="page <= 1" @click="page--">‹</button>
            <button class="btn btn-xs join-item btn-active">{{ page }} / {{ totalPages }}</button>
            <button class="btn btn-xs join-item" :disabled="page >= totalPages" @click="page++">›</button>
          </div>
        </div>
      </div>

      <!-- 部门详情 -->
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
            :disabled="!!selectedDepartment.isBuiltInAssistant || selectedDepartmentIsPrivateWorkspace || savingConfig"
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
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, DepartmentConfig, PersonaProfile } from "../../../../types/app";
import { validateDepartmentConfig } from "../../utils/department-validation";

const PAGE_SIZE = 5;

const props = defineProps<{
  config: AppConfig;
  apiConfigs: ApiConfigItem[];
  personas: PersonaProfile[];
  assistantDepartmentAgentId: string;
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
}>();

const emit = defineEmits<{
  (e: "update:assistantDepartmentAssigneeId", value: string): void;
}>();

const { t } = useI18n();
const selectedDepartmentId = ref("assistant-department");
const page = ref(1);

const sortedDepartments = computed(() =>
  [...(props.config.departments || [])].sort((a, b) => {
    const aRank = a.isBuiltInAssistant || a.id === "assistant-department" ? 0 : 1;
    const bRank = b.isBuiltInAssistant || b.id === "assistant-department" ? 0 : 1;
    return aRank - bRank || a.orderIndex - b.orderIndex;
  }),
);

const totalPages = computed(() => Math.max(1, Math.ceil(sortedDepartments.value.length / PAGE_SIZE)));
const pagedDepartments = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return sortedDepartments.value.slice(start, start + PAGE_SIZE);
});

const selectedDepartment = computed(
  () => props.config.departments.find((item) => item.id === selectedDepartmentId.value) ?? sortedDepartments.value[0] ?? null,
);
const selectedDepartmentIsPrivateWorkspace = computed(
  () => selectedDepartment.value?.source === "private_workspace",
);
const textDepartmentApiConfigs = computed(() =>
  props.apiConfigs.filter((api) => !!api.enableText && ["openai", "openai_responses", "gemini", "deepseek/kimi", "anthropic"].includes(api.requestFormat)),
);
const selectedDepartmentApiConfigIds = computed(() =>
  Array.from(new Set(
    (selectedDepartment.value?.apiConfigIds?.length
      ? selectedDepartment.value.apiConfigIds
      : [selectedDepartment.value?.apiConfigId || ""])
      .map((id) => String(id || "").trim())
      .filter(Boolean),
  )),
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
  validateDepartmentConfig(props.config, props.apiConfigs, (key, params) => t(key, params)),
);
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
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
let departmentAutosaveReady = false;

watch(
  () => sortedDepartments.value.map((item) => item.id).join("|"),
  () => {
    if (!sortedDepartments.value.some((item) => item.id === selectedDepartmentId.value)) {
      selectedDepartmentId.value = sortedDepartments.value[0]?.id || "assistant-department";
    }
  },
  { immediate: true },
);

// 分页变化时检查选中项是否还在当前页
watch(page, () => {
  const ids = pagedDepartments.value.map((d) => d.id);
  if (!ids.includes(selectedDepartmentId.value) && pagedDepartments.value.length > 0) {
    selectedDepartmentId.value = pagedDepartments.value[0].id;
  }
});

function syncAssistantDepartmentState() {
  const assistant = props.config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
  if (!assistant) return;
  const nextAssistantId = assistant.agentIds[0];
  if (nextAssistantId && nextAssistantId !== props.assistantDepartmentAgentId) {
    emit("update:assistantDepartmentAssigneeId", nextAssistantId);
  }
  const assistantPrimaryApiId = String(assistant.apiConfigIds?.[0] || assistant.apiConfigId || "").trim();
  if (assistantPrimaryApiId && props.config.assistantDepartmentApiConfigId !== assistantPrimaryApiId) {
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
    apiConfigId: textDepartmentApiConfigs.value[0]?.id || "",
    apiConfigIds: textDepartmentApiConfigs.value[0]?.id ? [textDepartmentApiConfigs.value[0].id] : [],
    agentIds: [],
    createdAt: now,
    updatedAt: now,
    orderIndex: props.config.departments.length + 1,
    isBuiltInAssistant: false,
    source: "main_config",
    scope: "global",
  });
  // 计算新部门在哪一页并跳转
  const newIndex = props.config.departments.length - 1;
  page.value = Math.floor(newIndex / PAGE_SIZE) + 1;
  selectedDepartmentId.value = id;
}

function nextDepartmentName() {
  const base = t("config.department.newName");
  let index = props.config.departments.filter((item) => !item.isBuiltInAssistant).length + 1;
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
  if (!target || target.isBuiltInAssistant) return;
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

function availableDepartmentApiConfigsForIndex(index: number) {
  const currentIds = currentDepartmentApiConfigIds(selectedDepartment.value);
  const currentId = currentIds[index];
  return textDepartmentApiConfigs.value.filter((api) => api.id === currentId || !currentIds.includes(api.id));
}

function updateDepartmentApiConfigAt(index: number, apiId: string) {
  const target = selectedDepartment.value;
  if (!target) return;
  const next = currentDepartmentApiConfigIds(target);
  if (next[index] === apiId) return;
  next[index] = apiId;
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
  target.apiConfigIds = next.length > 0 ? next : (textDepartmentApiConfigs.value[0]?.id ? [textDepartmentApiConfigs.value[0].id] : []);
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

watch(
  () => departmentSnapshot.value,
  (snapshot) => {
    if (!departmentAutosaveReady) {
      lastSavedDepartmentSnapshot.value = snapshot;
      departmentAutosaveReady = true;
      return;
    }
    if (snapshot === lastSavedDepartmentSnapshot.value) return;
    if (autosaveTimer) clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(async () => {
      syncAssistantDepartmentState();
      if (departmentValidationMessage.value) return;
      const saved = await Promise.resolve(props.saveConfigAction());
      if (saved) {
        lastSavedDepartmentSnapshot.value = departmentSnapshot.value;
      }
    }, 1000);
  },
);
</script>
