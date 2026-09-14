<template>
  <SettingsStickyLayout>
    <template #header>
      <Transition name="ecall-config-content" mode="out-in">
        <!-- 二级菜单头部：面包屑导航 + 部门专属操作 -->
        <div v-if="inDetailMode && selectedDepartment" :key="'detail-hdr-' + selectedDepartment.id" class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex min-w-0 items-center gap-2">
            <button
              class="btn btn-ghost btn-circle h-9 w-9 min-h-[2.25rem] shrink-0"
              type="button"
              :title="t('config.department.backToList')"
              @click="backToList"
            >
              <ArrowLeft class="h-5 w-5" />
            </button>
            <div class="breadcrumbs text-sm p-0">
              <ul>
                <li>
                  <a
                    class="cursor-pointer font-medium hover:text-primary transition-colors py-1 text-base-content/70 hover:text-base-content"
                    @click="backToList"
                  >
                    {{ selectedDepartmentIsSystemBuiltIn ? t("config.department.presetDepartments") : t("config.department.customDepartments") }}
                  </a>
                </li>
                <li class="font-semibold text-base-content max-w-[14rem] sm:max-w-xs md:max-w-md truncate py-1">
                  {{ selectedDepartment.name }}
                </li>
              </ul>
            </div>
            <!-- 状态徽章 -->
            <span v-if="selectedDepartment.isBuiltInAssistant" class="badge badge-primary badge-sm shrink-0 flex items-center gap-1">
              <Crown class="h-3 w-3" />
              <span>{{ t("config.department.assistantBadge") }}</span>
            </span>
            <span v-else-if="selectedDepartmentIsSystemBuiltIn" class="badge badge-neutral badge-sm shrink-0 flex items-center gap-1 opacity-80">
              <ShieldCheck class="h-3 w-3" />
              <span>{{ t("config.persona.systemTag") }}</span>
            </span>
            <span v-if="departmentDirty" class="badge badge-warning badge-sm shrink-0">
              {{ t("config.skill.unsaved") }}
            </span>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <!-- 系统内置部门支持恢复初始化 -->
            <button
              v-if="selectedDepartmentIsSystemBuiltIn"
              class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1.5 px-3"
              type="button"
              :disabled="savingConfig"
              :title="t('config.department.restoreInitial')"
              @click="handleSelectedDepartmentPrimaryAction"
            >
              <RotateCcw class="h-4 w-4" />
              <span>{{ t("config.department.restoreInitial") }}</span>
            </button>
            <!-- 自定义部门支持删除 -->
            <button
              v-else
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-error hover:bg-error/10 gap-1.5 px-3"
              type="button"
              :disabled="savingConfig"
              :title="t('config.department.remove')"
              @click="handleSelectedDepartmentPrimaryAction"
            >
              <Trash2 class="h-4 w-4" />
              <span>{{ t("config.department.remove") }}</span>
            </button>

            <!-- 还原未保存草稿 -->
            <button
              v-if="departmentDirty"
              class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1.5 px-3"
              type="button"
              :disabled="savingConfig"
              :title="t('common.reset')"
              @click="restoreDepartmentDraftsFromSaved"
            >
              <RotateCcw class="h-4 w-4" />
              <span>{{ t("common.reset") }}</span>
            </button>

            <!-- 保存部门 -->
            <button
              class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3.5"
              :class="departmentDirty ? 'btn-primary' : 'bg-base-100'"
              type="button"
              :disabled="!selectedDepartment || !!departmentValidationMessage || !departmentDirty || savingConfig"
              :title="savingConfig ? t('config.api.saving') : departmentDirty ? t('common.save') : t('status.configSaved')"
              @click="saveDepartments"
            >
              <span v-if="savingConfig" class="loading loading-spinner loading-xs"></span>
              <Save v-else class="h-4 w-4" />
              <span>{{ savingConfig ? t("common.saving") : t("common.save") }}</span>
            </button>
          </div>
        </div>

        <!-- 一级概览头部：分类筛选（自定义部门 vs 系统预设）+ 搜索过滤 + 新增按钮 -->
        <div v-else key="overview-hdr" class="flex flex-col gap-3">
          <SegmentedControl
            :model-value="activeCategoryTab"
            :options="departmentCategoryOptions"
            size="md"
            @change="(val) => { activeCategoryTab = val; departmentSearchQuery = ''; }"
          />

          <div class="flex flex-wrap items-center justify-between gap-3">
            <!-- 部门搜索过滤框 -->
            <div class="relative min-w-[12rem] flex-1">
              <input
                v-model="departmentSearchQuery"
                type="text"
                class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
                :placeholder="t('config.department.searchPlaceholder')"
              />
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
              <button
                v-if="departmentSearchQuery"
                type="button"
                class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
                :title="t('common.clear')"
                @click="departmentSearchQuery = ''"
              >
                ✕
              </button>
            </div>

            <div class="flex items-center gap-2">
              <button
                v-if="activeCategoryTab === 'custom'"
                class="btn btn-sm min-h-[2.25rem] btn-primary gap-1.5 px-3.5"
                type="button"
                :disabled="savingConfig"
                @click="addDepartment"
              >
                <Plus class="h-4 w-4" />
                <span>{{ t("config.department.add") }}</span>
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </template>

    <!-- 主体区域切换：一级卡片矩阵 ↔ 二级详情页 -->
    <Transition name="ecall-config-content" mode="out-in">
      <!-- 二级菜单：部门详情视图 -->
      <div v-if="inDetailMode && selectedDepartment" :key="'detail-body-' + selectedDepartment.id" class="grid gap-4 pb-8">
        <!-- 验证错误提示 -->
        <div v-if="departmentValidationMessage" class="rounded-box border border-warning/30 bg-warning/10 px-4 py-3 text-xs text-warning-content font-medium">
          {{ departmentValidationMessage }}
        </div>

        <!-- 卡片一：基本信息与负责人格 -->
        <div class="card bg-base-100 border border-base-300 card-sm shadow-xs">
          <div class="card-header border-b border-base-300/60 bg-base-200/40 px-4 py-2.5 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Building class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.department.basicSection") }}</span>
            </div>
          </div>

          <div class="card-body p-4 gap-4">
            <!-- 部门名称 -->
            <div class="flex flex-col gap-1.5">
              <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.department.name") }}</label>
              <input
                v-model.trim="selectedDepartment.name"
                class="input input-bordered input-sm h-9 w-full text-xs font-bold"
                :placeholder="t('config.department.namePlaceholder')"
                :disabled="selectedDepartmentIsFrozenHr"
                @input="touchSelectedDepartment"
              />
              <div v-if="selectedDepartmentNameEmpty" class="text-caption text-error">
                {{ t("config.department.emptyName") }}
              </div>
              <div v-if="selectedDepartmentNameDuplicated" class="text-caption text-error">
                {{ t("config.department.duplicateName") }}
              </div>
            </div>

            <!-- 负责人格勾选 -->
            <div class="flex flex-col gap-1.5">
              <div class="flex items-center justify-between">
                <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.department.assigneeLabel") }}</label>
                <span class="text-caption opacity-50">{{ t("config.department.memberCount", { count: selectedDepartmentAssigneeIds.length }) }}</span>
              </div>
              <div v-if="availableAssigneePersonas.length === 0" class="text-xs opacity-60 italic py-1">
                {{ t("config.department.assigneePlaceholder") }}
              </div>
              <OverlayScrollArea v-else scroller-class="flex max-h-48 flex-wrap items-center gap-2">
                <label
                  v-for="persona in availableAssigneePersonas"
                  :key="persona.id"
                  class="flex min-h-[2rem] cursor-pointer items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs transition-colors select-none"
                  :class="selectedDepartmentAssigneeIds.includes(persona.id)
                    ? 'border-primary bg-primary text-primary-content'
                    : 'border-base-300 bg-base-100 text-base-content/70 hover:border-primary/50 hover:text-base-content'"
                >
                  <input
                    type="checkbox"
                    class="sr-only"
                    :checked="selectedDepartmentAssigneeIds.includes(persona.id)"
                    :disabled="savingConfig"
                    @change="toggleDepartmentAssignee(persona.id)"
                  />
                  <Check
                    v-if="selectedDepartmentAssigneeIds.includes(persona.id)"
                    class="h-3.5 w-3.5 shrink-0"
                  />
                  <span class="font-medium truncate max-w-[10rem]">{{ persona.name || persona.id }}</span>
                </label>
              </OverlayScrollArea>
              <div v-if="selectedDepartmentAssigneeIds.length === 0" class="text-caption text-warning">
                {{ t("config.department.assigneeWarning") }}
              </div>
            </div>

            <!-- 什么时候呼唤我 -->
            <div class="flex flex-col gap-1.5">
              <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.department.summary") }}</label>
              <textarea
                v-model="selectedDepartment.summary"
                class="textarea textarea-bordered text-xs leading-relaxed min-h-20 w-full"
                :placeholder="t('config.department.summaryPlaceholder')"
                :disabled="selectedDepartmentIsFrozenHr"
                @input="touchSelectedDepartment"
              />
            </div>

            <!-- 办事指南 -->
            <div class="flex flex-col gap-1.5">
              <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.department.guide") }}</label>
              <textarea
                v-model="selectedDepartment.guide"
                class="textarea textarea-bordered text-xs leading-relaxed min-h-24 w-full"
                :placeholder="t('config.department.guidePlaceholder')"
                :disabled="selectedDepartmentIsFrozenHr"
                @input="touchSelectedDepartment"
              />
              <span class="text-caption opacity-50">{{ t("config.department.guideHint") }}</span>
            </div>
          </div>
        </div>

        <!-- 卡片二：驱动模型与容灾回退 -->
        <div class="card bg-base-100 border border-base-300 card-sm shadow-xs">
          <div class="card-header border-b border-base-300/60 bg-base-200/40 px-4 py-2.5 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Sparkles class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.department.modelSection") }}</span>
            </div>
          </div>

          <div class="card-body p-4 gap-3">
            <div class="grid min-w-0 gap-2.5">
              <div
                v-for="(apiId, idx) in selectedDepartmentVisibleApiConfigIds"
                :key="`${selectedDepartment.id}-api-${idx}`"
                class="flex items-center gap-2"
              >
                <ApiConfigPicker
                  class="flex-1"
                  :model-value="apiId"
                  :api-configs="availableDepartmentApiConfigsForIndex(idx)"
                  :extra-options="availableDepartmentRoleOptionsForIndex(idx).map((role) => ({ id: role.id, label: role.name }))"
                  @update:model-value="updateDepartmentApiConfigAt(idx, $event)"
                />

                <div class="join shrink-0">
                  <button
                    v-if="selectedDepartmentModelFailureFallbackEnabled"
                    class="btn btn-sm h-9 min-h-[2.25rem] btn-square join-item opacity-60 hover:opacity-100"
                    type="button"
                    :disabled="idx <= 0"
                    :title="t('config.department.moveUp')"
                    @click="moveDepartmentApiConfig(idx, -1)"
                  >
                    ↑
                  </button>
                  <button
                    v-if="selectedDepartmentModelFailureFallbackEnabled"
                    class="btn btn-sm h-9 min-h-[2.25rem] btn-square join-item opacity-60 hover:opacity-100"
                    type="button"
                    :disabled="idx >= selectedDepartmentApiConfigIds.length - 1"
                    :title="t('config.department.moveDown')"
                    @click="moveDepartmentApiConfig(idx, 1)"
                  >
                    ↓
                  </button>
                  <button
                    v-if="selectedDepartmentModelFailureFallbackEnabled"
                    class="btn btn-sm h-9 min-h-[2.25rem] btn-square join-item opacity-60 hover:opacity-100 text-error"
                    type="button"
                    :disabled="selectedDepartmentApiConfigIds.length <= 1"
                    :title="t('config.department.removeModel')"
                    @click="removeDepartmentApiConfigAt(idx)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>

              <button
                v-if="selectedDepartmentModelFailureFallbackEnabled"
                class="btn btn-sm min-h-[2.25rem] self-start"
                type="button"
                :disabled="remainingDepartmentRoleOptions.length <= 0 && remainingDepartmentApiConfigs.length <= 0"
                @click="addDepartmentApiConfig"
              >
                <Plus class="h-4 w-4 mr-1" />
                {{ t("config.department.addModel") }}
              </button>
            </div>
            <div class="text-caption opacity-50">{{ t("config.department.allowedModelsNote") }}</div>
          </div>
        </div>

        <!-- 卡片三：权限与工具控制 -->
        <div class="card bg-base-100 border border-base-300 card-sm shadow-xs">
          <div class="card-header border-b border-base-300/60 bg-base-200/40 px-4 py-2.5 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Wrench class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.department.permissionSection") }}</span>
            </div>
            <input
              type="checkbox"
              class="toggle toggle-sm toggle-primary"
              :checked="permissionControlEnabled"
              :disabled="selectedDepartmentIsFrozenHr"
              @change="updateDepartmentPermissionControl({ enabled: !!($event.target as HTMLInputElement).checked })"
            />
          </div>

          <div class="card-body p-4 gap-3" :class="selectedDepartmentIsFrozenHr ? 'pointer-events-none opacity-50' : ''">
            <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
              <span class="text-xs opacity-60">{{ t("config.department.permissionHint") }}</span>
              <select
                class="select select-bordered select-sm h-9 min-h-[2.25rem] w-full sm:w-40 text-xs shrink-0"
                :disabled="permissionListDisabled"
                :value="selectedDepartmentPermissionControl?.mode || 'blacklist'"
                @change="updateDepartmentPermissionControl({ mode: (($event.target as HTMLSelectElement).value === 'whitelist' ? 'whitelist' : 'blacklist') })"
              >
                <option value="blacklist">{{ t("config.department.permissionModeBlacklist") }}</option>
                <option value="whitelist">{{ t("config.department.permissionModeWhitelist") }}</option>
              </select>
            </div>

            <div v-if="permissionCatalogLoading" class="py-8 text-center text-xs opacity-60">
              <span class="loading loading-spinner loading-sm mr-2"></span>
              {{ t("config.department.permissionCatalogLoading") }}
            </div>
            <div v-else-if="permissionCatalogError" class="py-4 text-xs text-error">
              {{ t("config.department.permissionCatalogLoadFailed", { err: permissionCatalogError }) }}
            </div>
            <template v-else>
              <div v-if="skillPermissionRequiresExec" class="text-caption text-warning">
                {{ t("config.department.permissionSkillsRequireExec") }}
              </div>
              <DepartmentToolTree
                :sections="toolTreeSections"
                @leaf-toggle="handleToolTreeLeafToggle"
                @group-toggle="handleToolTreeGroupToggle"
              />
            </template>
          </div>
        </div>
      </div>

      <!-- 一级概览：部门卡片矩阵 -->
      <div v-else key="overview-grid" class="flex flex-col gap-4 pb-8">
        <!-- 空状态 -->
        <div v-if="displayedDepartments.length === 0" class="card border border-dashed border-base-300 bg-base-100 py-12">
          <div class="card-body items-center justify-center text-center">
            <Users class="h-10 w-10 opacity-30" />
            <h3 class="text-sm font-medium opacity-70">
              {{ departmentSearchQuery ? t("config.department.noSearchMatch") : (activeCategoryTab === 'custom' ? t("config.department.noCustomDepartments") : t("config.department.noPresetDepartments")) }}
            </h3>
            <p class="text-xs opacity-50">
              {{ departmentSearchQuery ? t("common.clear") : (activeCategoryTab === 'custom' ? t("config.department.noCustomDepartmentsHint") : "") }}
            </p>
            <div class="card-actions mt-3">
              <button
                v-if="departmentSearchQuery"
                class="btn btn-sm min-h-[2.25rem] btn-ghost text-xs"
                type="button"
                @click="departmentSearchQuery = ''"
              >
                {{ t("common.clear") }}
              </button>
              <button
                v-else-if="activeCategoryTab === 'custom'"
                class="btn btn-sm min-h-[2.25rem] btn-primary text-xs"
                type="button"
                @click="addDepartment"
              >
                <Plus class="h-4 w-4" />
                <span>{{ t("config.department.add") }}</span>
              </button>
            </div>
          </div>
        </div>

        <!-- 自适应卡片网格 -->
        <div v-else class="config-grid-auto-md">
          <div
            v-for="dept in displayedDepartments"
            :key="dept.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
            @click="enterDepartment(dept.id)"
            @keydown.enter.prevent="enterDepartment(dept.id)"
            @keydown.space.prevent="enterDepartment(dept.id)"
          >
            <!-- 头部：部门名称 + 徽章 + 模型名称 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ dept.name }}
                  </span>
                  <span v-if="dept.isBuiltInAssistant" class="badge badge-primary badge-xs shrink-0">
                    {{ t("config.department.assistantBadge") }}
                  </span>
                  <span v-else-if="isSystemBuiltInDepartment(dept)" class="badge badge-neutral badge-xs opacity-70 shrink-0">
                    {{ t("config.persona.systemTag") }}
                  </span>
                </div>
                <div class="text-caption opacity-50 truncate mt-0.5">
                  {{ getDepartmentModelName(dept) || t("config.department.model") }}
                </div>
              </div>
            </div>

            <!-- 中间：部门简介 -->
            <p class="text-xs text-base-content/70 line-clamp-2 leading-relaxed min-h-[2.5rem] break-words">
              {{ dept.summary || dept.guide || t("config.department.hint") }}
            </p>

            <!-- 底栏：成员数 + 权限模式 + 进入指示 -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption opacity-70">
              <div class="flex items-center gap-1.5">
                <Users class="h-3.5 w-3.5 opacity-60" />
                <span>{{ t("config.department.memberCount", { count: (dept.agentIds || []).length }) }}</span>
              </div>
              <div class="flex items-center gap-1.5">
                <span v-if="dept.permissionControl?.enabled" class="font-mono text-xs">
                  {{ dept.permissionControl.mode === 'whitelist' ? t("config.department.permissionModeWhitelist") : t("config.department.permissionModeBlacklist") }}
                </span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  ArrowLeft,
  Briefcase,
  Check,
  ChevronRight,
  Crown,
  Lock,
  Plus,
  RotateCcw,
  Save,
  Search,
  ShieldCheck,
  Sparkles,
  Trash2,
  Users,
  Wrench,
} from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import type { ApiConfigItem, AppConfig, DepartmentConfig, DepartmentPermissionCatalog, PersonaProfile } from "../../../../types/app";
import {
  buildBuiltinToolGroups,
  buildMcpToolGroups,
  normalizeDepartmentPermissionCatalog,
  type DepartmentToolLeafCategory,
  type DepartmentToolTreeSection,
} from "../../utils/department-tool-tree";
import DepartmentToolTree from "./department/DepartmentToolTree.vue";
import {
  buildDepartmentBasicSnapshot,
  departmentBasicComparableSnapshot,
  mergeDepartmentChildIdsFromSource,
} from "../../utils/department-basic-editor";
import { validateDepartmentConfig } from "../../utils/department-validation";
import { normalizeDepartmentChildIds } from "../../utils/department-graph";
import { MODEL_ROLE_EXPERT_API_CONFIG_ID, MODEL_ROLE_QUICK_API_CONFIG_ID } from "../../utils/model-role-options";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import ApiConfigPicker from "../../components/ApiConfigPicker.vue";
import ConfigCard from "../../components/ConfigCard.vue";
import SegmentedControl from "../../components/SegmentedControl.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";

