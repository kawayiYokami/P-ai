<template>
  <ConfigTemplate :model-value="templateValues" :groups="templateGroups">
    <template #row-vision-api>
      <label class="grid min-w-0 gap-2">
        <div>
          <div class="text-sm">{{ t("config.chatSettings.visionApi") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.visionApiHint") }}</div>
        </div>
        <ApiConfigPicker
          :model-value="config.visionApiConfigId ?? ''"
          :api-configs="imageCapableApiConfigs"
          :placeholder="t('config.chatSettings.noVision')"
          @update:model-value="onVisionSelect"
        />
      </label>
    </template>

    <template #row-tool-review-api>
      <label class="grid min-w-0 gap-2">
        <div>
          <div class="text-sm">{{ t("config.chatSettings.toolReviewApi") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.toolReviewApiHint") }}</div>
        </div>
        <ApiConfigPicker
          :model-value="config.toolReviewApiConfigId ?? ''"
          :api-configs="textCapableApiConfigs"
          @update:model-value="onToolReviewSelect"
        />
      </label>
    </template>

    <template #row-expert-chat-model>
      <label class="grid min-w-0 gap-2">
        <div>
          <div class="text-sm">{{ t("config.chatSettings.expertChatModelTitle") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.expertChatModelHint") }}</div>
        </div>
        <ApiConfigPicker
          :model-value="config.expertApiConfigId || ''"
          :api-configs="textCapableApiConfigs"
          @update:model-value="onExpertSelect"
        />
      </label>
    </template>

    <template #row-image-generation-model>
      <label class="grid min-w-0 gap-2">
        <div>
          <div class="text-sm">{{ t("config.imageGeneration.defaultModel") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.imageGeneration.defaultModelHint") }}</div>
        </div>
        <select :value="config.imageGenerationModelId || ''" class="select select-bordered select-sm w-full" @change="onImageGenerationSelectChange">
          <option value="">{{ t("config.imageGeneration.noDefaultModel") }}</option>
          <option v-for="option in imageGenerationModelOptions" :key="option.id" :value="option.id">
            {{ option.label }}
          </option>
        </select>
      </label>
    </template>

    <template #row-stt-api>
      <label class="grid min-w-0 gap-2">
        <div>
          <div class="text-sm">{{ t("config.chatSettings.sttTitle") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.sttHint") }}</div>
        </div>
        <select :value="config.sttApiConfigId ?? ''" class="select select-bordered select-sm w-full" @change="onSttSelectChange">
          <option value="">{{ t("config.chatSettings.sttLocalWebSpeech") }}</option>
          <option v-for="a in sttCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
    </template>

    <template #row-stt-auto-send>
      <div class="flex min-w-0 items-center justify-between gap-4" :class="{ 'opacity-50': !config.sttApiConfigId }">
        <div class="min-w-0">
          <div class="text-sm">{{ t("config.chatSettings.sttAutoSend") }}</div>
          <p class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.sttHint") }}</p>
        </div>
        <input
          :checked="!!config.sttAutoSend"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          :disabled="!config.sttApiConfigId"
          @change="onSttAutoSendChange"
        />
      </div>
    </template>

    <template #row-response-style>
      <div class="grid min-w-0 gap-2">
        <SegmentedControl
          :model-value="responseStyleId"
          :options="responseStyleSegmentOptions"
          size="sm"
          @change="onResponseStyleChange"
        />
      </div>
    </template>

    <template #row-exec-terminal>
      <div v-if="isWindowsHost" class="grid gap-2">
        <div v-if="t('config.chatSettings.execTerminalHint')" class="text-xs opacity-70">
          {{ t("config.chatSettings.execTerminalHint") }}
        </div>
        <select
          class="select select-bordered select-sm w-full"
          :value="terminalShellKindValue"
          :disabled="terminalShellOptionsLoading || savingConfig"
          @change="onTerminalShellKindChange"
        >
          <option v-for="item in terminalShellOptions" :key="item.kind" :value="item.kind">
            {{ item.label }}
          </option>
        </select>
        <div v-if="showGitInstallHintInWorkspace" class="text-xs bg-warning/10 text-base-content rounded px-2 py-1 flex items-center gap-2">
          <span>{{ t("config.chatSettings.gitRequiredHint") }}</span>
          <button class="btn btn-sm bg-base-100" @click="openGitDownloadLink">
            {{ t("config.chatSettings.installGit") }}
          </button>
        </div>
      </div>
    </template>

    <template #row-assistant-space-dir>
      <div class="grid min-w-0 gap-2">
        <div class="text-sm">{{ t("config.chatSettings.assistantSpaceDirTitle") }}</div>
        <div class="flex items-center gap-2">
          <code
            class="min-w-0 flex-1 truncate rounded bg-base-200 px-2 py-1 font-mono text-xs"
            :class="{ 'opacity-50': !assistantSpacePath }"
          >{{ assistantSpacePath || t("config.chatSettings.assistantSpaceDirEmpty") }}</code>
          <button
            class="btn btn-sm shrink-0"
            type="button"
            :disabled="!localFileSystemAvailable || !assistantSpacePath"
            @click="pickAssistantSpaceDir"
          >
            {{ t("config.chatSettings.assistantSpaceDirModify") }}
          </button>
        </div>
        <div v-if="assistantSpaceStatus" class="text-xs" :class="assistantSpaceStatusError ? 'text-error' : 'opacity-70'">
          {{ assistantSpaceStatus }}
        </div>
      </div>
    </template>

    <template #row-desktop-operate>
      <div class="flex min-w-0 items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("config.chatSettings.desktopOperateEnabled") }}</div>
          <p class="mt-1 text-xs text-base-content/60">{{ t("config.chatSettings.desktopOperateEnabledHint") }}</p>
        </div>
        <input
          :checked="!!props.config.desktopOperateEnabled"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="onDesktopOperateChange"
        />
      </div>
    </template>

    <template #row-instruction-presets>
      <div class="grid min-w-0 gap-3">
        <div v-if="instructionPresetsDraft.length === 0" class="text-sm opacity-60">
          {{ t("config.chatSettings.noInstructionPresets") }}
        </div>
        <div v-else class="grid gap-2">
          <div v-for="item in instructionPresetsDraft" :key="item.id" class="flex items-center gap-2">
            <input
              v-model="item.prompt"
              type="text"
              class="input input-bordered input-sm min-w-0 flex-1"
              :placeholder="t('config.chatSettings.instructionPresetPlaceholder')"
            />
            <button class="btn btn-sm btn-ghost btn-square shrink-0" @click="removeInstructionPreset(item.id)">
              <Trash2 class="h-4 w-4" />
            </button>
          </div>
        </div>
        <div class="flex items-center justify-between">
          <button class="btn btn-sm btn-ghost shrink-0" @click="addInstructionPreset">
            <Plus class="h-4 w-4" />
            <span>{{ t("config.chatSettings.addInstructionPreset") }}</span>
          </button>
          <button class="btn btn-sm btn-primary" :disabled="!instructionPresetsDirty" @click="saveInstructionPresets">
            {{ t("config.chatSettings.saveInstructionPresets") }}
          </button>
        </div>
      </div>
    </template>

  </ConfigTemplate>
  <WorkspaceDirectoryPickerDialog
    :open="assistantSpacePickerOpen"
    :initial-path="assistantSpacePickerInitialPath"
    @close="assistantSpacePickerOpen = false"
    @select="onAssistantSpacePicked"
  />

  <dialog ref="migrateWorkspaceDialog" class="modal">
    <div class="modal-box max-w-lg p-4">
      <h3 class="text-sm font-semibold">{{ t("config.tools.migrateWorkspaceTitle") }}</h3>
      <p v-if="workspaceMigrationUi.mode === 'confirm'" class="mt-3 text-sm whitespace-pre-wrap">
        {{ t("config.tools.migrateWorkspaceConfirm", { oldPath: pendingWorkspaceMigration.oldPath, newPath: pendingWorkspaceMigration.newPath }) }}
      </p>
      <div v-else class="mt-3 grid gap-3">
        <div class="text-sm">{{ workspaceMigrationUi.message || t("config.tools.migrateWorkspacePreparing") }}</div>
        <progress class="progress progress-primary w-full" :value="workspaceMigrationProgressValue" max="100"></progress>
        <div class="flex items-center justify-between text-xs opacity-70">
          <span>{{ workspaceMigrationStageLabel }}</span>
          <span>{{ workspaceMigrationUi.processed }}/{{ workspaceMigrationUi.total }}</span>
        </div>
        <div v-if="workspaceMigrationUi.currentPath" class="text-xs font-mono break-all opacity-70">
          {{ workspaceMigrationUi.currentPath }}
        </div>
        <div v-if="workspaceMigrationUi.error" class="rounded bg-error/10 px-3 py-2 text-sm text-error whitespace-pre-wrap break-all">
          {{ workspaceMigrationUi.error }}
        </div>
      </div>
      <div class="modal-action mt-4">
        <button
          class="btn btn-sm btn-ghost"
          type="button"
          :disabled="workspaceMigrationUi.mode === 'running'"
          @click="cancelWorkspaceMigration"
        >
          {{ workspaceMigrationUi.mode === 'error' ? t("common.close") : t("common.cancel") }}
        </button>
        <button
          v-if="workspaceMigrationUi.mode === 'confirm'"
          class="btn btn-sm btn-outline"
          type="button"
          @click="skipWorkspaceMigration"
        >
          {{ t("config.tools.migrateWorkspaceSkip") }}
        </button>
        <button
          v-if="workspaceMigrationUi.mode === 'confirm'"
          class="btn btn-sm btn-primary"
          type="button"
          @click="confirmWorkspaceMigration"
        >
          {{ t("common.confirm") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close" @click="cancelWorkspaceMigration">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Trash2 } from "@lucide/vue";
import SegmentedControl from "../../components/SegmentedControl.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import ApiConfigPicker from "../../components/ApiConfigPicker.vue";
import WorkspaceDirectoryPickerDialog from "../../../shared/components/WorkspaceDirectoryPickerDialog.vue";
import type { AppConfig, ApiConfigItem, ChatSettingsPatch, ConversationApiSettingsPatch, PromptCommandPreset, ResponseStyleOption, ToolLoadStatus } from "../../../../types/app";
import {
  getTransportCapabilities,
  invokeTauri,
  migrateTransportShellWorkspaceDirectory,
  onTransportNotification,
  openTransportExternalUrl,
} from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import { deriveImageGenerationModelOptions } from "../../utils/image-generation-config";

type TerminalShellCandidate = {
  kind: string;
  label: string;
  available: boolean;
  path?: string;
};

type TerminalShellCandidatesResult = {
  preferredKind?: string;
  currentKind?: string;
  currentPath?: string;
  options?: TerminalShellCandidate[];
};

const props = defineProps<{
  config: AppConfig;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  pdfReadMode: "text" | "image";
  instructionPresets: PromptCommandPreset[];
  toolStatuses: ToolLoadStatus[];
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
}>();

const { t } = useI18n();
const templateValues = {};
const terminalShellOptionsLoading = ref(false);
const terminalShellOptions = ref<TerminalShellCandidate[]>([]);
const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";
const isWindowsHost = typeof navigator !== "undefined" && /windows/i.test(String(navigator.userAgent || ""));
const terminalShellKindValue = computed(() => String(props.config.terminalShellKind || "auto"));

// ========== 助理空间目录（shellWorkspaces system 级条目） ==========

const localFileSystemAvailable = getTransportCapabilities().localFileSystem;
const assistantSpaceStatus = ref("");
const assistantSpaceStatusError = ref(false);

type WorkspaceMigrationDecision = "migrate" | "skip" | "cancel";
const migrateWorkspaceDialog = ref<HTMLDialogElement | null>(null);
const pendingWorkspaceMigration = ref({ oldPath: "", newPath: "" });
const workspaceMigrationUi = ref({
  taskId: "",
  mode: "confirm" as "confirm" | "running" | "error",
  stage: "idle",
  message: "",
  processed: 0,
  total: 0,
  currentPath: "",
  error: "",
});
let resolveWorkspaceMigrationConfirm: ((value: WorkspaceMigrationDecision) => void) | null = null;
let workspaceMigrationProgressUnlisten: (() => void) | null = null;

const assistantSpacePickerOpen = ref(false);
const assistantSpacePickerInitialPath = ref("");

const assistantSpacePath = computed(() => String(props.config.shellWorkspaces?.[0]?.path || "").trim());

const workspaceMigrationProgressValue = computed(() => {
  if (workspaceMigrationUi.value.total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((workspaceMigrationUi.value.processed / workspaceMigrationUi.value.total) * 100)));
});
const workspaceMigrationStageLabel = computed(() => {
  const stage = workspaceMigrationUi.value.stage;
  if (stage === "scanning") return t("config.tools.migrateWorkspaceStageScanning");
  if (stage === "copying") return t("config.tools.migrateWorkspaceStageCopying");
  if (stage === "deleting") return t("config.tools.migrateWorkspaceStageDeleting");
  if (stage === "completed") return t("config.tools.migrateWorkspaceStageCompleted");
  if (stage === "failed") return t("config.tools.migrateWorkspaceStageFailed");
  return t("config.tools.migrateWorkspacePreparing");
});

function setAssistantSpaceStatus(text: string, isError = false) {
  assistantSpaceStatus.value = text;
  assistantSpaceStatusError.value = isError;
}

function requestWorkspaceMigrationConfirm(oldPath: string, newPath: string): Promise<WorkspaceMigrationDecision> {
  const dialog = migrateWorkspaceDialog.value;
  if (!dialog) return Promise.resolve("cancel");
  pendingWorkspaceMigration.value = { oldPath, newPath };
  workspaceMigrationUi.value = {
    taskId: "",
    mode: "confirm",
    stage: "idle",
    message: "",
    processed: 0,
    total: 0,
    currentPath: "",
    error: "",
  };
  return new Promise<WorkspaceMigrationDecision>((resolve) => {
    resolveWorkspaceMigrationConfirm = resolve;
    dialog.showModal();
  });
}

function finishWorkspaceMigrationConfirm(value: WorkspaceMigrationDecision) {
  const dialog = migrateWorkspaceDialog.value;
  if (dialog?.open && value !== "migrate") {
    dialog.close();
  }
  resolveWorkspaceMigrationConfirm?.(value);
  resolveWorkspaceMigrationConfirm = null;
}

function confirmWorkspaceMigration() {
  workspaceMigrationUi.value.mode = "running";
  workspaceMigrationUi.value.stage = "scanning";
  workspaceMigrationUi.value.message = t("config.tools.migrateWorkspacePreparing");
  finishWorkspaceMigrationConfirm("migrate");
}

function skipWorkspaceMigration() {
  finishWorkspaceMigrationConfirm("skip");
}

function cancelWorkspaceMigration() {
  if (workspaceMigrationUi.value.mode === "running") return;
  finishWorkspaceMigrationConfirm("cancel");
}

async function ensureWorkspaceMigrationListener() {
  if (workspaceMigrationProgressUnlisten) return;
  workspaceMigrationProgressUnlisten = onTransportNotification<{
    taskId: string;
    stage: string;
    processed: number;
    total: number;
    currentPath?: string | null;
    message: string;
    done: boolean;
    error?: string | null;
  }>("workspace.migrationProgress", (payload) => {
    if (!payload || payload.taskId !== workspaceMigrationUi.value.taskId) return;
    workspaceMigrationUi.value.stage = String(payload.stage || "");
    workspaceMigrationUi.value.message = String(payload.message || "");
    workspaceMigrationUi.value.processed = Number(payload.processed || 0);
    workspaceMigrationUi.value.total = Number(payload.total || 0);
    workspaceMigrationUi.value.currentPath = String(payload.currentPath || "");
    workspaceMigrationUi.value.error = String(payload.error || "");
    if (payload.error) {
      workspaceMigrationUi.value.mode = "error";
    }
  });
}

async function migrateWorkspaceWithProgress(oldPath: string, newPath: string): Promise<string> {
  await ensureWorkspaceMigrationListener();
  const taskId = `workspace-migration-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  workspaceMigrationUi.value.taskId = taskId;
  workspaceMigrationUi.value.mode = "running";
  workspaceMigrationUi.value.stage = "scanning";
  workspaceMigrationUi.value.message = t("config.tools.migrateWorkspacePreparing");
  workspaceMigrationUi.value.processed = 0;
  workspaceMigrationUi.value.total = 0;
  workspaceMigrationUi.value.currentPath = oldPath;
  workspaceMigrationUi.value.error = "";
  try {
    const migratedPath = await migrateTransportShellWorkspaceDirectory({ oldPath, newPath, taskId });
    const dialog = migrateWorkspaceDialog.value;
    if (dialog?.open) {
      dialog.close();
    }
    return migratedPath;
  } catch (error) {
    workspaceMigrationUi.value.mode = "error";
    workspaceMigrationUi.value.stage = "failed";
    workspaceMigrationUi.value.message = t("config.tools.migrateWorkspaceFailed");
    workspaceMigrationUi.value.error = toErrorMessage(error);
    throw error;
  }
}

// 修改助理空间目录：选目录 → 与已保存路径不同则弹窗（迁移/跳过/取消），语义与原工具页保存流程一致
function pickAssistantSpaceDir() {
  if (!localFileSystemAvailable || workspaceMigrationUi.value.mode === "running") return;
  const workspace = props.config.shellWorkspaces?.[0];
  const previousSavedPath = String(workspace?.path || "").trim();
  if (!workspace || !previousSavedPath) return;
  assistantSpacePickerInitialPath.value = previousSavedPath;
  assistantSpacePickerOpen.value = true;
}

async function onAssistantSpacePicked(nextPathRaw: string) {
  assistantSpacePickerOpen.value = false;
  const workspace = props.config.shellWorkspaces?.[0];
  const previousSavedPath = String(workspace?.path || "").trim();
  if (!workspace || !previousSavedPath) return;
  const nextPath = String(nextPathRaw || "").trim();
  const normalizedPreviousSavedPath = previousSavedPath.replace(/[\\/]+$/g, "");
  const normalizedNextPath = nextPath.replace(/[\\/]+$/g, "");
  if (!normalizedNextPath || normalizedPreviousSavedPath === normalizedNextPath) return;
  const decision = await requestWorkspaceMigrationConfirm(previousSavedPath, nextPath);
  if (decision === "cancel") return;
  if (decision === "migrate") {
    try {
      const migratedPath = await migrateWorkspaceWithProgress(previousSavedPath, nextPath);
      workspace.path = migratedPath;
      setAssistantSpaceStatus(t("config.tools.migrateWorkspaceDone", { path: migratedPath }));
    } catch {
      return;
    }
  } else {
    workspace.path = nextPath;
  }
  const saved = await Promise.resolve(props.saveConfigAction());
  if (!saved) {
    workspace.path = previousSavedPath;
    setAssistantSpaceStatus(t("config.chatSettings.assistantSpaceDirSaveFailed"), true);
  }
}

async function loadTerminalShellCandidates() {
  if (!isWindowsHost) return;
  terminalShellOptionsLoading.value = true;
  try {
    const payload = await invokeTauri<TerminalShellCandidatesResult>("list_terminal_shell_candidates");
    const options = Array.isArray(payload.options) ? payload.options : [];
    terminalShellOptions.value =
      options.length > 0
        ? options
        : [{ kind: "auto", label: "Auto", available: true }];
    const preferred = String(payload.preferredKind || "").trim();
    if (preferred) {
      props.config.terminalShellKind = preferred;
    } else if (!String(props.config.terminalShellKind || "").trim()) {
      props.config.terminalShellKind = "auto";
    }
  } catch {
    terminalShellOptions.value = [{ kind: "auto", label: "Auto", available: true }];
    if (!String(props.config.terminalShellKind || "").trim()) {
      props.config.terminalShellKind = "auto";
    }
  } finally {
    terminalShellOptionsLoading.value = false;
  }
}

// 终端 shell 类型没有独立保存按钮，改动后立即持久化，避免重启后配置丢失。
async function onTerminalShellKindChange(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  const next = String(target?.value || "auto").trim() || "auto";
  const previous = String(props.config.terminalShellKind || "auto").trim() || "auto";
  props.config.terminalShellKind = next;
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      props.config.terminalShellKind = previous;
      console.warn("terminal shell kind save rejected");
    }
  } catch {
    props.config.terminalShellKind = previous;
    console.warn("terminal shell kind save failed");
  }
}

// 桌面操作开关没有独立保存按钮，改动后立即持久化，避免重启后配置丢失。
async function onDesktopOperateChange(event: Event) {
  const target = event.target as HTMLInputElement | null;
  const next = target?.checked ?? false;
  props.config.desktopOperateEnabled = next;
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      // 保存被拒绝时回滚开关状态，避免界面与实际配置不一致
      props.config.desktopOperateEnabled = !next;
      console.warn("desktop operate toggle save rejected");
    }
  } catch {
    props.config.desktopOperateEnabled = !next;
    console.warn("desktop operate toggle save failed");
  }
}

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

const showGitInstallHintInWorkspace = computed(
  () => isWindowsHost && toolStatusById("exec")?.status === "unavailable",
);

function openGitDownloadLink() {
  void openTransportExternalUrl(GIT_DOWNLOAD_URL);
}

onMounted(() => {
  void loadTerminalShellCandidates();
});
const templateGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "desktop-operate",
    title: t("config.chatSettings.desktopOperateTitle"),
    rows: [
      { key: "desktop-operate", items: [] },
    ],
  },
  {
    key: "default-models",
    title: t("config.chatSettings.defaultModelsTitle"),
    rows: [
      { key: "vision-api", items: [] },
      { key: "tool-review-api", items: [] },
      { key: "expert-chat-model", items: [] },
      { key: "image-generation-model", items: [] },
      { key: "stt-api", items: [] },
      { key: "stt-auto-send", items: [] },
    ],
  },
  {
    key: "response-style",
    title: t("config.chatSettings.responseStyle"),
    rows: [{ key: "response-style", items: [] }],
  },
  {
    key: "exec-terminal",
    title: t("config.chatSettings.execTerminalTitle"),
    rows: [
      { key: "exec-terminal", items: [] },
      { key: "assistant-space-dir", items: [] },
    ],
  },
  {
    key: "instruction-presets",
    title: t("config.chatSettings.instructionPresetsTitle"),
    rows: [{ key: "instruction-presets", items: [] }],
  },
]);
const responseStyleSegmentOptions = computed(() =>
  props.responseStyleOptions.map((style) => ({
    value: style.id,
    label: t(`responseStyle.${style.id}`),
  })),
);
const imageGenerationModelOptions = computed(() =>
  deriveImageGenerationModelOptions(props.config.imageProviders || []),
);
const emit = defineEmits<{
  (e: "update:responseStyleId", value: string): void;
  (e: "update:pdfReadMode", value: "text" | "image"): void;
  (e: "update:instructionPresets", value: PromptCommandPreset[]): void;
  (e: "patchConversationApiSettings", value: ConversationApiSettingsPatch): void;
  (e: "patchChatSettings", value: ChatSettingsPatch): void;
}>();

function onVisionSelect(value: string) {
  props.config.visionApiConfigId = value || undefined;
  emit("patchConversationApiSettings", {
    visionApiConfigId: props.config.visionApiConfigId ?? null,
  });
}

function onToolReviewSelect(value: string) {
  props.config.toolReviewApiConfigId = value || undefined;
  emit("patchConversationApiSettings", {
    toolReviewApiConfigId: props.config.toolReviewApiConfigId ?? null,
  });
}

function onExpertSelect(value: string) {
  props.config.expertApiConfigId = value || "";
  emit("patchConversationApiSettings", {
    expertApiConfigId: props.config.expertApiConfigId,
  });
}

// 图片生成模型没有独立保存按钮，改动后立即持久化，避免重启后配置丢失。
async function onImageGenerationSelectChange(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  const next = (target?.value || undefined) as string | undefined;
  const previous = props.config.imageGenerationModelId;
  props.config.imageGenerationModelId = next;
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      props.config.imageGenerationModelId = previous;
      console.warn("image generation model save rejected");
    }
  } catch {
    props.config.imageGenerationModelId = previous;
    console.warn("image generation model save failed");
  }
}

function onResponseStyleChange(value: string) {
  emit("update:responseStyleId", value);
  emit("patchChatSettings", {
    responseStyleId: value,
  });
}

function onSttSelectChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value || undefined;
  props.config.sttApiConfigId = value;
  if (!value) {
    props.config.sttAutoSend = false;
  }
  emit("patchConversationApiSettings", {
    sttApiConfigId: props.config.sttApiConfigId ?? null,
    sttAutoSend: !!props.config.sttAutoSend,
  });
}

function onSttAutoSendChange(event: Event) {
  if (!props.config.sttApiConfigId) {
    props.config.sttAutoSend = false;
    emit("patchConversationApiSettings", {
      sttApiConfigId: null,
      sttAutoSend: false,
    });
    return;
  }
  props.config.sttAutoSend = (event.target as HTMLInputElement).checked;
  emit("patchConversationApiSettings", {
    sttAutoSend: !!props.config.sttAutoSend,
  });
}

function randomInstructionPresetId(): string {
  return `instruction-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function normalizeInstructionPresets(value: PromptCommandPreset[]): PromptCommandPreset[] {
  return (Array.isArray(value) ? value : [])
    .map((item) => ({
      id: String(item?.id || "").trim() || randomInstructionPresetId(),
      name: String(item?.prompt || item?.name || "").trim(),
      prompt: String(item?.prompt || item?.name || "").trim(),
    }))
    .filter((item) => !!item.prompt);
}

const instructionPresetsDraft = ref<PromptCommandPreset[]>(normalizeInstructionPresets(props.instructionPresets));

watch(
  () => props.instructionPresets,
  (value) => {
    instructionPresetsDraft.value = normalizeInstructionPresets(value);
  },
  { deep: true },
);

const instructionPresetsDirty = computed(() =>
  JSON.stringify(instructionPresetsDraft.value) !== JSON.stringify(normalizeInstructionPresets(props.instructionPresets)),
);

function addInstructionPreset() {
  instructionPresetsDraft.value = [
    ...instructionPresetsDraft.value,
    {
      id: randomInstructionPresetId(),
      name: "",
      prompt: "",
    },
  ];
}

function removeInstructionPreset(id: string) {
  instructionPresetsDraft.value = instructionPresetsDraft.value.filter((item) => item.id !== id);
}

function saveInstructionPresets() {
  const normalized = instructionPresetsDraft.value
    .map((item) => ({
      id: String(item.id || "").trim() || randomInstructionPresetId(),
      name: String(item.prompt || item.name || "").trim(),
      prompt: String(item.prompt || item.name || "").trim(),
    }))
    .filter((item) => !!item.prompt);
  instructionPresetsDraft.value = normalized;
  emit("update:instructionPresets", normalized);
  emit("patchChatSettings", {
    instructionPresets: normalized,
  });
}
</script>
