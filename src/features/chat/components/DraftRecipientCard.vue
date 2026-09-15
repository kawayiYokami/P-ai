<template>
  <div class="absolute inset-0 z-10 flex items-center justify-center overflow-hidden bg-base-100/85 backdrop-blur-sm">
    <!-- 头像身后的扁椭圆辉光：取当前人格头像主色，取不到时降级到主题色。
         必须挂在卡片视口层；放进下面的滚动容器会被其 overflow 裁出硬边断层。
         垂直位置按「头像中心约在内容块中心上方 8rem」做补偿。 -->
    <div
      class="pointer-events-none absolute left-1/2 top-1/2 h-[280px] w-[560px] max-w-[85vw] -translate-x-1/2 -translate-y-[calc(50%+8rem)] rounded-[100%] blur-3xl transition-colors"
      :class="glowColor ? '' : 'bg-primary/[0.05]'"
      :style="glowColor ? { backgroundColor: glowColor } : undefined"
      aria-hidden="true"
    ></div>
    <div class="relative m-auto flex max-h-full w-full flex-col items-center gap-8 overflow-y-auto overscroll-contain px-6 pb-32 pt-8">
      <div class="flex items-center gap-1.5">
        <template v-if="titleEditing">
          <input
            ref="titleInputRef"
            v-model="draftTitle"
            type="text"
            class="input input-sm w-64 max-w-full text-center text-sm font-medium tracking-wide"
            :placeholder="t('chat.draftRecipientTitle')"
            @blur="commitTitle"
            @keydown.enter.prevent="commitTitle"
            @keydown.esc.prevent="cancelTitleEdit"
          />
        </template>
        <template v-else>
          <button
            type="button"
            class="group flex items-center gap-1.5 text-sm font-medium tracking-wide text-base-content/60 transition-colors hover:text-base-content"
            :title="t('common.edit')"
            @click="startTitleEdit"
          >
            <span>{{ displayTitleText }}</span>
            <Pencil class="h-3.5 w-3.5 opacity-50 transition-opacity group-hover:opacity-100" />
          </button>
        </template>
      </div>

      <div class="flex flex-col items-center gap-3">
        <div class="avatar">
          <div
            class="h-28 w-28 rounded-full shadow-2xl ring-4 ring-primary/60 ring-offset-4 ring-offset-base-100/50"
          >
            <img
              v-if="selectedOption && resolveAvatarUrl(selectedOption.agentId)"
              :src="resolveAvatarUrl(selectedOption.agentId)"
              :alt="selectedOption.agentName"
              class="h-28 w-28 rounded-full object-cover"
            />
            <div
              v-else
              class="flex h-28 w-28 items-center justify-center rounded-full bg-primary text-4xl font-semibold text-primary-content"
            >
              {{ selectedOption ? agentInitials(selectedOption.agentName) : "?" }}
            </div>
          </div>
        </div>
        <div class="flex flex-col items-center gap-0.5 text-center">
          <div class="text-xl font-bold text-base-content">
            {{ selectedOption ? selectedOption.agentName : t("chat.draftRecipientPlaceholder") }}
          </div>
          <div class="text-sm text-base-content/60">
            {{ selectedOption ? optionSubLabel(selectedOption) : t("chat.draftRecipientPickHint") }}
          </div>
        </div>
      </div>

      <div
        v-if="hasWorkspaceCapability"
        class="flex w-full max-w-md flex-col items-center gap-2"
      >
        <WorkspaceConfigCard
          :main-path="selectedPath"
          :secondary-paths="secondaryPaths"
          :access="selectedAccess"
          :work-mode="selectedWorkMode"
          :selected-branch="selectedBranch"
          :branch-list="branchList"
          :branch-loading="branchLoading"
          :git-root-available="gitRootAvailable"
          :git-check-message="worktreeCheckMessage"
          :available-workspaces="mergedOptions"
          @update:main-path="handleMainPathUpdate"
          @update:access="handleAccessUpdate"
          @update:work-mode="handleWorkModeUpdate"
          @update:branch="handleBranchUpdate"
          @browse-main="browseWorkspaceDirectory"
          @add-secondary="handleAddSecondary"
          @remove-secondary="handleRemoveSecondary"
        />
      </div>

      <div class="w-full text-center text-caption leading-tight text-base-content/45">
        {{ t("chat.draftRecipientExperimentalHint") }}
      </div>
      <div class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
        <Transition name="recipient-fade" mode="out-in">
          <div v-if="!expanded" key="recent" class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
            <template v-if="hrCollapsed">
              <div
                v-if="selectedOption"
                class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-primary/60 bg-primary/10 px-1 py-2.5 backdrop-blur-sm"
              >
                <button
                  type="button"
                  class="flex w-full flex-col items-center gap-1.5"
                  @click="emit('change', { agentId: selectedOption.agentId })"
                >
                  <div class="avatar">
                    <div class="h-14 w-14 rounded-full ring-2 ring-primary">
                      <img
                        v-if="resolveAvatarUrl(selectedOption.agentId)"
                        :src="resolveAvatarUrl(selectedOption.agentId)"
                        :alt="selectedOption.agentName"
                        class="h-14 w-14 rounded-full object-cover"
                      />
                      <div
                        v-else
                        class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/80 text-lg font-semibold text-primary-content"
                      >
                        {{ agentInitials(selectedOption.agentName) }}
                      </div>
                    </div>
                  </div>
                  <span class="max-w-full truncate text-center text-xs leading-tight text-base-content/80">
                    {{ selectedOption.agentName }}
                  </span>
                  <span
                    v-if="optionSubLabel(selectedOption)"
                    class="max-w-full truncate rounded-full px-1.5 py-0.5 text-caption leading-tight bg-primary/15 font-medium text-primary"
                  >
                    {{ optionSubLabel(selectedOption) }}
                  </span>
                </button>
              </div>
              <button
                type="button"
                class="flex w-20 shrink-0 flex-col items-center justify-center gap-1.5 rounded-2xl border border-dashed border-base-content/25 px-1 py-2.5 text-base-content/55 transition-colors hover:border-primary/50 hover:bg-base-100/70 hover:text-base-content"
                @click="handleExitHR"
              >
                <span class="flex h-14 w-14 items-center justify-center rounded-full bg-base-content/10 text-xl leading-none">
                  <ArrowLeft class="h-6 w-6" />
                </span>
                <span class="max-w-full truncate text-center text-xs leading-tight">
                  {{ t("chat.draftRecipientBack") }}
                </span>
              </button>
            </template>
            <template v-else>
            <div
              v-for="option in recentOptions"
              :key="option.id"
              class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-base-300/70 bg-base-100/60 px-1 py-2.5 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-base-100 hover:shadow-lg"
              :class="selectedAgentId === option.agentId ? 'border-primary/60 bg-primary/10' : ''"
            >
              <button
                type="button"
                class="flex w-full flex-col items-center gap-1.5"
                @click="emit('change', { agentId: option.agentId })"
              >
                <div class="avatar">
                  <div
                    class="h-14 w-14 rounded-full transition-shadow"
                    :class="selectedAgentId === option.agentId ? 'ring-2 ring-primary' : ''"
                  >
                    <img
                      v-if="resolveAvatarUrl(option.agentId)"
                      :src="resolveAvatarUrl(option.agentId)"
                      :alt="option.agentName"
                      class="h-14 w-14 rounded-full object-cover"
                    />
                    <div
                      v-else
                      class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/80 text-lg font-semibold text-primary-content"
                    >
                      {{ agentInitials(option.agentName) }}
                    </div>
                  </div>
                </div>
                <span class="max-w-full truncate text-center text-xs leading-tight text-base-content/80">
                  {{ option.agentName }}
                </span>
                <span
                  v-if="optionSubLabel(option)"
                  class="max-w-full truncate rounded-full px-1.5 py-0.5 text-caption leading-tight"
                  :class="option.id === selectedId
                    ? 'bg-primary/15 font-medium text-primary'
                    : 'bg-base-content/10 text-base-content/60'"
                >
                  {{ optionSubLabel(option) }}
                </span>
              </button>
            </div>

            <button
              v-if="recentOptions.length > 0"
              type="button"
              class="flex w-20 shrink-0 flex-col items-center justify-center gap-1.5 rounded-2xl border border-dashed border-base-content/25 px-1 py-2.5 text-base-content/55 transition-colors hover:border-primary/50 hover:bg-base-100/70 hover:text-base-content"
              @click="expanded = true"
            >
              <span class="flex h-14 w-14 items-center justify-center rounded-full bg-base-content/10 text-xl leading-none">
                +
              </span>
              <span class="max-w-full truncate text-center text-xs leading-tight">
                {{ t("chat.draftRecipientMore") }}
              </span>
            </button>
            <button
              v-else
              type="button"
              class="flex items-center gap-1.5 rounded-full border border-base-content/25 px-4 py-2 text-sm text-base-content/70 transition-colors hover:border-primary/50 hover:text-base-content"
              @click="expanded = true"
            >
              {{ t("chat.draftRecipientMore") }}
            </button>
            </template>
          </div>
          <div v-else key="all" class="flex max-h-[26rem] w-full max-w-2xl flex-col gap-2">
            <div class="min-h-0 flex-1 overflow-y-auto">
              <div class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
                <div
                  v-for="option in options"
                  :key="option.id"
                  class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-base-300/70 bg-base-100/60 px-1 py-2.5 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-base-100 hover:shadow-lg"
                  :class="selectedAgentId === option.agentId ? 'border-primary/60 bg-primary/10' : ''"
                >
                  <button
                    type="button"
                    class="flex w-full flex-col items-center gap-1.5"
                    @click="handleSelectFromAll(option)"
                  >
                    <div class="avatar">
                      <div
                        class="h-14 w-14 rounded-full transition-shadow"
                        :class="selectedAgentId === option.agentId ? 'ring-2 ring-primary' : ''"
                      >
                        <img
                          v-if="resolveAvatarUrl(option.agentId)"
                          :src="resolveAvatarUrl(option.agentId)"
                          :alt="option.agentName"
                          class="h-14 w-14 rounded-full object-cover"
                        />
                        <div
                          v-else
                          class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/80 text-lg font-semibold text-primary-content"
                        >
                          {{ agentInitials(option.agentName) }}
                        </div>
                      </div>
                    </div>
                    <span class="max-w-full truncate text-center text-xs leading-tight text-base-content/80">
                      {{ option.agentName }}
                    </span>
                    <span
                      v-if="optionSubLabel(option)"
                      class="max-w-full truncate rounded-full px-1.5 py-0.5 text-caption leading-tight"
                      :class="option.id === selectedId
                        ? 'bg-primary/15 font-medium text-primary'
                        : 'bg-base-content/10 text-base-content/60'"
                    >
                      {{ optionSubLabel(option) }}
                    </span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Transition>

        <button
          v-if="!hrCollapsed"
          type="button"
          class="flex shrink-0 flex-col items-center justify-center gap-1.5 rounded-2xl border border-dashed border-primary/40 px-3 py-2.5 text-primary transition-colors hover:border-primary hover:bg-primary/10"
          :class="expanded ? 'w-24' : ''"
          @click="emit('recruit')"
        >
          <span class="flex h-14 w-14 items-center justify-center rounded-full bg-primary/15">
            <UserPlus class="h-6 w-6" />
          </span>
          <span class="max-w-full truncate text-center text-xs font-medium leading-tight">
            {{ t("chat.draftRecipientRecruit") }}
          </span>
        </button>
      </div>
    </div>

    <WorkspaceDirectoryPickerDialog
      :open="directoryPickerOpen"
      :initial-path="directoryPickerInitialPath"
      @close="directoryPickerOpen = false"
      @select="onDirectoryPicked"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeft, Pencil, UserPlus } from "@lucide/vue";