const props = defineProps<{
  config: AppConfig;
  apiConfigs: ApiConfigItem[];
  personas: PersonaProfile[];
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  setStatusAction: (text: string) => void;
}>();

const { t } = useI18n();
const inDetailMode = ref(false);
const departmentSearchQuery = ref("");
const selectedDepartmentId = ref("assistant-department");
const SYSTEM_DEPARTMENT_IDS = new Set([
  "assistant-department",
  "leader-department",
  "deputy-department",
  "reviewer-department",
  "saddler-department",
  "remote-customer-service-department",
  "hr-department",
]);

const TEXT_REQUEST_FORMATS = new Set([
  "auto",
  "openai",
  "deepseek",
  "openai_responses",
  "codex",
  "gemini",
  "anthropic",
  "fireworks",
  "together",
  "groq",
  "mimo",
  "minimax",
  "moonshot",
  "nebius",
  "xai",
  "zai",
  "bigmodel",
  "aliyun",
  "baidu",
  "cohere",
  "ollama",
  "ollama_cloud",
  "vertex",
  "github_copilot",
  "opencode_go",
  "bedrock_api",
]);

function isTextRequestFormat(format: string): boolean {
  const normalized = String(format || "").trim().toLowerCase();
  return normalized === "deepseek/kimi" || TEXT_REQUEST_FORMATS.has(normalized);
}

