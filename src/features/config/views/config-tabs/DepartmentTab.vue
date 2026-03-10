<template>
  <div class="grid gap-3">
    <!-- 操作栏 -->
    <div class="flex items-center justify-between">
      <div class="text-sm opacity-70">{{ t("config.department.hint") }}</div>
      <div class="flex items-center gap-2">
        <button class="btn btn-sm" :disabled="savingConfig" @click="addDepartment">{{ t("config.department.add") }}</button>
        <button class="btn btn-sm btn-primary" :disabled="savingConfig || hasDuplicateDepartmentName || hasEmptyDepartmentName" @click="$emit('saveApiConfig')">
          {{ t("config.tools.save") }}
        </button>
      </div>
    </div>

    <div class="grid gap-3 lg:grid-cols-[280px_minmax(0,1fr)]">
      <!-- 部门列表 -->
      <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
        <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
          <div class="font-medium">{{ t("config.department.title") }}<span v-if="sortedDepartments.length">（{{ sortedDepartments.length }}）</span></div>
        </div>
        <div class="divide-y divide-base-300/60">
          <button
            v-for="department in pagedDepartments"
            :key="department.id"
            class="w-full text-left px-3 py-2 hover:bg-base-200/60 transition-colors"
            :class="selectedDepartmentId === department.id ? 'bg-primary/10' : ''"
            @click="selectedDepartmentId = department.id"
          >
            <div class="flex items-center gap-2">
              <div class="font-medium text-sm">{{ department.name }}</div>
              <span v-if="department.isBuiltInAssistant" class="badge badge-xs badge-primary">{{ t("config.department.assistantBadge") }}</span>
              <span v-else-if="department.source === 'private_workspace'" class="badge badge-xs badge-secondary">{{ t("config.department.privateWorkspaceBadge") }}</span>
            </div>
            <div class="text-[11px] opacity-60 mt-1 line-clamp-2">
              {{ department.summary || t("config.department.emptySummary") }}
            </div>
          </button>
        </div>
        <!-- 分页 -->
        <div v-if="totalPages > 1" class="flex justify-center border-t border-base-300/70 px-3 py-2">
          <div class="join">
            <button class="btn btn-xs join-item" :disabled="page <= 1" @click="page--">‹</button>
            <button class="btn btn-xs join-item btn-active">{{ page }} / {{ totalPages }}</button>
            <button class="btn btn-xs join-item" :disabled="page >= totalPages" @click="page++">›</button>
          </div>
        </div>
      </div>

      <!-- 部门详情 -->
      <div v-if="selectedDepartment" class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
        <div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-base-300/70">
          <div class="flex items-center gap-2">
            <div class="font-medium">{{ selectedDepartment.name }}</div>
            <span v-if="selectedDepartmentIsPrivateWorkspace" class="badge badge-xs badge-secondary">{{ t("config.department.privateWorkspaceBadge") }}</span>
          </div>
          <button
            class="btn btn-sm btn-ghost text-error"
            :disabled="!!selectedDepartment.isBuiltInAssistant || selectedDepartmentIsPrivateWorkspace || savingConfig"
            @click="removeSelectedDepartment"
          >
            {{ t("config.department.remove") }}
          </button>
        </div>

        <div class="divide-y divide-base-300/60">
          <!-- 名称 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.department.name") }}</div>
            <input
              v-model.trim="selectedDepartment.name"
              class="input input-bordered input-sm w-full"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.namePlaceholder')"
            />
            <div v-if="selectedDepartmentNameEmpty" class="text-xs text-error mt-1">
              {{ t("config.department.emptyName") }}
            </div>
            <div v-if="selectedDepartmentNameDuplicated" class="text-xs text-error mt-1">
              {{ t("config.department.duplicateName") }}
            </div>
          </div>

          <!-- 任命 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.department.assignee") }}</div>
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
              class="text-xs opacity-60 mt-1"
            >
              {{ t("config.department.currentAssistant") }}
            </div>
          </div>

          <!-- 驱动模型 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.department.model") }}</div>
            <select v-model="selectedDepartment.apiConfigId" class="select select-bordered select-sm w-full" :disabled="selectedDepartmentIsPrivateWorkspace" @change="syncAssistantDepartmentState">
              <option v-for="api in textDepartmentApiConfigs" :key="api.id" :value="api.id">{{ api.name }}</option>
            </select>
            <div class="text-[11px] opacity-50 mt-1">{{ t("config.department.allowedModelsNote") }}</div>
          </div>

          <!-- 概述 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.department.summary") }}</div>
            <textarea
              v-model="selectedDepartment.summary"
              class="textarea textarea-bordered textarea-sm w-full min-h-20"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.summaryPlaceholder')"
            />
          </div>

          <!-- 办事指南 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.department.guide") }}</div>
            <textarea
              v-model="selectedDepartment.guide"
              class="textarea textarea-bordered textarea-sm w-full min-h-28"
              :disabled="selectedDepartmentIsPrivateWorkspace"
              :placeholder="t('config.department.guidePlaceholder')"
            />
            <div class="text-[11px] opacity-50 mt-1">{{ t("config.department.guideHint") }}</div>
          </div>
        </div>
      </div>

      <div v-else class="border border-base-300 rounded-box bg-base-100 p-8 text-center">
        <div class="text-sm opacity-50">{{ t("config.department.selectHint") }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, DepartmentConfig, PersonaProfile } from "../../../../types/app";

const PAGE_SIZE = 5;

const props = defineProps<{
  config: AppConfig;
  apiConfigs: ApiConfigItem[];
  personas: PersonaProfile[];
  assistantDepartmentAgentId: string;
  savingConfig: boolean;
}>();

const emit = defineEmits<{
  (e: "update:assistantDepartmentAssigneeId", value: string): void;
  (e: "saveApiConfig"): void;
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
  assistant.apiConfigId = String(assistant.apiConfigId || "").trim() || textDepartmentApiConfigs.value[0]?.id || "";
  props.config.assistantDepartmentApiConfigId = assistant.apiConfigId;
  const nextAssistantId = assistant.agentIds[0];
  if (nextAssistantId && nextAssistantId !== props.assistantDepartmentAgentId) {
    emit("update:assistantDepartmentAssigneeId", nextAssistantId);
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
  target.agentIds = agentId ? [agentId] : [];
  target.updatedAt = new Date().toISOString();
  syncAssistantDepartmentState();
}
</script>
