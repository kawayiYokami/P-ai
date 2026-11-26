<template>
  <SettingsPageShell
    :breadcrumb="personaBreadcrumb"
    :tabs="headerTabs"
    :tab="headerTab"
    @update:tab="onHeaderTabChange"
  >
    <template #actions>
      <div v-if="inDetailMode" class="flex shrink-0 items-center gap-1.5 sm:gap-2">
            <!-- 放弃修改 / 还原草稿 -->
            <button
              v-if="personaDirty"
              class="btn btn-sm btn-ghost gap-1.5 px-2.5 text-base-content/70 hover:text-base-content"
              type="button"
              :disabled="personaSaving"
              :title="t('config.persona.restoreDraft')"
              @click="$emit('resetPersonas')"
            >
              <AppIcon name="rotate-ccw" class="h-4 w-4" />
              <span class="hidden sm:inline">{{ t("config.persona.restoreDraft") }}</span>
            </button>

            <!-- 保存按钮 -->
            <button
              class="btn btn-sm btn-ghost gap-1.5 px-3"
              :class="personaDirty ? 'btn-primary shadow-sm' : 'bg-base-100'"
              type="button"
              :disabled="!selectedPersona || !personaDirty || personaSaving"
              :title="personaSaving ? t('config.persona.saving') : personaDirty ? t('common.save') : t('status.personaSaved')"
              @click="$emit('savePersonas')"
            >
              <span v-if="personaSaving" class="loading loading-spinner loading-xs"></span>
              <AppIcon name="save" v-else class="h-4 w-4" />
              <span>{{ personaSaving ? t("config.persona.saving") : t("common.save") }}</span>
            </button>
      </div>
    </template>

    <!-- 二级详情与三级子视图内容 -->
    <div v-if="inDetailMode" class="min-w-0 max-w-full">
      <div v-if="selectedPersona" class="grid gap-5 min-w-0 max-w-full">
        <!-- 子视图：工具与技能权限 -->
        <PersonaPermissionView
          v-if="detailView === 'permission'"
          key="permission"
          :persona="selectedPersona"
          :catalog="permissionCatalog"
          :loading="capabilityLoading"
          :load-error="capabilityLoadError"
        />

        <!-- 子视图：能力（委托模型 / 上下文注入与随身技能 / 私有记忆） -->
        <PersonaCapabilityTab
          v-else-if="detailView === 'capability'"
          key="capability"
          :persona="selectedPersona"
          :skills="capabilitySkills"
          :api-configs="props.textCapableApiConfigs"
          :saving="personaSaving"
          @import-persona-memories="emit('importPersonaMemories', $event)"
        />

        <!-- 人格视图：身份看板 + 人设 + 表情贴纸 -->
        <div v-else key="persona" class="space-y-6 min-w-0 max-w-full">
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
                    <AppIcon name="camera" class="h-5 w-5 text-white" />
                  </div>
                </button>
                <div
                  class="absolute -bottom-0.5 -right-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-base-100 shadow ring-1 ring-base-200 pointer-events-none text-base-content/70"
                >
                  <AppIcon name="camera" class="h-3 w-3" />
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
                <AppIcon name="rotate-ccw" class="h-3.5 w-3.5" />
                <span>{{ t("config.persona.restoreInitial") }}</span>
              </button>
            </template>
          </MarkdownEditor>

          <!-- 表情贴纸库：形象神态的延伸，跟着人格走，不再单独占一个 Tab -->
          <PersonaMemesTab :persona="selectedPersona" />

        </div>
      </div>
    </div>

    <!-- 一级概览模式内容：响应式网格 (手机单列，桌面双列) -->
    <div v-else key="overview" class="flex flex-col gap-4">
      <!-- 空状态（自定义分类的入口由网格末尾的新增卡承担） -->
      <div v-if="displayedPersonas.length === 0 && activeCategoryTab !== 'custom'" class="card border border-dashed border-base-300 bg-base-100 py-12">
        <div class="card-body items-center justify-center text-center">
          <AppIcon name="user" class="h-10 w-10 opacity-30" />
          <h3 class="text-sm font-medium opacity-70">
            {{ t("config.persona.noPresetPersonas") }}
          </h3>
        </div>
      </div>

      <!-- 人格卡片网格：与能力商店同一套自动列宽，卡片大小才对得齐 -->
      <div class="config-grid-auto-sm">
        <div
          v-for="persona in displayedPersonas"
          :key="persona.id"
          role="button"
          tabindex="0"
          class="group rounded-box border border-base-300 bg-base-100 p-4 hover:border-primary/50 hover:bg-base-content/[0.02] transition-all cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99]"
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
                <AppIcon name="trash" class="h-3.5 w-3.5" />
              </button>
              <AppIcon name="chevron-right" class="h-4 w-4 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all text-base-content" />
            </div>
          </div>

          <!-- 中部：Prompt 预览 -->
          <p class="text-xs text-base-content/65 line-clamp-2 leading-relaxed min-h-[2.25rem] break-words">
            <InlineMarkdownText :text="persona.systemPrompt?.trim() || t('config.persona.noPrompt')" />
          </p>
        </div>

        <!-- 新增人格卡：仅自定义分类，作为网格末位入口 -->
        <button
          v-if="activeCategoryTab === 'custom'"
          type="button"
          class="flex flex-col items-center justify-center gap-1.5 rounded-box border border-dashed border-base-300 bg-base-100 p-4 text-base-content/50 transition-all hover:border-primary/50 hover:text-primary"
          @click="onAddPersonaClick"
        >
          <AppIcon name="plus" class="h-5 w-5" />
          <span class="text-sm font-medium">{{ t("config.persona.add") }}</span>
        </button>
      </div>
    </div>
  </SettingsPageShell>

  <!-- 删除人格确认对话框 -->
  <dialog ref="deleteDialogRef" class="modal">
    <div class="modal-box max-w-sm">
      <h3 class="text-sm font-semibold mb-2 text-error flex items-center gap-2">
        <AppIcon name="trash" class="h-4 w-4" />
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