function isSystemBuiltInDepartment(department: DepartmentConfig | null | undefined) {
  if (!department) return false;
  const id = String(department.id || "").trim();
  return SYSTEM_DEPARTMENT_IDS.has(id) || !!department.isBuiltInAssistant;
}

// 人力部除负责人格外全部字段冻结（后端也会强制覆盖）
function isFrozenHrDepartment(department: DepartmentConfig | null | undefined) {
  return String(department?.id || "").trim() === "hr-department";
}

function normalizeNameList(value: unknown): string[] {
  return Array.isArray(value)
    ? Array.from(new Set(value.map((item) => String(item || "").trim()).filter(Boolean)))
    : [];
}

function normalizePermissionControl(permissionControl: DepartmentConfig["permissionControl"] | null | undefined) {
  return {
    enabled: !!permissionControl?.enabled,
    mode: permissionControl?.mode === "whitelist" ? "whitelist" : "blacklist",
    builtinToolNames: normalizeNameList(permissionControl?.builtinToolNames),
    skillNames: normalizeNameList(permissionControl?.skillNames),
    mcpToolNames: normalizeNameList(permissionControl?.mcpToolNames),
  } as const;
}

function cloneDepartment(department: DepartmentConfig): DepartmentConfig {
  const apiConfigIds = normalizeNameList(
    Array.isArray(department.apiConfigIds) && department.apiConfigIds.length > 0
      ? department.apiConfigIds
      : [department.apiConfigId || ""],
  );
  const id = String(department.id || "").trim();
  const agentIds = normalizeNameList(department.agentIds);
  return {
    id,
    name: String(department.name || ""),
    summary: String(department.summary || ""),
    guide: String(department.guide || ""),
    apiConfigId: apiConfigIds[0] || "",
    apiConfigIds,
    modelFailureFallbackEnabled: !!department.modelFailureFallbackEnabled,
    agentIds,
    childDepartmentIds: normalizeDepartmentChildIds(department.childDepartmentIds, id),
    createdAt: String(department.createdAt || "").trim(),
    updatedAt: String(department.updatedAt || "").trim(),
    orderIndex: Math.max(1, Number(department.orderIndex || 1)),
    isBuiltInAssistant: !!department.isBuiltInAssistant,
    source: String(department.source || "").trim() || "main_config",
    scope: String(department.scope || "").trim() || "global",
    permissionControl: normalizePermissionControl(department.permissionControl),
  };
}