import { gitPanelBranchList, gitPanelCheckoutCheck, gitPanelCheckout } from "../../../services/tauri-api";
import { agentPersonaOptionId, type AgentPersonaOption } from "../../shared/agent-persona-options";
import WorkspaceConfigCard from "../../shared/components/WorkspaceConfigCard.vue";
import WorkspaceDirectoryPickerDialog from "../../shared/components/WorkspaceDirectoryPickerDialog.vue";
import type { ShellWorkspace, ShellWorkMode } from "../../../types/app";
import { stripExtendedPathPrefix } from "../../../utils/shell-workspaces";
import { pushRecentWorkspacePath } from "../../../utils/recent-workspaces";
import { extractAvatarGlowColor } from "../../shared/utils/avatar-glow-color";

type WorkspaceOption = {
  id: string;
  name: string;
  path: string;
  access: "approval" | "full_access";
};

type ShellWorkspaceAccess = WorkspaceOption["access"];

const props = withDefaults(defineProps<{
  options?: AgentPersonaOption[];
  recentOptions?: AgentPersonaOption[];
  selectedAgentId?: string;
  avatarUrlMap?: Record<string, string>;
  title?: string;
  workspaceOptions?: WorkspaceOption[];
  workspaceRootPath?: string;
  workspaceAccess?: ShellWorkspaceAccess | "";
  workspaceWorkMode?: ShellWorkMode;
  workspaceBranch?: string;
  workspaces?: ShellWorkspace[];
  workspaceAutonomousMode?: boolean;
  saveWorkspace?: (input: { path: string; name: string; access: ShellWorkspaceAccess; workMode: ShellWorkMode }) => Promise<void>;
  // 新的多目录+分支持久化通道，优先于 saveWorkspace
  saveWorkspaces?: (items: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, branch?: string) => Promise<void>;
  gitRootCheck?: (path: string) => Promise<boolean>;
}>(), {
  options: () => [],
  recentOptions: () => [],
  selectedAgentId: "",
  avatarUrlMap: () => ({}),
  title: "",
  workspaceOptions: () => [],
  workspaceRootPath: "",
  workspaceAccess: "",
  workspaceWorkMode: "directory",
  workspaceBranch: "",
  workspaces: () => [],
  workspaceAutonomousMode: false,
});