</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import type {
  ApiConfigItem,
  PermissionCatalog,
  PersonaProfile,
  SkillSummaryItem,
} from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import ConfigCard from "../../components/ConfigCard.vue";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbBadge, SettingsBreadcrumbItem } from "../../components/settings-breadcrumb-types";
import type { UnderlineTabItem } from "../../../shared/components/underline-tabs-types";
import MarkdownEditor from "../../components/MarkdownEditor.vue";
import InlineMarkdownText from "../../../chat/markdown/InlineMarkdownText.vue";
import PersonaCapabilityTab from "./persona-capability/PersonaCapabilityTab.vue";
import PersonaMemesTab from "./persona-capability/PersonaMemesTab.vue";
import PersonaPermissionView from "./persona-capability/PersonaPermissionView.vue";
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
  textCapableApiConfigs?: ApiConfigItem[];
  expertApiConfigId?: string;
  toolReviewApiConfigId?: string;
  setStatusAction?: (message: string) => void;
}>(), {
  personaAvatarUrlMap: () => ({}),
  textCapableApiConfigs: () => [],
  expertApiConfigId: "",
  toolReviewApiConfigId: "",
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
}>();

const { t } = useI18n();

const inDetailMode = ref(false);

// 二级详情视图：人格（默认）/ 能力 / 权限
type PersonaDetailView = "persona" | "capability" | "permission";
const detailView = ref<PersonaDetailView>("persona");

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
  detailView.value = "persona";
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
  return [
    {
      value: "persona" as const,
      label: t("config.persona.tabPersona"),
    },
    {
      value: "capability" as const,
      label: t("config.persona.tabCapability"),
    },
    {
      value: "permission" as const,
      label: t("config.persona.permission.title"),
    },
  ];
});

