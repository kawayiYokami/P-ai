<template>
  <SettingsStickyLayout content-class="mx-auto max-w-5xl" header-class="pb-0">
    <template #header>
      <!-- 面包屑：常驻；一级仅「人格」，进入详情后在原位追加人格名 -->
      <div class="breadcrumbs mb-2.5 min-w-0 p-0 text-xl sm:mb-3">
        <ul class="flex flex-wrap items-center">
          <li v-if="inDetailMode">
            <a
              class="cursor-pointer py-1 text-base-content/50 transition-colors hover:text-base-content"
              :title="t('config.persona.backToList')"
              @click="backToList"
            >
              {{ t("config.tabs.persona") }}
            </a>
          </li>
          <li v-else class="py-1 font-semibold text-base-content">{{ t("config.tabs.persona") }}</li>
          <li v-if="inDetailMode" class="flex min-w-0 items-center gap-2 py-1">
            <span class="max-w-[14rem] truncate font-semibold text-base-content sm:max-w-xs">
              {{ selectedPersona?.name || t("config.persona.title") }}
            </span>
            <span
              v-if="selectedPersona && isPresetPersona(selectedPersona)"
              class="badge badge-neutral badge-xs shrink-0"
            >
              {{ t("config.persona.systemTag") }}
            </span>
            <span
              v-else-if="selectedPersonaIsPrivateWorkspace"
              class="badge badge-secondary badge-xs shrink-0"
            >
              {{ t("config.persona.privateWorkspaceTag") }}
            </span>
          </li>
        </ul>
      </div>

      <Transition name="ecall-config-content" mode="out-in">
        <!-- 二级详情模式头部：子菜单 Tab + 操作区 -->
        <div v-if="inDetailMode" key="detail" class="flex flex-wrap items-center justify-between gap-2 sm:gap-3">
          <!-- 人格子菜单 Tab：资料 / 随身技能 / 权限 / 委托人 -->
          <div role="tablist" class="tabs tabs-border">
            <button
              v-for="option in detailSubMenuOptions"
              :key="option.value"
              type="button"
              role="tab"
              class="tab h-10 gap-1.5 px-3 text-base"
              :class="detailView === option.value ? 'tab-active font-medium' : 'text-base-content/60 hover:text-base-content'"
              @click="onSubMenuTabChange(option.value)"
            >
              <span class="truncate">{{ option.label }}</span>
              <span
                v-if="option.badge"
                class="badge badge-xs font-mono"
                :class="detailView === option.value ? 'badge-neutral' : 'badge-ghost opacity-70'"
              >
                {{ option.badge }}
              </span>
            </button>
          </div>

          <div class="flex shrink-0 items-center gap-1.5 sm:gap-2">
            <!-- 放弃修改 / 还原草稿 -->
            <button
              v-if="personaDirty"
              class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1 px-2 sm:px-3 text-xs"
              type="button"
              :disabled="personaSaving"
              :title="t('config.persona.restoreDraft')"
              @click="$emit('resetPersonas')"
            >
              <RotateCcw class="h-4 w-4" />
              <span class="hidden sm:inline">{{ t("config.persona.restoreDraft") }}</span>
            </button>

            <!-- 保存按钮 -->
            <button
              class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3 sm:px-3.5"
              :class="personaDirty ? 'btn-primary shadow-sm' : 'bg-base-100'"
              type="button"
              :disabled="!selectedPersona || !personaDirty || personaSaving"
              :title="personaSaving ? t('config.persona.saving') : personaDirty ? t('common.save') : t('status.personaSaved')"
              @click="$emit('savePersonas')"
            >
              <span v-if="personaSaving" class="loading loading-spinner loading-xs"></span>
              <Save v-else class="h-4 w-4" />
              <span>{{ personaSaving ? t("config.persona.saving") : t("common.save") }}</span>
            </button>
          </div>
        </div>

        <!-- 一级概览模式头部：分类 Tab，下划线贴合头部分界线 -->
        <div v-else key="overview">
          <div role="tablist" class="tabs tabs-border">
            <button
              v-for="option in personaCategoryOptions"
              :key="option.value"
              type="button"
              role="tab"
              class="tab h-10 gap-1.5 px-3 text-base"
              :class="activeCategoryTab === option.value ? 'tab-active font-medium' : 'text-base-content/60 hover:text-base-content'"
              @click="activeCategoryTab = option.value"
            >
              <span class="truncate">{{ option.label }}</span>
            </button>
          </div>
        </div>
      </Transition>
    </template>

    <!-- 二级详情与三级子视图内容 -->
    <Transition name="ecall-config-content" mode="out-in">
    <div v-if="inDetailMode" key="detail" class="min-w-0 max-w-full">
      <div v-if="selectedPersona" class="grid gap-5 min-w-0 max-w-full">
        <Transition name="ecall-config-content" mode="out-in">
        <!-- 子视图：随身技能与上下文顺序 -->
        <PersonaInjectionTable
          v-if="detailView === 'injection'"
          key="injection"
          :persona="selectedPersona"
          :skills="capabilitySkills"
        />

        <!-- 子视图：工具与技能权限 -->
        <PersonaPermissionView
          v-else-if="detailView === 'permission'"
          key="permission"
          :persona="selectedPersona"
          :catalog="permissionCatalog"
          :loading="capabilityLoading"
          :load-error="capabilityLoadError"
        />

        <!-- 子视图：下级委托人 -->
        <PersonaDelegateView
          v-else-if="detailView === 'delegate'"
          key="delegate"
          :persona="selectedPersona"
          :personas="personas"
          :avatar-url-map="personaAvatarUrlMap"
          :save-relations="saveRelations"
          :set-status-action="setStatusAction"
        />

        <!-- 主资料视图：扁平流式设计，杜绝卡片套卡片 -->
        <div v-else key="profile" class="space-y-6 min-w-0 max-w-full">
          <!-- 1. 核心身份看板：头像 + 姓名输入 + 标签 -->
          <ConfigCard flush>
            <div class="flex items-center gap-3.5 sm:gap-4 p-4">
              <div class="relative shrink-0">
                <button
                  type="button"
                  class="avatar group relative flex h-14 w-14 cursor-pointer overflow-hidden rounded-full ring-2 ring-base-200 hover:ring-primary/50 transition-all"
                  :disabled="avatarSaving"
                  :title="avatarSaving ? t('config.persona.avatarSaving') : t('config.persona.editAvatar')"
                  @click="$emit('openAvatarEditor')"
                >
                  <img
                    v-if="selectedPersonaAvatarUrl"
                    :src="selectedPersonaAvatarUrl"
                    :alt="selectedPersona.name"
                    class="h-full w-full object-cover"
                  />
                  <div
                    v-else
                    class="flex h-full w-full items-center justify-center bg-primary/10 text-primary font-bold text-lg"
                  >
                    {{ avatarInitial(selectedPersona.name) }}
                  </div>
                  <div class="absolute inset-0 flex items-center justify-center bg-black/35 opacity-0 transition-opacity group-hover:opacity-100">
                    <Camera class="h-5 w-5 text-white" />
                  </div>
                </button>
                <div
                  class="absolute -bottom-0.5 -right-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-base-100 shadow ring-1 ring-base-200 pointer-events-none text-base-content/70"
                >
                  <Camera class="h-3 w-3" />
                </div>
              </div>

              <div class="min-w-0 flex-1 space-y-1.5">
                <div class="flex items-center gap-2">
                  <input
                    v-model="selectedPersona.name"
                    type="text"
                    class="input input-sm h-9 w-full max-w-xs px-2.5 font-semibold text-sm sm:text-base input-bordered focus:border-primary"
                    :placeholder="t('config.persona.name')"
                  />
                </div>
                <div class="flex flex-wrap items-center gap-1.5 text-xs">
                  <span v-if="isPresetPersona(selectedPersona)" class="badge badge-neutral badge-xs">
                    {{ t("config.persona.systemTag") }}
                  </span>
                  <span v-if="selectedPersonaIsPrivateWorkspace" class="badge badge-secondary badge-xs">
                    {{ t("config.persona.privateWorkspaceTag") }}
                  </span>
                  <button
                    v-if="selectedPersonaIsPrivateWorkspace"
                    type="button"
                    class="btn btn-xs min-h-[1.5rem] btn-outline gap-1"
                    :disabled="personaSaving"
                    @click="emitConvertPrivatePersona"
                  >
                    {{ t("config.persona.convertToPublic") }}
                  </button>
                  <span v-if="avatarError" class="text-error text-caption">{{ avatarError }}</span>
                </div>
              </div>
            </div>
          </ConfigCard>

          <!-- 2. 人设与提示词 -->
          <MarkdownEditor
            v-model="selectedPersona.systemPrompt"
            :title="t('config.persona.prompt')"
            :placeholder="selectedPersona.isBuiltInUser ? t('config.persona.userPlaceholder') : (selectedPersona.id === 'system-persona' ? t('config.persona.systemPlaceholder') : t('config.persona.assistantPlaceholder'))"
          >
            <template #actions>
              <button
                v-if="selectedPersonaIsPreset"
                type="button"
                class="btn btn-ghost btn-xs gap-1 text-base-content/70 hover:text-base-content"
                @click="restoreSelectedPersonaPreset"
              >
                <RotateCcw class="h-3.5 w-3.5" />
                <span>{{ t("config.persona.restoreInitial") }}</span>
              </button>
            </template>
          </MarkdownEditor>

          <!-- 3. 记忆设置 (对用户和系统角色隐藏) -->
          <div v-if="!selectedPersona.isBuiltInUser && !selectedPersona.isBuiltInSystem" class="space-y-2">
            <ConfigCard :title="t('config.persona.memorySettings')" flush>
              <div class="divide-y divide-base-200/60">
                <!-- 私有记忆开关 -->
                <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-3.5 sm:p-4 min-w-0">
                  <div class="min-w-0 flex-1">
                    <div class="text-sm font-medium text-base-content">{{ t("config.persona.privateMemory") }}</div>
                    <div class="mt-0.5 text-xs text-base-content/50 leading-relaxed">{{ t("config.persona.privateMemoryHint") }}</div>
                  </div>
                  <SegmentedControl
                    :model-value="!!selectedPersona.privateMemoryEnabled"
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
                    <Upload class="h-4 w-4" />
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
        </div>
        </Transition>
      </div>
    </div>

    <!-- 一级概览模式内容：响应式网格 (手机单列，桌面双列) -->
    <div v-else key="overview" class="flex flex-col gap-4">
      <!-- 空状态（自定义分类的入口由网格末尾的新增卡承担） -->
      <div v-if="displayedPersonas.length === 0 && activeCategoryTab !== 'custom'" class="card border border-dashed border-base-300 bg-base-100 py-12">
        <div class="card-body items-center justify-center text-center">
          <User class="h-10 w-10 opacity-30" />
          <h3 class="text-sm font-medium opacity-70">
            {{ t("config.persona.noPresetPersonas") }}
          </h3>
        </div>
      </div>

      <!-- 人格卡片网格 -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5 sm:gap-3">
        <div
          v-for="persona in displayedPersonas"
          :key="persona.id"
          role="button"
          tabindex="0"
          class="group rounded-box border border-base-300 bg-base-100 p-3.5 sm:p-4 hover:border-primary/50 hover:bg-base-content/[0.02] transition-all cursor-pointer flex flex-col justify-between gap-2.5 select-none active:scale-[0.99]"
          :class="persona.id === selectedPersona?.id ? 'ring-1 ring-primary/40 border-primary/40' : ''"
          @click="enterPersona(persona)"
          @keydown.enter.prevent="enterPersona(persona)"
          @keydown.space.prevent="enterPersona(persona)"
        >
          <!-- 头部：头像 + 姓名/标识 + 标签 + 删除操作 -->
          <div class="flex items-start justify-between gap-2.5 min-w-0">
            <div class="flex items-center gap-2.5 min-w-0 flex-1">
              <div class="avatar shrink-0">
                <div class="w-10 h-10 sm:w-11 sm:h-11 rounded-full ring-1 ring-base-200 overflow-hidden">
                  <img
                    v-if="resolveAvatarUrl(persona)"
                    :src="resolveAvatarUrl(persona)"
                    :alt="persona.name"
                    class="w-full h-full object-cover rounded-full"
                  />
                  <div
                    v-else
                    class="flex h-full w-full items-center justify-center rounded-full bg-primary/10 text-primary font-bold text-sm"
                  >
                    {{ avatarInitial(persona.name) }}
                  </div>
                </div>
              </div>

              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <span class="font-semibold text-sm truncate group-hover:text-primary transition-colors">
                    {{ persona.name }}
                  </span>
                  <span v-if="persona.isBuiltInUser" class="badge badge-info badge-xs shrink-0 font-medium">
                    {{ t("config.persona.userTag") }}
                  </span>
                  <span v-else-if="isPresetPersona(persona)" class="badge badge-neutral badge-xs shrink-0 font-medium">
                    {{ t("config.persona.systemTag") }}
                  </span>
                </div>

                <!-- 角色标识 -->
                <div class="mt-0.5 flex flex-wrap items-center gap-1">
                  <span
                    v-if="persona.id === 'default-agent'"
                    class="badge badge-primary badge-outline badge-xs text-caption"
                  >
                    {{ t("config.persona.defaultAgent") }}
                  </span>
                  <span
                    v-else-if="persona.id === 'deputy-agent'"
                    class="badge badge-secondary badge-outline badge-xs text-caption"
                  >
                    {{ t("config.persona.deputyAgent") }}
                  </span>
                  <span
                    v-else-if="persona.id === 'system-persona'"
                    class="badge badge-neutral badge-outline badge-xs text-caption"
                  >
                    {{ t("config.persona.systemPersona") }}
                  </span>
                  <span
                    v-else-if="persona.id === 'user-persona' || persona.isBuiltInUser"
                    class="badge badge-info badge-outline badge-xs text-caption"
                  >
                    {{ t("config.persona.userPersona") }}
                  </span>
                </div>
              </div>
            </div>

            <!-- 操作区：删除按钮 (移动端直接可见，桌面端 hover 显示) + 箭头 -->
            <div class="flex items-center gap-1 shrink-0">
              <button
                v-if="canDeletePersona(persona)"
                type="button"
                class="btn btn-ghost btn-xs btn-circle text-base-content/40 hover:text-error hover:bg-error/10 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity"
                :title="t('config.persona.remove')"
                @click.stop="promptDelete(persona)"
              >
                <Trash2 class="h-3.5 w-3.5" />
              </button>
              <ChevronRight class="h-4 w-4 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all text-base-content" />
            </div>
          </div>

          <!-- 中部：Prompt 预览 -->
          <p class="text-xs text-base-content/65 line-clamp-2 leading-relaxed min-h-[2.25rem] break-words">
            <InlineMarkdownText :text="persona.systemPrompt?.trim() || t('config.persona.noPrompt')" />
          </p>

          <!-- 底栏：特性指示 (若有) -->
          <div
            v-if="persona.privateMemoryEnabled || (persona.memoryRecallMode && persona.memoryRecallMode !== 'auto')"
            class="flex items-center gap-1.5 border-t border-base-200/60 pt-2 text-caption opacity-80"
          >
            <span v-if="persona.privateMemoryEnabled" class="badge badge-xs badge-accent badge-outline text-caption">
              {{ t("config.persona.privateMemory") }}
            </span>
            <span v-if="persona.memoryRecallMode && persona.memoryRecallMode !== 'auto'" class="badge badge-xs badge-ghost text-caption">
              {{ recallModeLabel(persona.memoryRecallMode) }}
            </span>
          </div>
        </div>

        <!-- 新增人格卡：仅自定义分类，作为网格末位入口 -->
        <button
          v-if="activeCategoryTab === 'custom'"
          type="button"
          class="flex min-h-[7.5rem] flex-col items-center justify-center gap-1.5 rounded-box border border-dashed border-base-300 bg-base-100 p-3.5 text-base-content/50 transition-all hover:border-primary/50 hover:text-primary sm:p-4"
          @click="onAddPersonaClick"
        >
          <Plus class="h-5 w-5" />
          <span class="text-sm font-medium">{{ t("config.persona.add") }}</span>
        </button>
      </div>
    </div>
    </Transition>
  </SettingsStickyLayout>

  <!-- 删除人格确认对话框 -->
  <dialog ref="deleteDialogRef" class="modal">
    <div class="modal-box max-w-sm">
      <h3 class="text-sm font-semibold mb-2 text-error flex items-center gap-2">
        <Trash2 class="h-4 w-4" />
        {{ t("config.persona.deletePersona") }}
      </h3>
      <p class="text-sm text-base-content/80">
        {{ t("config.persona.deleteConfirm", { name: pendingDeletePersona?.name || "" }) }}
      </p>
      <div class="modal-action">
        <button class="btn btn-sm min-h-[2.25rem]" type="button" @click="cancelDelete">
          {{ t("common.cancel") }}
        </button>
        <button class="btn btn-sm min-h-[2.25rem] btn-error" type="button" @click="confirmDelete">
          {{ t("common.delete") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="cancelDelete">close</button>
    </form>
  </dialog>

  <!-- 私有记忆关闭对话框 -->
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
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Camera,
  ChevronRight,
  Plus,
  RotateCcw,
  Save,
  Trash2,
  Upload,
  User,
} from "@lucide/vue";
import type {
  MemoryRecallMode,
  PermissionCatalog,
  PersonaProfile,
  SkillSummaryItem,
} from "../../../../types/app";
import {
  exportTransportAgentPrivateMemories,
  invokeTauri,
} from "../../../../services/tauri-api";
import ConfigCard from "../../components/ConfigCard.vue";
import SegmentedControl from "../../components/SegmentedControl.vue";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import MarkdownEditor from "../../components/MarkdownEditor.vue";
import InlineMarkdownText from "../../../chat/markdown/InlineMarkdownText.vue";
import PersonaInjectionTable from "./persona-capability/PersonaInjectionTable.vue";
import PersonaPermissionView from "./persona-capability/PersonaPermissionView.vue";
import PersonaDelegateView from "./persona-capability/PersonaDelegateView.vue";
import { normalizePermissionCatalog } from "../../utils/permission-tree";

const props = withDefaults(defineProps<{
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  personaEditorId: string;
  selectedPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  personaAvatarUrlMap?: Record<string, string>;
  avatarSaving: boolean;
  avatarError: string;
  personaSaving: boolean;
  personaDirty: boolean;
  configSaving: boolean;
  saveRelations?: (updates: { agentId: string; childAgentIds: string[] }[]) => Promise<boolean>;
  setStatusAction?: (message: string) => void;
}>(), {
  personaAvatarUrlMap: () => ({}),
  saveRelations: undefined,
  setStatusAction: undefined,
});

const emit = defineEmits<{
  (e: "update:personaEditorId", value: string): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "resetPersonas"): void;
  (e: "openAvatarEditor"): void;
  (e: "importPersonaMemories", value: { agentId: string; file: File }): void;
  (e: "savePersonas"): void;
  (e: "convertPrivatePersonaToPublic", agentId: string): void;
}>();

