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
          <div v-if="!selectedOption" class="text-sm text-base-content/60">
            {{ t("chat.draftRecipientPickHint") }}
          </div>
          <div v-else-if="optionSubLabel(selectedOption)" class="text-sm text-warning">
            {{ optionSubLabel(selectedOption) }}
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
          :selected-worktree-path="selectedWorktreePath"
          :available-workspaces="mergedOptions"
          :sync-workspace-branch="props.syncWorkspaceBranch"
          :git-root-check="props.gitRootCheck"
          @update:main-path="handleMainPathUpdate"
          @update:access="handleAccessUpdate"
          @update:work-mode="handleWorkModeUpdate"
          @update:branch="handleBranchUpdate"
          @update:worktree-path="handleWorktreePathUpdate"
          @add-secondary="handleAddSecondary"
          @remove-secondary="handleRemoveSecondary"
        />
      </div>

      <div class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
        <Transition name="recipient-fade" mode="out-in">
          <div v-if="!expanded" key="recent" class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
            <div
              v-for="option in recentOptions"
              :key="option.id"
              class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-base-300 bg-base-100/60 px-1 py-2.5 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-base-100 hover:shadow-lg"
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
          </div>
          <div v-else key="all" class="flex max-h-[26rem] w-full max-w-2xl flex-col gap-2">
            <div class="min-h-0 flex-1 overflow-y-auto">
              <div class="flex max-w-full flex-wrap items-stretch justify-center gap-3">
                <div
                  v-for="option in options"
                  :key="option.id"
                  class="flex w-24 shrink-0 flex-col items-center gap-1.5 rounded-2xl border border-base-300 bg-base-100/60 px-1 py-2.5 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-base-100 hover:shadow-lg"
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
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Pencil } from "@lucide/vue";
import { agentPersonaOptionId, type AgentPersonaOption } from "../../shared/agent-persona-options";
import WorkspaceConfigCard from "../../shared/components/WorkspaceConfigCard.vue";
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
  workspaceWorktreePath?: string;
  workspaces?: ShellWorkspace[];
  workspaceAutonomousMode?: boolean;
  saveWorkspace?: (input: { path: string; name: string; access: ShellWorkspaceAccess; workMode: ShellWorkMode }) => Promise<void>;
  saveWorkspaces?: (items: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, branch?: string, worktreePath?: string) => Promise<void>;
  gitRootCheck?: (path: string) => Promise<boolean>;
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
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
  workspaceWorktreePath: "",
  workspaces: () => [],
  workspaceAutonomousMode: false,
});

const emit = defineEmits<{
  change: [value: { agentId: string }];
  "update:title": [value: string];
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

// ========== 草稿工作区 ==========

const selectedPath = ref("");
const selectedAccess = ref<ShellWorkspaceAccess>("approval");
const selectedWorkMode = ref<ShellWorkMode>("directory");
const selectedBranch = ref("");
const selectedWorktreePath = ref("");
const secondaryPaths = ref<string[]>([]);
const saving = ref(false);
let pendingSave = false;

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
  const secondaries = list
    .filter((ws) => String(ws.level || "").trim().toLowerCase() === "secondary")
    .map((ws) => stripExtendedPathPrefix(String(ws.path || "").trim()))
    .filter(Boolean);
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
  () => [props.workspaceAccess, props.workspaceWorkMode, props.workspaceBranch, props.workspaceWorktreePath] as const,
  ([nextAccess, nextMode, nextBranch, nextWorktree]) => {
    selectedAccess.value = normalizeAccess(nextAccess);
    selectedWorkMode.value = nextMode === "worktree" ? "worktree" : "directory";
    const normalizedBranch = String(nextBranch || "").trim();
    if (normalizedBranch) selectedBranch.value = normalizedBranch;
    selectedWorktreePath.value = String(nextWorktree || "").trim();
  },
  { immediate: true },
);