const emit = defineEmits<{
  change: [value: { agentId: string }];
  "update:title": [value: string];
  recruit: [];
}>();

const { t } = useI18n();

const expanded = ref(false);

// ========== 草稿标题 ==========

const draftTitle = ref(String(props.title || "").trim());
const titleEditing = ref(false);
const titleInputRef = ref<HTMLInputElement | null>(null);

const displayTitleText = computed(() => draftTitle.value.trim() || t("chat.draftRecipientDefaultTitle"));

watch(
  () => props.title,
  (next) => {
    draftTitle.value = String(next || "").trim();
  },
);

function startTitleEdit() {
  titleEditing.value = true;
  void nextTick(() => {
    titleInputRef.value?.focus();
    titleInputRef.value?.select();
  });
}

function cancelTitleEdit() {
  titleEditing.value = false;
  draftTitle.value = String(props.title || "").trim();
}

function commitTitle() {
  titleEditing.value = false;
  const next = draftTitle.value.trim();
  emit("update:title", next);
}

// ========== 草稿工作区（以 WorkspaceConfigCard 为唯一真相源） ==========

const selectedPath = ref("");
const selectedAccess = ref<ShellWorkspaceAccess>("approval");
const selectedWorkMode = ref<ShellWorkMode>("directory");
const selectedBranch = ref("");
const secondaryPaths = ref<string[]>([]);
const branchList = ref<string[]>([]);
const branchLoading = ref(false);
const gitRootAvailable = ref(false);
const worktreeCheckMessage = ref("");
const saving = ref(false);
let pendingSave = false;
let checkSequence = 0;
let branchSequence = 0;
let lastGitCheckPath = "";