const { t } = useI18n();

const inDetailMode = ref(false);

// 二级详情视图：资料主页（默认）/ 随身技能与顺序 / 权限控制 / 委托人
type PersonaDetailView = "profile" | "injection" | "permission" | "delegate";
const detailView = ref<PersonaDetailView>("profile");

const capabilitySkills = ref<SkillSummaryItem[]>([]);
const permissionCatalog = ref<PermissionCatalog>({ builtinTools: [], skills: [], mcpTools: [] });
const capabilityLoading = ref(false);
const capabilityLoadError = ref("");
let capabilityLoaded = false;
// 预览口注入数据后置真，用于丢弃仍在途的自动取数结果，避免覆盖注入值。
let capabilityDataLocked = false;

async function loadCapabilityData() {
  if (capabilityDataLocked || capabilityLoaded || capabilityLoading.value) return;
  capabilityLoading.value = true;
  capabilityLoadError.value = "";
  try {
    const [skillResult, catalog] = await Promise.all([
      invokeTauri<{ skills?: SkillSummaryItem[] }>("mcp_list_skills"),
      invokeTauri<PermissionCatalog>("list_permission_catalog"),
    ]);
    if (capabilityDataLocked) return;
    capabilitySkills.value = (skillResult?.skills || []).filter((item) => String(item?.name || "").trim());
    permissionCatalog.value = normalizePermissionCatalog(catalog);
    capabilityLoaded = true;
  } catch (error) {
    if (capabilityDataLocked) return;
    capabilitySkills.value = [];
    permissionCatalog.value = { builtinTools: [], skills: [], mcpTools: [] };
    capabilityLoadError.value = error instanceof Error ? error.message : String(error);
  } finally {
    capabilityLoading.value = false;
  }
}