function onSubMenuTabChange(val: PersonaDetailView) {
  detailView.value = val;
  if (val === "capability" || val === "permission") {
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

const headerTabs = computed<UnderlineTabItem[]>(() =>
  inDetailMode.value
    ? detailSubMenuOptions.value.map((option) => ({ key: option.value, label: option.label }))
    : personaCategoryOptions.value.map((option) => ({ key: option.value, label: option.label })),
);

const headerTab = computed(() => (inDetailMode.value ? detailView.value : activeCategoryTab.value));

function onHeaderTabChange(key: string) {
  if (inDetailMode.value) {
    if (detailSubMenuOptions.value.some((option) => option.value === key)) {
      onSubMenuTabChange(key as PersonaDetailView);
    }
    return;
  }
  if (personaCategoryOptions.value.some((option) => option.value === key)) {
    activeCategoryTab.value = key as PersonaCategoryTab;
  }
}

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
  detailView.value = "persona";
  inDetailMode.value = true;
  void loadCapabilityData();
}

function onAddPersonaClick() {
  activeCategoryTab.value = "custom";
  emit("addPersona");
  detailView.value = "persona";
  inDetailMode.value = true;
}

const selectedPersonaIsPreset = computed(
  () => isPresetPersona(props.selectedPersona),
);

const personaBreadcrumb = computed<SettingsBreadcrumbItem[]>(() => {
  const persona = props.selectedPersona;
  if (!inDetailMode.value) return [{ label: t("config.tabs.persona") }];
  const badges: SettingsBreadcrumbBadge[] = [];
  if (persona && isPresetPersona(persona)) {
    badges.push({ text: t("config.persona.systemTag"), class: "badge-neutral" });
  }
  return [
    { label: t("config.tabs.persona"), title: t("config.persona.backToList"), onClick: backToList },
    { label: persona?.name || t("config.persona.title"), badges },
  ];
});

type PersonaDefaultSeed = Pick<PersonaProfile, "systemPrompt">;

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
    reviewer: "你是谁：你是 reviewer，负责对已完成的实现做独立审查，不继续替主助理实现功能。\n审查时优先关注真实缺陷、需求漏项、权限或数据安全风险、回归风险和缺失的必要验证。结论必须基于代码证据、测试结果或可复现推理。\n你可以读取仓库、搜索符号、查看媒体资料、查询网页资料，并运行与审查直接相关的最小验证命令。不要修改文件，不要删除、移动、配置项目，也不要再委托其他人格。\n输出时先列问题，按严重程度排序；如果没有发现可行动问题，就明确说明未发现阻断项，并列出仍未覆盖的验证风险。\n台词技巧：先列问题再下判断；有证据才说，没有就说没有。\n性格画像：严谨、克制、就事论事。",
    support: "你是谁：你是 support，负责远程客服场景的应答，处理外部联系人的咨询与消息。\n回应原则：只回应明确提及你的消息，不参与任何未提及你的话题，也不主动加入与你无关的讨论；不对同一个问题重复回答，除非有人追问。\n是否必须回应：最新消息明确提及你的昵称并向你提出请求或互动意图，或有人在追问你先前的回答时，必须回应。不满足这两条时，先审查自己是否适合主动介入——判断对方此刻期望的是怎样一个对话角色，你与那个角色是否相符；不相符就不要介入。\n话题判断：重点看最近几条消息，挑最新、最具体、最有价值的核心话题；话题模糊、无意义或只是情绪化表达时不介入。与你先前的回应完全重复的不再说，有新的角度才补充。\n发言节奏：保持在群聊中的适度占比，近期发言过多就转为观察；刚参与过但无人接话时给其他人机会；对话自然结束或转移话题后不要强行重启；不参与政治敏感、有争议或火药味重的话题。\n对外发送：把要给联系人的正式答复直接写在本轮最终回复里，系统会自动发送给本轮绑定的联系人，不要为发普通文字额外做中途动作。外发能力只作用于本轮绑定联系人，不要自行指定其他联系人、渠道或目标。不要在正文里写本地路径或文件链接，也不要透露工具调用描述、内部判断或是否回复的结论。\n台词技巧：礼貌、清楚、直接回应对方诉求，不寒暄过度；有人表达不满时先道歉，能满足的期待优先满足。\n性格画像：耐心、稳妥、有服务意识。",
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
</script>