const hasWorkspaceCapability = computed(() => {
  return Boolean(props.saveWorkspace || props.saveWorkspaces || props.workspaceOptions.length > 0);
});

function normalizeAccess(value: unknown): ShellWorkspaceAccess {
  const text = String(value || "").trim();
  if (text === "full_access" || text === "approval") return text;
  return "approval";
}

const mergedOptions = computed<WorkspaceOption[]>(() => [...props.workspaceOptions]);

function findOptionByPath(path: string): WorkspaceOption | null {
  const target = String(path || "").trim().toLowerCase();
  if (!target) return null;
  return mergedOptions.value.find((item) => item.path.toLowerCase() === target) ?? null;
}

function syncSecondaryFromProps() {
  const list = Array.isArray(props.workspaces) ? props.workspaces : [];
  const secondaries = list.filter((ws) => String(ws.level || "").trim().toLowerCase() === "secondary").map((ws) => stripExtendedPathPrefix(String(ws.path || "").trim())).filter(Boolean);
  const deduped: string[] = [];
  const seen = new Set<string>();
  for (const path of secondaries) {
    const key = path.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.push(path);
  }
  secondaryPaths.value = deduped;
}

watch(
  () => [props.workspaceAccess, props.workspaceWorkMode, props.workspaceBranch] as const,
  ([nextAccess, nextMode, nextBranch]) => {
    selectedAccess.value = normalizeAccess(nextAccess);
    selectedWorkMode.value = nextMode === "worktree" ? "worktree" : "directory";
    const normalizedBranch = String(nextBranch || "").trim();
    if (normalizedBranch) selectedBranch.value = normalizedBranch;
  },
  { immediate: true },
);