watch(
  () => props.workspaceRootPath,
  (nextPath) => {
    selectedPath.value = stripExtendedPathPrefix(String(nextPath || "").trim());
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

function buildSnapshotWorkspaces(): ShellWorkspace[] {
  const mainName = String(findOptionByPath(selectedPath.value)?.name || "").trim() || selectedPath.value.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || selectedPath.value;
  const items: ShellWorkspace[] = [];
  if (selectedPath.value) {
    items.push({
      id: `conversation-workspace-main-${Date.now().toString(36)}`,
      name: mainName,
      path: selectedPath.value,
      level: "main",
      access: selectedAccess.value,
      builtIn: false,
    });
  }
  const seen = new Set<string>([String(selectedPath.value || "").trim().toLowerCase()]);
  for (const secPath of secondaryPaths.value) {
    const normalized = String(secPath || "").trim();
    if (!normalized) continue;
    const key = normalized.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    const secName = String(findOptionByPath(normalized)?.name || "").trim() || normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized;
    items.push({
      id: `conversation-workspace-sec-${key}-${Math.random().toString(36).slice(2, 6)}`,
      name: secName,
      path: normalized,
      level: "secondary",
      access: selectedAccess.value,
      builtIn: false,
    });
  }
  return items;
}

async function commitSave() {
  if (!selectedPath.value) return;
  if (props.saveWorkspaces) {
    if (saving.value) {
      pendingSave = true;
      return;
    }
    saving.value = true;
    try {
      while (true) {
        pendingSave = false;
        const workspaces = buildSnapshotWorkspaces();
        const branchToSave = selectedWorkMode.value === "worktree" ? String(selectedBranch.value || "").trim() : "";
        const worktreeToSave = selectedWorkMode.value === "worktree" ? String(selectedWorktreePath.value || "").trim() : "";
        try {
          await props.saveWorkspaces(workspaces, Boolean(props.workspaceAutonomousMode), selectedWorkMode.value, branchToSave, worktreeToSave);
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
  selectedWorktreePath.value = String(props.workspaceWorktreePath || "").trim();
  syncSecondaryFromProps();
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
  if (isPathChanged) {
    selectedBranch.value = "";
    selectedWorktreePath.value = "";
    selectedWorkMode.value = "directory";
  }
  void commitSave();
}

function handleAccessUpdate(access: ShellWorkspaceAccess) {
  const normalized = normalizeAccess(access);
  if (selectedAccess.value === normalized) return;
  selectedAccess.value = normalized;
  void commitSave();
}

function handleWorkModeUpdate(mode: ShellWorkMode) {
  selectedWorkMode.value = mode === "worktree" ? "worktree" : "directory";
  void commitSave();
}

function handleBranchUpdate(branch: string) {
  selectedBranch.value = String(branch || "").trim();
  void commitSave();
}

function handleWorktreePathUpdate(payload: { worktreePath: string; branch: string; isMain: boolean }) {
  selectedWorktreePath.value = payload.isMain ? "" : payload.worktreePath;
  selectedWorkMode.value = payload.isMain ? "directory" : "worktree";
  if (payload.branch) selectedBranch.value = payload.branch;
  void commitSave();
}

function handleAddSecondary(path: string) {
  const normalized = stripExtendedPathPrefix(String(path || "").trim());
  if (!normalized) return;
  const key = normalized.toLowerCase();
  if (secondaryPaths.value.some((p) => p.toLowerCase() === key) || String(selectedPath.value || "").trim().toLowerCase() === key) return;
  pushRecentWorkspacePath(normalized);
  secondaryPaths.value = [...secondaryPaths.value, normalized];
  void commitSave();
}

function handleRemoveSecondary(path: string) {
  const key = String(path || "").trim().toLowerCase();
  secondaryPaths.value = secondaryPaths.value.filter((p) => p.toLowerCase() !== key);
  void commitSave();
}

// ========== 人格候选 ==========

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

const recentOptions = computed<AgentPersonaOption[]>(() => props.recentOptions);

function optionSubLabel(option: AgentPersonaOption): string {
  return option.modelMissing ? t("chat.personaModelNotConfigured") : "";
}

function resolveAvatarUrl(agentId: string): string {
  return props.avatarUrlMap?.[agentId] || "";
}

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