function backToList() {
  inDetailMode.value = false;
  detailView.value = "profile";
}

// 预览/测试用：允许外部（demo 页）注入能力数据并直接进入详情，不改变正式运行路径。
function previewSetCapabilityData(skills: SkillSummaryItem[], catalog: PermissionCatalog) {
  capabilityDataLocked = true;
  capabilitySkills.value = (skills || []).filter((item) => String(item?.name || "").trim());
  permissionCatalog.value = normalizePermissionCatalog(catalog);
  capabilityLoaded = true;
  capabilityLoading.value = false;
  capabilityLoadError.value = "";
}

function previewOpenPersona(id: string) {
  const persona = props.personas.find((item) => String(item.id) === String(id));
  if (persona) enterPersona(persona);
}

function previewSetDetailView(view: PersonaDetailView) {
  detailView.value = view;
}

defineExpose({
  previewSetCapabilityData,
  previewOpenPersona,
  previewSetDetailView,
});

const detailSubMenuOptions = computed(() => {
  const residentCount = (props.selectedPersona?.residentSkillNames || []).filter(Boolean).length;
  const childCount = (props.selectedPersona?.childAgentIds || []).filter(Boolean).length;
  return [
    {
      value: "profile" as const,
      label: t("config.persona.profile"),
    },
    {
      value: "injection" as const,
      label: t("config.persona.skillsTab"),
      badge: residentCount > 0 ? residentCount : undefined,
    },
    {
      value: "permission" as const,
      label: t("config.persona.permission.title"),
    },
    {
      value: "delegate" as const,
      label: t("config.persona.delegate.title"),
      badge: childCount > 0 ? childCount : undefined,
    },
  ];
});