watch(
  () => props.workspaceRootPath,
  (nextPath) => {
    const normalized = stripExtendedPathPrefix(String(nextPath || "").trim());
    if (selectedPath.value !== normalized) worktreeCheckMessage.value = "";
    selectedPath.value = normalized;
    if (normalized && normalized !== lastGitCheckPath) void runGitRootCheck(normalized);
    else if (!normalized) {
      gitRootAvailable.value = false;
      branchList.value = [];
      branchLoading.value = false;
      lastGitCheckPath = "";
    }
  },
  { immediate: true },
);

watch(
  () => props.workspaces,
  () => {
    syncSecondaryFromProps();
  },
  { immediate: true, deep: true },
);

watch(
  () => props.workspaceBranch,
  (nextBranch) => {
    const normalized = String(nextBranch || "").trim();
    if (normalized) selectedBranch.value = normalized;
  },
);

function buildSnapshotWorkspaces(): ShellWorkspace[] {
  const mainName = String(findOptionByPath(selectedPath.value)?.name || "").trim() || selectedPath.value.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || selectedPath.value;
  const items: ShellWorkspace[] = [];
  if (selectedPath.value) items.push({ id: `conversation-workspace-main-${Date.now().toString(36)}`, name: mainName, path: selectedPath.value, level: "main", access: selectedAccess.value, builtIn: false });
  const seen = new Set<string>([String(selectedPath.value || "").trim().toLowerCase()]);
  for (const secPath of secondaryPaths.value) {
    const normalized = String(secPath || "").trim();
    if (!normalized) continue;
    const key = normalized.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    const secName = String(findOptionByPath(normalized)?.name || "").trim() || normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized;
    items.push({ id: `conversation-workspace-sec-${key}-${Math.random().toString(36).slice(2, 6)}`, name: secName, path: normalized, level: "secondary", access: selectedAccess.value, builtIn: false });
  }
  return items;
}

async function commitSave() {
  if (!selectedPath.value) return;
  if (props.saveWorkspaces) {
    if (saving.value) { pendingSave = true; return; }
    saving.value = true;
    try {
      while (true) {
        pendingSave = false;
        const workspaces = buildSnapshotWorkspaces();
        const branchToSave = selectedWorkMode.value === "worktree" ? String(selectedBranch.value || "").trim() : "";
        try {
          await props.saveWorkspaces(workspaces, Boolean(props.workspaceAutonomousMode), selectedWorkMode.value, branchToSave);
        } catch {
          restoreFromProps();
          break;
        }
        if (!pendingSave) break;
      }
    } finally {
      saving.value = false;
    }
    return;
  }
  if (!props.saveWorkspace) return;
  if (saving.value) {
    pendingSave = true;
    return;
  }
  saving.value = true;
  try {
    while (true) {
      pendingSave = false;
      const source = findOptionByPath(selectedPath.value);
      const snapshot = {
        path: selectedPath.value,
        name: String(source?.name || "").trim() || selectedPath.value.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || selectedPath.value,
        access: selectedAccess.value,
        workMode: selectedWorkMode.value,
      };
      try {
        await props.saveWorkspace(snapshot);
      } catch {
        restoreFromProps();
        break;
      }
      if (!pendingSave) break;
    }
  } finally {
    saving.value = false;
  }
}