function cloneDepartmentList(departments: DepartmentConfig[] | null | undefined) {
  return (departments || []).map(cloneDepartment);
}

function removedDepartmentIdsFromSource(
  drafts: DepartmentConfig[] | null | undefined,
  source: DepartmentConfig[] | null | undefined,
) {
  const draftIds = new Set((drafts || []).map((item) => String(item.id || "").trim()).filter(Boolean));
  return (source || [])
    .map((item) => String(item.id || "").trim())
    .filter((id) => !!id && !draftIds.has(id));
}

const departmentDrafts = ref<DepartmentConfig[]>(cloneDepartmentList(props.config.departments || []));
const permissionCatalog = ref<DepartmentPermissionCatalog>({
  builtinTools: [],
  skills: [],
  mcpTools: [],
});
const permissionCatalogLoading = ref(false);
const permissionCatalogError = ref("");

const sortedDepartments = computed(() =>
  [...departmentDrafts.value].sort((a, b) => {
    const rank = (id: string) =>
      id === "assistant-department" ? 0 : id === "leader-department" ? 1 : id === "deputy-department" ? 2 : id === "reviewer-department" ? 3 : id === "saddler-department" ? 4 : id === "remote-customer-service-department" ? 5 : id === "hr-department" ? 6 : 7;
    const aRank = rank(String(a.id || "").trim());
    const bRank = rank(String(b.id || "").trim());
    return aRank - bRank || a.orderIndex - b.orderIndex;
  }),
);

const activeCategoryTab = ref<"custom" | "preset">(
  props.config.departments?.some((d) => !isSystemBuiltInDepartment(d)) ? "custom" : "preset"
);

const presetDepartments = computed(() =>
  sortedDepartments.value.filter((d) => isSystemBuiltInDepartment(d))
);

const customDepartments = computed(() =>
  sortedDepartments.value.filter((d) => !isSystemBuiltInDepartment(d))
);

const departmentCategoryOptions = computed(() => [
  {
    value: "custom" as const,
    label: t("config.department.customDepartments"),
    badge: customDepartments.value.length,
  },
  {
    value: "preset" as const,
    label: t("config.department.presetDepartments"),
    badge: presetDepartments.value.length,
  },
]);