function onSubMenuTabChange(val: PersonaDetailView) {
  detailView.value = val;
  if (val === "injection" || val === "permission") {
    void loadCapabilityData();
  }
}



onMounted(() => {
  if (props.selectedPersona) void loadCapabilityData();
});

type PersonaCategoryTab = "custom" | "preset";
const activeCategoryTab = ref<PersonaCategoryTab>("custom");

function isPresetPersona(persona: PersonaProfile | null | undefined): boolean {
  const id = String(persona?.id || "").trim();
  if (!id) return false;
  return id === "default-agent"
    || id === "deputy-agent"
    || id === "user-persona"
    || id === "system-persona"
    // 内置组织人格：出厂预设、只读（不可删）。与 default-agent 同列，
    // 靠 id 名单判定，不带系统标记（「内置」与「系统」是两件事）。
    || id === "reviewer"
    || id === "support"
    || !!persona?.isBuiltInUser
    || !!persona?.isBuiltInSystem;
}

const customPersonas = computed(() =>
  props.personas.filter((p) => !isPresetPersona(p)),
);

const presetPersonas = computed(() =>
  sortPersonasForSelect(props.personas.filter((p) => isPresetPersona(p))),
);

const personaCategoryOptions = computed(() => [
  {
    value: "custom" as const,
    label: t("config.persona.customPersonas"),
  },
  {
    value: "preset" as const,
    label: t("config.persona.presetPersonas"),
  },
]);