function restoreFromProps() {
  selectedPath.value = stripExtendedPathPrefix(String(props.workspaceRootPath || "").trim());
  selectedAccess.value = normalizeAccess(props.workspaceAccess);
  selectedWorkMode.value = props.workspaceWorkMode === "worktree" ? "worktree" : "directory";
  selectedBranch.value = String(props.workspaceBranch || "").trim();
  syncSecondaryFromProps();
  gitRootAvailable.value = false;
  worktreeCheckMessage.value = "";
  branchList.value = [];
  branchLoading.value = false;
}

async function runGitRootCheck(path: string) {
  const sequence = ++checkSequence;
  lastGitCheckPath = path;
  if (!path) {
    gitRootAvailable.value = false;
    worktreeCheckMessage.value = "";
    branchList.value = [];
    return;
  }
  // 检查期间保留上一目录的 gitRootAvailable/branchList，不立即隐藏，避免切换时跳动
  worktreeCheckMessage.value = "";
  branchLoading.value = true;
  let available = false;
  try {
    if (props.gitRootCheck) {
      available = await props.gitRootCheck(path);
    } else {
      try {
        const entries = await gitPanelBranchList(path);
        const names = entries.map((e) => String(e.name || "").trim()).filter(Boolean);
        if (names.length === 0) {
          available = false;
        } else {
          available = true;
          if (sequence === checkSequence) {
            branchList.value = names;
            const current = entries.find((e) => e.isCurrent)?.name;
            if (current && !String(selectedBranch.value || "").trim()) {
              selectedBranch.value = String(current).trim();
            }
            gitRootAvailable.value = true;
            worktreeCheckMessage.value = "";
            branchLoading.value = false;
            return;
          }
        }
      } catch {
        available = false;
      }
    }
    if (sequence !== checkSequence) return;
    gitRootAvailable.value = Boolean(available);
    worktreeCheckMessage.value = "";
  } catch {
    if (sequence !== checkSequence) return;
    gitRootAvailable.value = false;
    worktreeCheckMessage.value = "";
  } finally {
    if (sequence === checkSequence) branchLoading.value = false;
  }
  if (!gitRootAvailable.value && selectedWorkMode.value !== "directory") {
    selectedWorkMode.value = "directory";
    void commitSave();
  }
  if (gitRootAvailable.value) {
    void loadBranchList(path);
  } else {
    branchList.value = [];
  }
}

async function loadBranchList(path: string) {
  const seq = ++branchSequence;
  const normalized = String(path || "").trim();
  if (!normalized) {
    branchList.value = [];
    return;
  }
  branchLoading.value = true;
  try {
    const entries = await gitPanelBranchList(normalized);
    if (seq !== branchSequence) return;
    const names = entries.map((e) => String(e.name || "").trim()).filter(Boolean);
    branchList.value = names;
    const current = entries.find((e) => e.isCurrent)?.name;
    if (current) {
      const curName = String(current).trim();
      // 若用户尚未选择分支，默认选中当前分支
      if (!String(selectedBranch.value || "").trim()) {
        selectedBranch.value = curName;
        void commitSave();
      }
    } else if (!String(selectedBranch.value || "").trim() && names.length > 0) {
      selectedBranch.value = names[0];
      void commitSave();
    }
  } catch (error) {
    if (seq !== branchSequence) return;
    // 分支拉取失败不阻塞主流程，仅清空列表
    console.warn("[分支] 获取分支列表失败", error);
    branchList.value = [];
  } finally {
    if (seq === branchSequence) branchLoading.value = false;
  }
}

function handleMainPathUpdate(path: string) {
  const normalized = stripExtendedPathPrefix(String(path || "").trim());
  if (!normalized) return;
  pushRecentWorkspacePath(normalized);
  const previousPath = String(selectedPath.value || "").trim().toLowerCase();
  const isPathChanged = normalized.toLowerCase() !== previousPath;
  selectedPath.value = normalized;
  const source = findOptionByPath(normalized);
  if (source) selectedAccess.value = normalizeAccess(source.access);
  if (isPathChanged) { selectedBranch.value = ""; branchList.value = []; }
  void commitSave();
  void runGitRootCheck(normalized);
}