const displayedDepartments = computed(() => {
  const sourceList = activeCategoryTab.value === "custom" ? customDepartments.value : presetDepartments.value;
  const q = departmentSearchQuery.value.trim().toLowerCase();
  if (!q) return sourceList;
  return sourceList.filter(
    (d) =>
      (d.name || "").toLowerCase().includes(q) ||
      (d.summary || "").toLowerCase().includes(q) ||
      (d.guide || "").toLowerCase().includes(q) ||
      (d.id || "").toLowerCase().includes(q)
  );
});

function enterDepartment(deptId: string) {
  selectedDepartmentId.value = deptId;
  const dept = departmentDrafts.value.find((d) => d.id === deptId);
  if (dept) {
    activeCategoryTab.value = isSystemBuiltInDepartment(dept) ? "preset" : "custom";
  }
  inDetailMode.value = true;
}

function backToList() {
  if (departmentDirty.value) {
    const confirmLeave = window.confirm(t("config.skill.confirmLeaveUnsaved") || "当前部门有未保存的修改，确认返回列表吗？");
    if (!confirmLeave) return;
  }
  inDetailMode.value = false;
}

function getDepartmentModelName(department: DepartmentConfig): string {
  const primaryId = (department.apiConfigIds && department.apiConfigIds[0]) || department.apiConfigId;
  if (!primaryId) return "";
  if (primaryId === MODEL_ROLE_EXPERT_API_CONFIG_ID) return t("config.modelRoles.expert");
  if (primaryId === MODEL_ROLE_QUICK_API_CONFIG_ID) return t("config.modelRoles.quick");
  const found = props.apiConfigs.find((c) => c.id === primaryId);
  return found?.name || primaryId;
}

function getDepartmentPersonaNames(department: DepartmentConfig): string[] {
  const ids = department.agentIds || [];
  return ids
    .map((id) => props.personas.find((p) => p.id === id)?.name || id)
    .filter(Boolean);
}