const displayedPersonas = computed(() =>
  activeCategoryTab.value === "custom" ? customPersonas.value : presetPersonas.value,
);

function resolveAvatarUrl(persona: PersonaProfile): string {
  if (persona.id === props.selectedPersona?.id && props.selectedPersonaAvatarUrl) {
    return props.selectedPersonaAvatarUrl;
  }
  return props.personaAvatarUrlMap?.[persona.id] || "";
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function canDeletePersona(persona: PersonaProfile | null | undefined): boolean {
  if (!persona) return false;
  return !isPresetPersona(persona) && props.assistantPersonas.length > 1;
}

function recallModeLabel(mode?: string): string {
  if (mode === "manual") return t("config.persona.memoryRecallManual");
  if (mode === "off") return t("config.persona.memoryRecallOff");
  return t("config.persona.memoryRecallAuto");
}

const deleteDialogRef = ref<HTMLDialogElement | null>(null);
const pendingDeletePersona = ref<PersonaProfile | null>(null);

function promptDelete(persona: PersonaProfile) {
  pendingDeletePersona.value = persona;
  deleteDialogRef.value?.showModal();
}

function cancelDelete() {
  deleteDialogRef.value?.close();
  pendingDeletePersona.value = null;
}

function confirmDelete() {
  if (!pendingDeletePersona.value) return;
  emit("update:personaEditorId", pendingDeletePersona.value.id);
  emit("removeSelectedPersona");
  deleteDialogRef.value?.close();
  pendingDeletePersona.value = null;
  inDetailMode.value = false;
}

function enterPersona(persona: PersonaProfile) {
  emit("update:personaEditorId", persona.id);
  detailView.value = "profile";
  inDetailMode.value = true;
  void loadCapabilityData();
}

function onAddPersonaClick() {
  activeCategoryTab.value = "custom";
  emit("addPersona");
  detailView.value = "profile";
  inDetailMode.value = true;
}

const privateMemoryModeOptions = computed(() => [
  { value: false, label: t("config.persona.global") },
  { value: true, label: t("config.persona.private") },
]);

const memoryRecallModeOptions = computed(() => [
  { value: "auto" as MemoryRecallMode, label: t("config.persona.memoryRecallAuto") },
  { value: "manual" as MemoryRecallMode, label: t("config.persona.memoryRecallManual") },
  { value: "off" as MemoryRecallMode, label: t("config.persona.memoryRecallOff") },
]);

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

const selectedPersonaIsPreset = computed(
  () => isPresetPersona(props.selectedPersona),
);

const selectedPersonaIsPrivateWorkspace = computed(
  () => props.selectedPersona?.source === "private_workspace",
);

function emitConvertPrivatePersona() {
  const agentId = props.selectedPersona?.id;
  if (!agentId || !selectedPersonaIsPrivateWorkspace.value) return;
  emit("convertPrivatePersonaToPublic", agentId);
}

const selectedPersonaMemoryRecallMode = computed(() =>
  normalizeMemoryRecallMode(props.selectedPersona?.memoryRecallMode),
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

type PersonaDefaultSeed = Pick<PersonaProfile, "systemPrompt">;

function normalizeMemoryRecallMode(value: unknown): MemoryRecallMode {
  const raw = String(value || "").trim();
  if (raw === "manual" || raw === "off") return raw;
  return "auto";
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

function personaDefaultSeed(persona: PersonaProfile | null | undefined): PersonaDefaultSeed | null {
  const id = String(persona?.id || "").trim();
  if (!id) return null;
  if (id === "default-agent") {
    return {
      systemPrompt: "你是谁：你是助理，是用户默认会先对话的助手。\n台词技巧：表达自然、直接、有人味；先给结论，再补必要说明；少空话，少套话。\n性格画像：耐心、友善、靠谱、利落。",
    };
  }
  if (id === "user-persona" || persona?.isBuiltInUser) {
    return {
      systemPrompt: "我是...",
    };
  }
  if (id === "deputy-agent") {
    return {
      systemPrompt: "你是谁：你是副手，是一个偏执行、偏推进的助手分身。\n台词技巧：短句作答，直给重点，少铺垫，少客套。\n性格画像：简洁、干脆、克制、利落。",
    };
  }
  if (id === "system-persona") {
    return {
      systemPrompt: "你是谁：你是 pai system，是系统消息与状态播报使用的人格。\n台词技巧：用词明确、稳定、客观，像系统通知，不抒情，不延展。\n性格画像：冷静、克制、严谨。",
    };
  }
  const builtInOrganizationPrompts: Record<string, string> = {
    reviewer: "你是谁：你是 reviewer，负责对已完成的实现做独立审查，只报告真实、可复现、影响正确性/稳定性/安全的缺陷。详细职责见你的常驻 skill。\n台词技巧：先列问题再下判断；有证据才说，没有就说没有。\n性格画像：严谨、克制、就事论事。",
    support: "你是谁：你是 support，负责远程客服场景的应答，处理外部联系人的咨询与消息。详细职责见你的常驻 skill。\n台词技巧：礼貌、清楚、直接回应对方诉求，不寒暄过度。\n性格画像：耐心、稳妥、有服务意识。",
  };
  if (builtInOrganizationPrompts[id]) {
    return { systemPrompt: builtInOrganizationPrompts[id] };
  }
  return null;
}

function restoreSelectedPersonaPreset() {
  if (!selectedPersonaIsPreset.value) return;
  const target = props.selectedPersona;
  const defaults = personaDefaultSeed(target);
  if (!target || !defaults) return;
  target.systemPrompt = defaults.systemPrompt;
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
  const agentId = props.selectedPersona?.id;
  if (!agentId) return;
  emit("importPersonaMemories", { agentId, file });
}

async function setPrivateMemoryMode(enabled: boolean) {
  const agentId = props.selectedPersona?.id;
  if (!agentId) return;
  const current = !!props.selectedPersona?.privateMemoryEnabled;
  if (current === enabled) return;
  privateMemoryError.value = "";
  if (enabled) {
    privateMemorySwitching.value = true;
    try {
      await invokeTauri("set_agent_private_memory_enabled", {
        input: { agentId, enabled: true },
      });
      if (props.selectedPersona) props.selectedPersona.privateMemoryEnabled = true;
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
  const agentId = props.selectedPersona?.id;
  if (!agentId) return;
  const nextMode = normalizeMemoryRecallMode(mode);
  const current = normalizeMemoryRecallMode(props.selectedPersona?.memoryRecallMode);
  if (current === nextMode) return;
  privateMemoryError.value = "";
  memoryRecallModeSwitching.value = true;
  try {
    const result = await invokeTauri<{ agentId: string; mode: MemoryRecallMode }>("set_agent_memory_recall_mode", {
      input: { agentId, mode: nextMode },
    });
    if (props.selectedPersona) {
      props.selectedPersona.memoryRecallMode = normalizeMemoryRecallMode(result.mode);
    }
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
    const persona = props.personas.find((p) => p.id === agentId);
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