function handleAccessUpdate(access: ShellWorkspaceAccess) {
  const normalized = normalizeAccess(access);
  if (selectedAccess.value === normalized) return;
  selectedAccess.value = normalized;
  // 统一权限：同步所有目录的 access（本地预览）
  void commitSave();
}

async function handleWorkModeUpdate(mode: ShellWorkMode) {
  const normalized = mode === "worktree" ? "worktree" : "directory";
  if (selectedWorkMode.value === normalized) return;
  if (normalized === "worktree" && !gitRootAvailable.value) return;
  if (normalized === "directory") {
    selectedWorkMode.value = "directory" as ShellWorkMode;
    if (gitRootAvailable.value && selectedPath.value) {
      try {
        const entries = await gitPanelBranchList(selectedPath.value);
        const current = entries.find((e) => e.isCurrent)?.name;
        if (current) {
          const curName = String(current).trim();
          if (curName) selectedBranch.value = curName;
        }
        branchList.value = entries.map((e) => String(e.name || "").trim()).filter(Boolean);
      } catch {
        // ignore
      }
    }
    worktreeCheckMessage.value = "";
    void commitSave();
    return;
  }
  selectedWorkMode.value = "worktree" as ShellWorkMode;
  worktreeCheckMessage.value = "";
  if (gitRootAvailable.value && branchList.value.length === 0 && selectedPath.value) {
    void loadBranchList(selectedPath.value);
  }
  void commitSave();
}

async function handleBranchUpdate(branch: string) {
  const normalized = String(branch || "").trim();
  if (!normalized) return;
  if (selectedBranch.value === normalized) return;
  if (selectedWorkMode.value === "worktree") {
    selectedBranch.value = normalized;
    worktreeCheckMessage.value = "";
    void commitSave();
    return;
  }
  if (!gitRootAvailable.value || !selectedPath.value) {
    selectedBranch.value = normalized;
    worktreeCheckMessage.value = "";
    void commitSave();
    return;
  }
  branchLoading.value = true;
  worktreeCheckMessage.value = "";
  try {
    const check = await gitPanelCheckoutCheck(selectedPath.value, normalized);
    const dirtyPaths: string[] = (check as unknown as { dirtyPaths: string[] }).dirtyPaths || [];
    if (Array.isArray(dirtyPaths) && dirtyPaths.length > 0) {
      const preview = dirtyPaths.slice(0, 3).join(", ");
      const more = dirtyPaths.length > 3 ? t("chat.workspaceBranchDirtyMore", { count: dirtyPaths.length - 3 }) : "";
      const detail = preview ? t("chat.workspaceBranchDirtyDetail", { preview, more }) : "";
      worktreeCheckMessage.value = t("chat.workspaceBranchDirtyBlocked", { detail });
      return;
    }
    try {
      await gitPanelCheckout(selectedPath.value, normalized);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      worktreeCheckMessage.value = t("chat.workspaceBranchCheckoutFailed", { message });
      return;
    }
    selectedBranch.value = normalized;
    try {
      const entries = await gitPanelBranchList(selectedPath.value);
      branchList.value = entries.map((e) => String(e.name || "").trim()).filter(Boolean);
    } catch {
      // ignore
    }
    void commitSave();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    worktreeCheckMessage.value = t("chat.workspaceBranchCheckFailed", { message });
  } finally {
    branchLoading.value = false;
  }
}

const directoryPickerOpen = ref(false);
const directoryPickerMode = ref<"main" | "secondary">("main");
const directoryPickerInitialPath = ref("");

function browseWorkspaceDirectory() {
  directoryPickerMode.value = "main";
  directoryPickerInitialPath.value = String(selectedPath.value || "").trim();
  directoryPickerOpen.value = true;
}

function handleAddSecondary() {
  directoryPickerMode.value = "secondary";
  directoryPickerInitialPath.value = String(selectedPath.value || "").trim();
  directoryPickerOpen.value = true;
}

function onDirectoryPicked(pickedPath: string) {
  const path = stripExtendedPathPrefix(String(pickedPath || "").trim());
  directoryPickerOpen.value = false;
  if (!path) return;
  pushRecentWorkspacePath(path);
  if (directoryPickerMode.value === "main") handleMainPathUpdate(path);
  else {
    const key = path.toLowerCase();
    if (secondaryPaths.value.some((p) => p.toLowerCase() === key) || String(selectedPath.value || "").trim().toLowerCase() === key) return;
    secondaryPaths.value = [...secondaryPaths.value, path];
    void commitSave();
  }
}

