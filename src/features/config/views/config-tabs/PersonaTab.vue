<template>
  <SettingsStickyLayout>
    <template #header>
      <Transition name="fade" mode="out-in">
        <!-- 二级详情模式头部：面包屑导航 + 返回按钮 + 操作区 -->
        <div v-if="inDetailMode" key="detail" class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <button
              class="btn btn-ghost btn-sm min-h-[2.25rem] gap-1.5 px-2.5"
              type="button"
              :title="detailReturnTitle"
              @click="handleDetailBack"
            >
              <ArrowLeft class="h-4 w-4" />
              <span class="text-xs">{{ detailReturnTitle }}</span>
            </button>
            <div class="divider divider-horizontal my-1 py-0 opacity-40"></div>
            <div class="breadcrumbs p-0 text-xs">
              <ul>
                <li>
                  <button
                    type="button"
                    class="link link-hover font-normal opacity-70 hover:opacity-100"
                    @click="backToList"
                  >
                    {{ t("config.persona.overview") }}
                  </button>
                </li>
                <li>
                  <button
                    type="button"
                    class="link link-hover font-normal opacity-70 hover:opacity-100"
                    @click="backToList"
                  >
                    {{ isPresetPersona(selectedPersona) ? t("config.persona.presetPersonas") : t("config.persona.customPersonas") }}
                  </button>
                </li>
                <li>
                  <button
                    v-if="detailView !== 'profile'"
                    type="button"
                    class="link link-hover font-normal opacity-70 hover:opacity-100"
                    @click="backToProfile"
                  >
                    {{ selectedPersona?.name || t("config.persona.title") }}
                  </button>
                  <span v-else class="font-semibold text-base-content">
                    {{ selectedPersona?.name || t("config.persona.title") }}
                  </span>
                </li>
                <li v-if="detailView !== 'profile'" class="font-semibold text-base-content">
                  {{ detailViewLabel }}
                </li>
              </ul>
            </div>
          </div>

          <div class="flex items-center gap-2">
            <button
              v-if="personaDirty"
              class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1.5 px-3"
              type="button"
              :disabled="personaSaving"
              @click="$emit('resetPersonas')"
            >
              <RotateCcw class="h-4 w-4" />
              <span>{{ t("config.persona.restoreDraft") }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3.5"
              :class="personaDirty ? 'btn-primary' : 'bg-base-100'"
              type="button"
              :disabled="!selectedPersona || !personaDirty || personaSaving"
              :title="personaSaving ? t('config.persona.saving') : personaDirty ? t('common.save') : t('status.personaSaved')"
              @click="$emit('savePersonas')"
            >
              <span v-if="personaSaving" class="loading loading-spinner loading-xs"></span>
              <Save v-else class="h-4 w-4" />
              <span>{{ personaSaving ? t("config.persona.saving") : t("common.save") }}</span>
            </button>
            <button
              v-if="selectedPersona && canDeletePersona(selectedPersona)"
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-error gap-1.5 px-3"
              type="button"
              :title="t('config.persona.remove')"
              @click="promptDelete(selectedPersona)"
            >
              <Trash2 class="h-4 w-4" />
              <span>{{ t("common.delete") }}</span>
            </button>
          </div>
        </div>

        <!-- 一级概览模式头部：分类筛选（自定义人格 vs 系统预设）+ 搜索过滤 + 新增操作 -->
        <div v-else key="overview" class="flex flex-col gap-3">
          <SegmentedControl
            :model-value="activeCategoryTab"
            :options="personaCategoryOptions"
            size="md"
            @change="(val) => { activeCategoryTab = val; searchQuery = ''; }"
          />

          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="relative min-w-[14rem] flex-1">
              <input
                v-model="searchQuery"
                type="text"
                class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
                :placeholder="t('config.persona.searchPlaceholder')"
              />
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
              <button
                v-if="searchQuery"
                type="button"
                class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
                :title="t('config.persona.clearSearch')"
                @click="searchQuery = ''"
              >
                ✕
              </button>
            </div>

            <div class="flex items-center gap-2">
              <button
                v-if="activeCategoryTab === 'custom'"
                class="btn btn-sm min-h-[2.25rem] btn-primary gap-1.5 px-3.5"
                type="button"
                @click="onAddPersonaClick"
              >
                <Plus class="h-4 w-4" />
                <span>{{ t("config.persona.add") }}</span>
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </template>

    <!-- 二级详情模式内容 -->
    <div v-if="inDetailMode">
      <div v-if="selectedPersona" class="grid gap-3">
        <PersonaCapabilityOverview
          :persona="selectedPersona"
          :mcp-server-name-by-id="mcpServerNameById"
          :loading="capabilityLoading"
          @open="openCapabilityView"
        />
        <ConfigTemplate v-if="detailView === 'profile'" :model-value="templateValues" :groups="templateGroups">
          <template #row-persona-name>
            <div class="flex min-w-0 flex-wrap items-center gap-3">
              <div class="shrink-0 text-sm font-medium">{{ t('config.persona.name') }}</div>
              <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                <input v-model="selectedPersona.name" class="input input-bordered input-sm w-52 max-w-full shrink-0" :placeholder="t('config.persona.name')" />
                <span v-if="isPresetPersona(selectedPersona)" class="badge badge-neutral shrink-0">{{ t("config.persona.systemTag") }}</span>
                <span v-if="selectedPersonaIsPrivateWorkspace" class="badge badge-secondary shrink-0">{{ t("config.persona.privateWorkspaceTag") }}</span>
                <button
                  v-if="selectedPersonaIsPrivateWorkspace"
                  class="btn btn-sm min-h-[2.25rem] btn-outline shrink-0 gap-1.5"
                  type="button"
                  :disabled="personaSaving"
                  @click="emitConvertPrivatePersona"
                >
                  {{ t("config.persona.convertToPublic") }}
                </button>
              </div>
            </div>
          </template>

          <template #row-persona-avatar>
            <div class="grid min-w-0 gap-2">
              <div class="flex items-center justify-between gap-4">
                <div class="text-sm font-medium">{{ t('config.persona.avatar') }}</div>
                <button
                  class="btn btn-ghost btn-circle h-12 w-12 min-h-[3rem] shrink-0 p-0 hover:ring-2 hover:ring-primary/40"
                  :disabled="avatarSaving"
                  :title="avatarSaving ? t('config.persona.avatarSaving') : t('config.persona.editAvatar')"
                  @click="$emit('openAvatarEditor')"
                >
                  <div v-if="selectedPersonaAvatarUrl" class="avatar">
                    <div class="w-11 rounded-full">
                      <img :src="selectedPersonaAvatarUrl" :alt="selectedPersona.name" :title="selectedPersona.name" />
                    </div>
                  </div>
                  <div v-else class="avatar placeholder">
                    <div class="w-11 rounded-full bg-neutral text-neutral-content font-bold">
                      <span>{{ avatarInitial(selectedPersona.name) }}</span>
                    </div>
                  </div>
                </button>
              </div>
              <div v-if="avatarError" class="break-all text-error">{{ avatarError }}</div>
            </div>
          </template>

          <template #row-persona-prompt>
            <div class="grid min-w-0 gap-3">
              <div class="flex items-center justify-between gap-3">
                <div class="text-sm font-medium">{{ t('config.persona.prompt') }}</div>
                <button v-if="selectedPersonaIsPreset" class="btn btn-ghost btn-sm min-h-[2rem] gap-2" @click="restoreSelectedPersonaPreset">
                  <RotateCcw class="h-4 w-4" />
                  {{ t("config.persona.restoreInitial") }}
                </button>
              </div>
              <MarkdownEditor
                v-model="selectedPersona.systemPrompt"
                :placeholder="selectedPersona.isBuiltInUser ? t('config.persona.userPlaceholder') : (selectedPersona.id === 'system-persona' ? t('config.persona.systemPlaceholder') : t('config.persona.assistantPlaceholder'))"
              />
            </div>
          </template>

          <template #row-private-memory>
            <div class="grid min-w-0 gap-2">
              <div>
                <div class="text-sm">{{ t('config.persona.privateMemory') }}</div>
                <div class="mt-1 text-xs leading-snug text-base-content/60">{{ t('config.persona.privateMemoryHint') }}</div>
              </div>
              <SegmentedControl
                :model-value="!!selectedPersona.privateMemoryEnabled"
                :options="privateMemoryModeOptions"
                :disabled="privateMemoryCounting || privateMemorySwitching"
                size="sm"
                @change="setPrivateMemoryMode"
              />
            </div>
          </template>

          <template #row-memory-recall-mode>
            <div class="grid min-w-0 gap-2">
              <div>
                <div class="text-sm">{{ t('config.persona.memoryRecallMode') }}</div>
                <div class="mt-1 text-xs leading-snug text-base-content/60">{{ memoryRecallModeHint }}</div>
              </div>
              <SegmentedControl
                :model-value="selectedPersonaMemoryRecallMode"
                :options="memoryRecallModeOptions"
                :disabled="memoryRecallModeSwitching"
                size="sm"
                @change="setMemoryRecallMode"
              />
            </div>
          </template>

          <template #row-memory-import>
            <div class="flex min-w-0 items-center justify-between gap-4">
              <div class="text-sm">{{ t('config.persona.import') }}</div>
              <button class="btn btn-sm min-h-[2rem] btn-ghost shrink-0" @click="triggerPersonaMemoryImport" :title="t('config.persona.import')">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>
                {{ t('config.persona.import') }}
              </button>
            </div>
          </template>
        </ConfigTemplate>

        <PersonaSkillView
          v-else-if="detailView === 'skills'"
          :persona="selectedPersona"
          :skills="capabilitySkills"
          :loading="capabilityLoading"
        />

        <PersonaToolView
          v-else-if="detailView === 'tools'"
          :persona="selectedPersona"
          :builtin-tools="capabilityBuiltinTools"
          :servers="capabilityServers"
          :loading="capabilityLoading"
        />

        <div v-if="detailView === 'profile' && !selectedPersona.isBuiltInUser && !selectedPersona.isBuiltInSystem && privateMemoryError" class="text-sm text-error">
          {{ privateMemoryError }}
        </div>

        <input
          v-if="detailView === 'profile'"
          ref="personaMemoryImportInput"
          type="file"
          accept=".json,application/json"
          class="hidden"
          @change="onPersonaMemoryImportFile"
        />
      </div>
    </div>

    <!-- 一级概览模式内容：2 列卡片矩阵（保持与供应商和连接器统一的列宽与间距） -->
    <div v-else class="flex flex-col gap-4">
      <!-- 空状态 -->
      <div v-if="displayedPersonas.length === 0" class="card border border-dashed border-base-300 bg-base-100 py-12">
        <div class="card-body items-center justify-center text-center">
          <User class="h-10 w-10 opacity-30" />
          <h3 class="text-sm font-medium opacity-70">
            {{ searchQuery ? t("config.persona.noPersonas") : (activeCategoryTab === 'custom' ? t("config.persona.noCustomPersonas") : t("config.persona.noPresetPersonas")) }}
          </h3>
          <p class="text-xs opacity-50">
            {{ searchQuery ? t("config.persona.clearSearch") : (activeCategoryTab === 'custom' ? t("config.persona.noCustomPersonasHint") : "") }}
          </p>
          <div class="card-actions mt-3">
            <button
              v-if="searchQuery"
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-xs"
              type="button"
              @click="searchQuery = ''"
            >
              {{ t("config.persona.clearSearch") }}
            </button>
            <button
              v-else-if="activeCategoryTab === 'custom'"
              class="btn btn-sm min-h-[2.25rem] btn-primary text-xs"
              type="button"
              @click="onAddPersonaClick"
            >
              <Plus class="h-4 w-4" />
              <span>{{ t("config.persona.add") }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- 自适应卡片网格 -->
      <div v-else class="config-grid-auto-md">
        <div
          v-for="persona in displayedPersonas"
          :key="persona.id"
          role="button"
          tabindex="0"
          class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          :class="persona.id === selectedPersona?.id ? 'ring-1 ring-primary/40 border-primary/40' : ''"
          @click="enterPersona(persona)"
          @keydown.enter.prevent="enterPersona(persona)"
          @keydown.space.prevent="enterPersona(persona)"
        >
          <!-- 头部：头像 + 姓名/标识 + 标签 + 删除操作 -->
          <div class="flex items-start justify-between gap-2.5 min-w-0">
            <div class="flex items-center gap-2.5 min-w-0 flex-1">
              <div class="avatar shrink-0">
                <div class="w-11 h-11 rounded-full ring-1 ring-base-200 overflow-hidden">
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
                <div class="mt-1 flex flex-wrap items-center gap-1">
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

            <!-- 卡片右上角独立删除按钮（仅限自定义人格） -->
            <button
              v-if="canDeletePersona(persona)"
              type="button"
              class="btn btn-ghost btn-xs btn-circle opacity-0 group-hover:opacity-100 hover:text-error transition-opacity shrink-0"
              :title="t('config.persona.remove')"
              @click.stop="promptDelete(persona)"
            >
              <Trash2 class="h-4 w-4" />
            </button>
          </div>

          <!-- 中部：Prompt 预览（简单 Markdown：行内格式 + 标题加粗） -->
          <p class="text-xs text-base-content/70 line-clamp-2 leading-relaxed min-h-[2.5rem] break-words">
            <InlineMarkdownText :text="persona.systemPrompt?.trim() || t('config.persona.noPrompt')" />
          </p>

          <!-- 底栏：记忆特性标签 + 进入提示 -->
          <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption opacity-70">
            <div class="flex items-center gap-1.5">
              <span v-if="persona.privateMemoryEnabled" class="badge badge-sm badge-accent badge-outline text-caption">
                {{ t("config.persona.privateMemory") }}
              </span>
              <span v-if="persona.memoryRecallMode && persona.memoryRecallMode !== 'auto'" class="badge badge-sm badge-ghost text-caption">
                {{ recallModeLabel(persona.memoryRecallMode) }}
              </span>
            </div>
            <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all ml-auto" />
          </div>
        </div>
      </div>
    </div>
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
import { ArrowLeft, ChevronRight, Plus, RotateCcw, Save, Search, Trash2, User } from "@lucide/vue";
import type { FrontendToolDefinition, McpServerConfig, MemoryRecallMode, PersonaProfile, SkillSummaryItem } from "../../../../types/app";
import { exportTransportAgentPrivateMemories, invokeTauri } from "../../../../services/tauri-api";
import SegmentedControl from "../../components/SegmentedControl.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import MarkdownEditor from "../../components/MarkdownEditor.vue";
import InlineMarkdownText from "../../../chat/markdown/InlineMarkdownText.vue";
import PersonaCapabilityOverview from "./persona-capability/PersonaCapabilityOverview.vue";
import PersonaSkillView from "./persona-capability/PersonaSkillView.vue";
import PersonaToolView from "./persona-capability/PersonaToolView.vue";

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
}>(), {
  personaAvatarUrlMap: () => ({}),
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
const searchQuery = ref("");

// 二级详情内的视图：资料（默认）/ 技能 / 工具。三级只负责改，概览留在二级。
type PersonaDetailView = "profile" | "skills" | "tools";
const detailView = ref<PersonaDetailView>("profile");

const capabilitySkills = ref<SkillSummaryItem[]>([]);
const capabilityServers = ref<McpServerConfig[]>([]);
const capabilityBuiltinTools = ref<FrontendToolDefinition[]>([]);
const capabilityLoading = ref(false);
let capabilityLoaded = false;

const mcpServerNameById = computed(() => {
  const map: Record<string, string> = {};
  for (const server of capabilityServers.value) {
    const id = String(server.id || "").trim();
    if (id) map[id] = String(server.name || "").trim() || id;
  }
  return map;
});

async function loadCapabilityData() {
  if (capabilityLoaded || capabilityLoading.value) return;
  capabilityLoading.value = true;
  try {
    const [skillResult, servers, builtinTools] = await Promise.all([
      invokeTauri<{ skills?: SkillSummaryItem[] }>("mcp_list_skills"),
      invokeTauri<McpServerConfig[]>("mcp_list_servers"),
      invokeTauri<FrontendToolDefinition[]>("list_tool_catalog"),
    ]);
    capabilitySkills.value = (skillResult?.skills || []).filter((item) => String(item?.name || "").trim());
    capabilityServers.value = Array.isArray(servers) ? servers : [];
    capabilityBuiltinTools.value = Array.isArray(builtinTools) ? builtinTools : [];
    capabilityLoaded = true;
  } catch {
    capabilitySkills.value = [];
    capabilityServers.value = [];
    capabilityBuiltinTools.value = [];
  } finally {
    capabilityLoading.value = false;
  }
}

function openCapabilityView(view: "skills" | "tools") {
  detailView.value = view;
  void loadCapabilityData();
}

function backToProfile() {
  detailView.value = "profile";
}

const detailReturnTitle = computed(() =>
  detailView.value === "profile"
    ? t("config.persona.backToList")
    : t("config.persona.backToProfile"),
);

const detailViewLabel = computed(() =>
  detailView.value === "tools"
    ? t("config.persona.capability.toolTitle")
    : t("config.persona.capability.skillTitle"),
);

function handleDetailBack() {
  if (detailView.value === "profile") {
    backToList();
  } else {
    backToProfile();
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
    || id === "saddler"
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
    badge: customPersonas.value.length,
  },
  {
    value: "preset" as const,
    label: t("config.persona.presetPersonas"),
    badge: presetPersonas.value.length,
  },
]);

const displayedPersonas = computed(() => {
  const sourceList = activeCategoryTab.value === "custom" ? customPersonas.value : presetPersonas.value;
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return sourceList;

  return sourceList.filter((p) => {
    const nameMatch = (p.name || "").toLowerCase().includes(q);
    const promptMatch = (p.systemPrompt || "").toLowerCase().includes(q);
    const idMatch = (p.id || "").toLowerCase().includes(q);
    return nameMatch || promptMatch || idMatch;
  });
});

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

function backToList() {
  inDetailMode.value = false;
  detailView.value = "profile";
}

function onAddPersonaClick() {
  activeCategoryTab.value = "custom";
  emit("addPersona");
  inDetailMode.value = true;
}

const templateValues = {};
const templateGroups = computed<ConfigTemplateGroup[]>(() => {
  const groups: ConfigTemplateGroup[] = [
    {
      key: "persona-settings",
      title: t("config.persona.settings"),
      rows: [
        { key: "persona-name", items: [] },
        { key: "persona-avatar", items: [] },
        { key: "persona-prompt", items: [] },
      ],
    },
  ];
  const persona = props.selectedPersona;
  if (persona && !persona.isBuiltInUser && !persona.isBuiltInSystem) {
    groups.push({
      key: "persona-memory",
      title: t("config.persona.memorySettings"),
      rows: [
        { key: "private-memory", items: [] },
        { key: "memory-recall-mode", items: [] },
        { key: "memory-import", items: [] },
      ],
    });
  }
  return groups;
});

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
    saddler: "你是谁：你是 saddler，专门在当前项目 `.pai/` 目录下生成和维护能力资产。详细职责见你的常驻 skill。\n台词技巧：说清写在哪、为什么这么定；不越界改业务代码。\n性格画像：细致、有规范意识、克制。",
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