const selectedDepartment = computed(
  () => departmentDrafts.value.find((item) => item.id === selectedDepartmentId.value) ?? sortedDepartments.value[0] ?? null,
);
const selectedDepartmentIsSystemBuiltIn = computed(() => isSystemBuiltInDepartment(selectedDepartment.value));
const selectedDepartmentIsFrozenHr = computed(() => isFrozenHrDepartment(selectedDepartment.value));
const textDepartmentApiConfigs = computed(() =>
  props.apiConfigs.filter((api) => !!api.enableText && isTextRequestFormat(api.requestFormat)),
);
const departmentRoleApiConfigOptions = computed(() => [
  { id: MODEL_ROLE_EXPERT_API_CONFIG_ID, name: roleModelDisplayName(MODEL_ROLE_EXPERT_API_CONFIG_ID) },
  { id: MODEL_ROLE_QUICK_API_CONFIG_ID, name: roleModelDisplayName(MODEL_ROLE_QUICK_API_CONFIG_ID) },
]);
const selectedDepartmentApiConfigIds = computed(() =>
  currentDepartmentApiConfigIdsForEditor(selectedDepartment.value),
);
const selectedDepartmentCanEnableModelFailureFallback = computed(() => true);
const selectedDepartmentModelFailureFallbackEnabled = computed(() =>
  selectedDepartmentCanEnableModelFailureFallback.value && !!selectedDepartment.value?.modelFailureFallbackEnabled,
);
const selectedDepartmentVisibleApiConfigIds = computed(() =>
  selectedDepartmentModelFailureFallbackEnabled.value
    ? selectedDepartmentApiConfigIds.value
    : selectedDepartmentApiConfigIds.value.slice(0, 1),
);
const remainingDepartmentApiConfigs = computed(() => {
  const selectedIds = new Set(selectedDepartmentApiConfigIds.value);
  return textDepartmentApiConfigs.value.filter((api) => !selectedIds.has(api.id));
});
const remainingDepartmentRoleOptions = computed(() => {
  const selectedIds = new Set(selectedDepartmentApiConfigIds.value);
  return departmentRoleApiConfigOptions.value.filter((role) => !selectedIds.has(role.id));
});
const departmentNameCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const department of departmentDrafts.value) {
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
const sourceDepartmentSnapshot = computed(() => buildDepartmentBasicSnapshot(props.config.departments || []));
const sourceDepartmentRelationSnapshot = computed(() =>
  JSON.stringify(
    (props.config.departments || []).map((item) => ({
      id: String(item.id || "").trim(),
      childDepartmentIds: normalizeDepartmentChildIds(item.childDepartmentIds, item.id),
    })),
  ),
);
const departmentSnapshot = computed(() => buildDepartmentBasicSnapshot(departmentDrafts.value));
const departmentDirty = computed(() => departmentSnapshot.value !== sourceDepartmentSnapshot.value);
const departmentValidationMessage = computed(() =>
  validateDepartmentConfig(
    {
      ...props.config,
      departments: mergeDepartmentChildIdsFromSource(
        cloneDepartmentList(departmentDrafts.value),
        props.config.departments || [],
        removedDepartmentIdsFromSource(departmentDrafts.value, props.config.departments || []),
      ),
    },
    props.apiConfigs,
    (key, params) => t(key, params ?? {}),
  ),
);

const selectedDepartmentPermissionControl = computed(() => selectedDepartment.value?.permissionControl ?? null);
const permissionControlEnabled = computed(() => !!selectedDepartmentPermissionControl.value?.enabled);
const permissionListDisabled = computed(() =>
  !permissionControlEnabled.value,
);
const permissionExecAllowed = computed(() => {
  const control = selectedDepartmentPermissionControl.value;
  if (!control?.enabled) return false;
  const execSelected = (control.builtinToolNames || []).includes("exec");
  return control.mode === "whitelist" ? execSelected : !execSelected;
});
const skillPermissionRequiresExec = computed(() =>
  permissionControlEnabled.value && !permissionExecAllowed.value,
);
const skillPermissionListDisabled = computed(() =>
  permissionListDisabled.value || skillPermissionRequiresExec.value,
);

const BUILTIN_TOOL_GROUP_LABEL_KEYS: Record<string, string> = {
  files: "config.department.permissionGroupFiles",
  execConfig: "config.department.permissionGroupExecConfig",
  desktop: "config.department.permissionGroupDesktop",
  web: "config.department.permissionGroupWeb",
  delegate: "config.department.permissionGroupDelegate",
  media: "config.department.permissionGroupMedia",
  other: "config.department.permissionGroupOther",
};

const toolTreeSections = computed<DepartmentToolTreeSection[]>(() => {
  const control = selectedDepartmentPermissionControl.value;
  const checkedSetFor = (category: DepartmentToolLeafCategory) =>
    new Set((control?.[category] || []).map((value) => String(value || "").trim()).filter(Boolean));
  const builtinChecked = checkedSetFor("builtinToolNames");
  const skillChecked = checkedSetFor("skillNames");
  const mcpChecked = checkedSetFor("mcpToolNames");
  return [
    {
      key: "builtinToolNames",
      label: t("config.department.permissionBuiltinTools"),
      disabled: permissionListDisabled.value,
      groups: buildBuiltinToolGroups(
        permissionCatalog.value.builtinTools,
        (name) => builtinChecked.has(name),
        (groupKey) => t(BUILTIN_TOOL_GROUP_LABEL_KEYS[groupKey] ?? "config.department.permissionGroupOther"),
      ),
      leaves: [],
    },
    {
      key: "skillNames",
      label: t("config.department.permissionSkills"),
      disabled: skillPermissionListDisabled.value,
      groups: [],
      leaves: permissionCatalog.value.skills.map((item) => ({
        category: "skillNames" as const,
        name: item.name,
        displayName: item.name,
        description: item.description,
        enabled: skillChecked.has(item.name),
      })),
    },
    {
      key: "mcpToolNames",
      label: t("config.department.permissionMcpTools"),
      disabled: permissionListDisabled.value,
      groups: buildMcpToolGroups(
        permissionCatalog.value.mcpTools,
        (name) => mcpChecked.has(name),
        t("config.department.permissionGroupOther"),
      ),
      leaves: [],
    },
  ];
});

const availableAssigneePersonas = computed(() =>
  sortPersonasForSelect(
    props.personas.filter((persona) => {
      const id = String(persona.id || "").trim();
      return !!id && canServeAsRegularDepartmentPersona(persona);
    }),
  ),
);
const selectedDepartmentAssigneeIds = computed(() =>
  normalizeNameList(selectedDepartment.value?.agentIds || []),
);
function canServeAsRegularDepartmentPersona(persona: PersonaProfile): boolean {
  const id = String(persona.id || "").trim();
  return id !== "user-persona" && !persona.isBuiltInUser && (id === "deputy-agent" || !persona.isBuiltInSystem);
}

function personaSelectRank(persona: PersonaProfile): number {
  if (persona.isBuiltInUser) return 0;
  if (persona.isBuiltInSystem) return 1;
  return 2;
}

function sortPersonasForSelect(personas: PersonaProfile[]): PersonaProfile[] {
  return personas
    .map((persona, index) => ({ persona, index }))
    .sort((a, b) => personaSelectRank(a.persona) - personaSelectRank(b.persona) || a.index - b.index)
    .map((item) => item.persona);
}

function ensureDepartmentPermissionControl(target: DepartmentConfig | null | undefined) {
  if (!target) return null;
  if (!target.permissionControl) {
    target.permissionControl = normalizePermissionControl(null);
  }
  return target.permissionControl;
}

function syncDepartmentDraftsFromSource() {
  const currentSelection = selectedDepartmentId.value;
  departmentDrafts.value = cloneDepartmentList(props.config.departments || []);
  if (departmentDrafts.value.some((item) => item.id === currentSelection)) {
    selectedDepartmentId.value = currentSelection;
    return;
  }
  selectedDepartmentId.value = departmentDrafts.value[0]?.id || "assistant-department";
}

function restoreDepartmentDraftsFromSaved() {
  syncDepartmentDraftsFromSource();
}

function touchSelectedDepartment() {
  // Draft fields are already reactive; timestamps are refreshed on save only.
}

watch(
  () => sortedDepartments.value.map((item) => item.id).join("|"),
  () => {
    if (!sortedDepartments.value.some((item) => item.id === selectedDepartmentId.value)) {
      selectedDepartmentId.value = sortedDepartments.value[0]?.id || "assistant-department";
    }
  },
  { immediate: true },
);

watch(
  () => sourceDepartmentSnapshot.value,
  () => {
    if (departmentDirty.value) return;
    syncDepartmentDraftsFromSource();
  },
);

watch(
  () => sourceDepartmentRelationSnapshot.value,
  () => {
    departmentDrafts.value = mergeDepartmentChildIdsFromSource(
      departmentDrafts.value,
      props.config.departments || [],
    );
  },
);

watch(
  () => ({
    departmentId: selectedDepartment.value?.id || "",
    enabled: permissionControlEnabled.value,
    mode: selectedDepartmentPermissionControl.value?.mode || "blacklist",
    builtinToolNames: (selectedDepartmentPermissionControl.value?.builtinToolNames || []).join("|"),
    blocked: skillPermissionRequiresExec.value,
  }),
  () => {
    const control = selectedDepartmentPermissionControl.value;
    if (!control || !skillPermissionRequiresExec.value || control.skillNames.length <= 0) {
      return;
    }
    updateDepartmentPermissionControl({ skillNames: [] });
  },
);

async function loadPermissionCatalog() {
  permissionCatalogLoading.value = true;
  permissionCatalogError.value = "";
  try {
    const payload = await invokeTauri<DepartmentPermissionCatalog>("list_department_permission_catalog");
    permissionCatalog.value = normalizeDepartmentPermissionCatalog(payload);
  } catch (error) {
    permissionCatalogError.value = String(error || "");
  } finally {
    permissionCatalogLoading.value = false;
  }
}

function updateDepartmentPermissionControl(patch: Partial<NonNullable<DepartmentConfig["permissionControl"]>>) {
  const target = selectedDepartment.value;
  const control = ensureDepartmentPermissionControl(target);
  console.info("[部门权限] 更新开关", {
    departmentId: target?.id || "",
    patch,
    hasTarget: !!target,
    hasControl: !!control,
    enabledBefore: !!control?.enabled,
    modeBefore: control?.mode || "",
  });
  if (!target || !control) return;
  if ("enabled" in patch) {
    control.enabled = !!patch.enabled;
  }
  if ("mode" in patch) {
    control.mode = patch.mode === "whitelist" ? "whitelist" : "blacklist";
  }
  if ("builtinToolNames" in patch) {
    control.builtinToolNames = normalizeNameList(patch.builtinToolNames);
  }
  if ("skillNames" in patch) {
    control.skillNames = normalizeNameList(patch.skillNames);
  }
  if ("mcpToolNames" in patch) {
    control.mcpToolNames = normalizeNameList(patch.mcpToolNames);
  }
  console.info("[部门权限] 更新完成", {
    departmentId: target.id,
    enabledAfter: !!control.enabled,
    modeAfter: control.mode,
    builtinCount: control.builtinToolNames.length,
    skillCount: control.skillNames.length,
    mcpCount: control.mcpToolNames.length,
  });
  touchSelectedDepartment();
}

function setPermissionNamesBatch(category: DepartmentToolLeafCategory, names: string[], checked: boolean) {
  const control = selectedDepartmentPermissionControl.value;
  if (!control) return;
  const next = new Set((control[category] || []).map((value) => String(value || "").trim()).filter(Boolean));
  for (const raw of names) {
    const trimmed = String(raw || "").trim();
    if (!trimmed) continue;
    if (checked) {
      next.add(trimmed);
    } else {
      next.delete(trimmed);
    }
  }
  updateDepartmentPermissionControl({ [category]: Array.from(next) } as Partial<NonNullable<DepartmentConfig["permissionControl"]>>);
}

function handleToolTreeLeafToggle(payload: { category: DepartmentToolLeafCategory; name: string; checked: boolean }) {
  setPermissionNamesBatch(payload.category, [payload.name], payload.checked);
}

function handleToolTreeGroupToggle(payload: { category: DepartmentToolLeafCategory; names: string[]; checked: boolean }) {
  setPermissionNamesBatch(payload.category, payload.names, payload.checked);
}

function nextDepartmentName() {
  const base = t("config.department.newName");
  let index = departmentDrafts.value.filter((item) => !isSystemBuiltInDepartment(item)).length + 1;
  while (true) {
    const name = `${base} ${index}`;
    const exists = departmentDrafts.value.some(
      (item) => String(item.name || "").trim().toLocaleLowerCase() === name.trim().toLocaleLowerCase(),
    );
    if (!exists) return name;
    index += 1;
  }
}

async function addDepartment() {
  if (props.savingConfig) return;
  activeCategoryTab.value = "custom";
  const previousDepartments = cloneDepartmentList(props.config.departments || []);
  const previousSelectedDepartmentId = selectedDepartmentId.value;
  const now = new Date().toISOString();
  const id = `department-${Date.now()}`;
  const maxOrderIndex = departmentDrafts.value.reduce((max, item) => Math.max(max, Number(item.orderIndex || 0)), 0);
  const defaultChildDepartmentIds = departmentDrafts.value.some((item) => String(item.id || "").trim() === "deputy-department")
    ? ["deputy-department"]
    : [];
  departmentDrafts.value.push({
    id,
    name: nextDepartmentName(),
    summary: "",
    guide: "",
    apiConfigId: MODEL_ROLE_EXPERT_API_CONFIG_ID,
    apiConfigIds: [MODEL_ROLE_EXPERT_API_CONFIG_ID],
    modelFailureFallbackEnabled: false,
    agentIds: [],
    childDepartmentIds: defaultChildDepartmentIds,
    createdAt: now,
    updatedAt: now,
    orderIndex: maxOrderIndex + 1,
    isBuiltInAssistant: false,
    source: "main_config",
    scope: "global",
    permissionControl: normalizePermissionControl(null),
  });
  selectedDepartmentId.value = id;
  props.config.departments = cloneDepartmentList(departmentDrafts.value);
  const saved = await Promise.resolve(props.saveConfigAction());
  if (!saved) {
    props.config.departments = previousDepartments;
    syncDepartmentDraftsFromSource();
    selectedDepartmentId.value = previousSelectedDepartmentId;
    return;
  }
  syncDepartmentDraftsFromSource();
  selectedDepartmentId.value = id;
  inDetailMode.value = true;
}

function removeSelectedDepartment() {
  const target = selectedDepartment.value;
  if (!target || isSystemBuiltInDepartment(target)) return;
  const targetId = String(target.id || "").trim();
  if (!targetId) return;
  const nextSelectedId =
    departmentDrafts.value.find((item) => item.id !== targetId)?.id
    || "";

  departmentDrafts.value = departmentDrafts.value
    .filter((item) => item.id !== targetId)
    .map((item) => {
      const nextChildDepartmentIds = normalizeDepartmentChildIds(item.childDepartmentIds, item.id)
        .filter((childId) => childId !== targetId);
      if (JSON.stringify(nextChildDepartmentIds) === JSON.stringify(normalizeDepartmentChildIds(item.childDepartmentIds, item.id))) {
        return item;
      }
      return {
        ...item,
        childDepartmentIds: nextChildDepartmentIds,
        updatedAt: new Date().toISOString(),
      };
    });
  selectedDepartmentId.value = nextSelectedId;
  inDetailMode.value = false;
}

async function restoreSelectedDepartment() {
  const target = selectedDepartment.value;
  if (!target) return;
  try {
    const defaults = await invokeTauri<DepartmentConfig>("get_department_default_draft", {
      departmentId: target.id,
    });
    if (String(selectedDepartment.value?.id || "").trim() !== target.id) return;
    target.name = String(defaults.name || "");
    target.summary = String(defaults.summary || "");
    target.guide = String(defaults.guide || "");
    target.apiConfigId = String(defaults.apiConfigId || "");
    target.apiConfigIds = normalizeNameList(defaults.apiConfigIds);
    target.modelFailureFallbackEnabled = !!defaults.modelFailureFallbackEnabled;
    target.agentIds = normalizeNameList(defaults.agentIds);
    target.permissionControl = normalizePermissionControl(defaults.permissionControl);
    touchSelectedDepartment();
  } catch (error) {
    props.setStatusAction(String(error || ""));
  }
}

function handleSelectedDepartmentPrimaryAction() {
  if (!selectedDepartment.value) return;
  if (selectedDepartmentIsSystemBuiltIn.value) {
    void restoreSelectedDepartment();
    return;
  }
  removeSelectedDepartment();
}

function updateDepartmentAssignees(agentIds: string[]) {
  const target = selectedDepartment.value;
  if (!target) return;
  const allowedIds = new Set(availableAssigneePersonas.value.map((persona) => String(persona.id || "").trim()).filter(Boolean));
  const nextAgentIds = normalizeNameList(agentIds).filter((agentId) => allowedIds.has(agentId));
  if (JSON.stringify(nextAgentIds) === JSON.stringify(normalizeNameList(target.agentIds || []))) return;
  target.agentIds = nextAgentIds;
  touchSelectedDepartment();
}

function toggleDepartmentAssignee(agentId: string) {
  const normalizedAgentId = String(agentId || "").trim();
  if (!normalizedAgentId) return;
  const current = selectedDepartmentAssigneeIds.value;
  updateDepartmentAssignees(
    current.includes(normalizedAgentId)
      ? current.filter((item) => item !== normalizedAgentId)
      : [...current, normalizedAgentId],
  );
}

function currentDepartmentApiConfigIds(target: DepartmentConfig | null | undefined) {
  if (!target) return [];
  const ids = Array.isArray(target.apiConfigIds) && target.apiConfigIds.length > 0
    ? target.apiConfigIds
    : [target.apiConfigId || ""];
  return ids.map((id) => String(id || "").trim()).filter(Boolean);
}

function departmentCanEnableModelFailureFallback(_target: DepartmentConfig | null | undefined) {
  return true;
}

function apiConfigName(apiConfigId: string): string {
  const id = String(apiConfigId || "").trim();
  if (!id) return "";
  const apiConfig = textDepartmentApiConfigs.value.find((api) => String(api.id || "").trim() === id);
  return String(apiConfig?.name || "").trim();
}

function roleModelDisplayName(roleId: string): string {
  const roleLabel = roleId === MODEL_ROLE_QUICK_API_CONFIG_ID
    ? t("config.modelRoles.quick")
    : t("config.modelRoles.expert");
  const concreteId = roleId === MODEL_ROLE_QUICK_API_CONFIG_ID
    ? props.config.toolReviewApiConfigId
    : props.config.assistantDepartmentApiConfigId;
  const concreteName = apiConfigName(String(concreteId || "").trim());
  return concreteName ? `${roleLabel}（${concreteName}）` : roleLabel;
}

function currentDepartmentApiConfigIdsForEditor(target: DepartmentConfig | null | undefined) {
  const ids = currentDepartmentApiConfigIds(target);
  return ids.length > 0 ? Array.from(new Set(ids)) : [MODEL_ROLE_EXPERT_API_CONFIG_ID];
}

function departmentModelIdsForSave(target: DepartmentConfig): string[] {
  const ids = currentDepartmentApiConfigIdsForEditor(target);
  return departmentCanEnableModelFailureFallback(target) && target.modelFailureFallbackEnabled ? ids : ids.slice(0, 1);
}

function availableDepartmentApiConfigsForIndex(index: number) {
  const currentIds = currentDepartmentApiConfigIds(selectedDepartment.value);
  const currentId = currentIds[index];
  return textDepartmentApiConfigs.value.filter((api) => api.id === currentId || !currentIds.includes(api.id));
}

function availableDepartmentRoleOptionsForIndex(index: number) {
  const currentIds = currentDepartmentApiConfigIds(selectedDepartment.value);
  const currentId = currentIds[index];
  return departmentRoleApiConfigOptions.value.filter((role) => role.id === currentId || !currentIds.includes(role.id));
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
  if (target.apiConfigIds.length === 0) {
    target.apiConfigIds = [MODEL_ROLE_EXPERT_API_CONFIG_ID];
  }
  target.apiConfigId = target.apiConfigIds[0] || "";
  touchSelectedDepartment();
}

function addDepartmentApiConfig() {
  const target = selectedDepartment.value;
  if (!target) return;
  const nextRole = remainingDepartmentRoleOptions.value[0];
  const nextApi = remainingDepartmentApiConfigs.value[0];
  if (!nextRole && !nextApi) return;
  const next = currentDepartmentApiConfigIds(target);
  next.push(nextRole?.id || nextApi?.id || MODEL_ROLE_EXPERT_API_CONFIG_ID);
  target.apiConfigIds = next;
  target.apiConfigId = next[0] || "";
  touchSelectedDepartment();
}

function removeDepartmentApiConfigAt(index: number) {
  const target = selectedDepartment.value;
  if (!target) return;
  const next = currentDepartmentApiConfigIds(target);
  next.splice(index, 1);
  target.apiConfigIds = next.length > 0 ? next : [MODEL_ROLE_EXPERT_API_CONFIG_ID];
  target.apiConfigId = target.apiConfigIds[0] || "";
  touchSelectedDepartment();
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
  touchSelectedDepartment();
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

function applyUpdatedAtToChangedDepartments(
  nextDepartments: DepartmentConfig[],
  previousDepartments: DepartmentConfig[],
) {
  const previousById = new Map(
    previousDepartments.map((item) => [item.id, departmentBasicComparableSnapshot(item)] as const),
  );
  const now = new Date().toISOString();
  return nextDepartments.map((item) => {
    const previousSnapshot = previousById.get(item.id);
    const nextSnapshot = departmentBasicComparableSnapshot(item);
    if (previousSnapshot === nextSnapshot) {
      return item;
    }
    return {
      ...item,
      updatedAt: now,
    };
  });
}

function prepareDepartmentsForSave(departments: DepartmentConfig[]) {
  return departments.map((department) => {
    const departmentId = String(department.id || "").trim();
    const apiConfigIds = departmentModelIdsForSave(department);
    return {
      ...department,
      apiConfigIds,
      apiConfigId: apiConfigIds[0] || "",
      modelFailureFallbackEnabled: departmentCanEnableModelFailureFallback(department) && department.modelFailureFallbackEnabled,
      childDepartmentIds: normalizeDepartmentChildIds(department.childDepartmentIds, departmentId),
      permissionControl: normalizePermissionControl(department.permissionControl),
    };
  });
}

async function saveDepartments() {
  if (!selectedDepartment.value || departmentValidationMessage.value) return;

  const previousDepartments = cloneDepartmentList(props.config.departments || []);
  const nextDrafts = cloneDepartmentList(departmentDrafts.value);
  const nextDepartments = applyUpdatedAtToChangedDepartments(
    mergeDepartmentChildIdsFromSource(
      prepareDepartmentsForSave(nextDrafts),
      previousDepartments,
      removedDepartmentIdsFromSource(nextDrafts, previousDepartments),
    ),
    previousDepartments,
  );

  props.config.departments = nextDepartments;

  const saved = await Promise.resolve(props.saveConfigAction());
  if (!saved) {
    props.config.departments = previousDepartments;
    return;
  }

  syncDepartmentDraftsFromSource();
}

onMounted(() => {
  void loadPermissionCatalog();
});
</script>