function handleRemoveSecondary(path: string) {
  const key = String(path || "").trim().toLowerCase();
  secondaryPaths.value = secondaryPaths.value.filter((p) => p.toLowerCase() !== key);
  void commitSave();
}

watch(
  () => selectedWorkMode.value,
  (mode) => {
    if (mode === "worktree" && gitRootAvailable.value && branchList.value.length === 0) {
      void loadBranchList(selectedPath.value);
    }
  },
);

// ========== 人格候选 ==========

// 选中 HR（人力人格）时进入收敛态：卡片墙仅保留当前 HR 卡 + 「返回」入口
const isHRSelected = computed(() => {
  const agentId = String(props.selectedAgentId || "").trim();
  return agentId === "hr";
});

// 手动展开过全量卡片墙后，即使仍选中 HR 也保持展示（返回不改变选中，仅恢复卡片墙）
const exitHRView = ref(false);
watch(isHRSelected, (hr) => {
  if (!hr) exitHRView.value = false;
});
// HR 收敛态是否生效
const hrCollapsed = computed(() => isHRSelected.value && !exitHRView.value);

// 返回：退出 HR 收敛态，并自动切到最近用过的 agent（recentOptions 首位，已排除当前会话）
function handleExitHR() {
  exitHRView.value = true;
  expanded.value = false;
  const recent = recentOptions.value[0];
  if (recent) {
    emit("change", { agentId: recent.agentId });
  }
}

const selectedAgentId = computed(() => String(props.selectedAgentId || "").trim());

const selectedId = computed(() => {
  const agentId = String(props.selectedAgentId || "").trim();
  if (!agentId) return "";
  return agentPersonaOptionId(agentId);
});

const selectedOption = computed<AgentPersonaOption | null>(() => {
  const id = selectedId.value;
  if (!id) return null;
  return (
    props.options.find((option) => option.id === id)
    || props.recentOptions.find((option) => option.id === id)
    || null
  );
});

// 行星候选直接按人格（agentId）铺开展示：组织已扁平为「人格即目标」，一个人格一张卡片
const recentOptions = computed<AgentPersonaOption[]>(() => props.recentOptions);

const allOptions = computed<AgentPersonaOption[]>(() => props.options);

// 卡片副标签：以模型名（取不到时退回供应商名）表达这张卡指向的运行时
function optionSubLabel(option: AgentPersonaOption): string {
  const model = String(option.modelName || "").trim();
  if (model) return model;
  const provider = String(option.providerName || "").trim();
  if (provider) return provider;
  return option.modelMissing ? t("chat.personaModelNotConfigured") : "";
}

function resolveAvatarUrl(agentId: string): string {
  return props.avatarUrlMap?.[agentId] || "";
}

// 背景光斑从当前人格头像取主色：头像换色时重新取一次，取不到就交给模板降级到主题色
const glowColor = ref<string | null>(null);
let glowToken = 0;
const selectedAvatarUrl = computed(() => {
  const option = selectedOption.value;
  return option ? resolveAvatarUrl(option.agentId) : "";
});
watch(
  selectedAvatarUrl,
  async (url) => {
    const token = ++glowToken;
    const color = await extractAvatarGlowColor(url);
    if (token !== glowToken) return;
    glowColor.value = color;
  },
  { immediate: true },
);

function agentInitials(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  const firstTwo = text.slice(0, 2);
  if (/^[A-Za-z]{2}/.test(firstTwo)) {
    return firstTwo.toUpperCase();
  }
  return text.charAt(0).toUpperCase();
}

function handleSelectFromAll(option: AgentPersonaOption) {
  // 切换人格后保持全量卡片墙展开，不自动收起，方便连续比较与再切换
  emit("change", { agentId: option.agentId });
}
</script>

<style scoped>
.recipient-fade-enter-active,
.recipient-fade-leave-active {
  transition: opacity 0.18s ease;
}
.recipient-fade-enter-from,
.recipient-fade-leave-to {
  opacity: 0;
}
</style>
