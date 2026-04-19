<template>
  <div class="grid gap-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="flex items-center justify-between gap-3 p-4">
        <span class="text-sm font-medium">{{ t('config.tools.shellWorkspace') }}</span>
        <div class="flex items-center gap-2">
          <button class="btn btn-sm" type="button" @click="openShellWorkspaceDir">{{ t('config.tools.openDir') }}</button>
          <button class="btn btn-sm" type="button" :disabled="shellWorkspacePathResetting" @click="resetShellWorkspacePath">{{ t('config.tools.resetWorkspacePath') }}</button>
          <button class="btn btn-sm" type="button" :disabled="shellWorkspaceInitializing" @click="initializeShellWorkspace">{{ t('config.tools.initializeWorkspace') }}</button>
          <button class="btn btn-sm btn-primary" :disabled="savingConfig" @click="$emit('saveApiConfig')">
            {{ t('config.tools.save') }}
          </button>
        </div>
      </div>
      <div class="grid gap-3 px-4 pb-4">
        <div v-for="(ws, index) in config.shellWorkspaces" :key="`ws-${index}-${ws.name}`">
          <div class="mb-3">
            <input v-model.trim="ws.name" class="input input-bordered input-sm w-full" :placeholder="t('config.tools.workspaceName')" />
          </div>
          <div class="flex items-center gap-2">
            <input v-model.trim="ws.path" class="input input-bordered input-sm flex-1 font-mono" :placeholder="t('config.tools.directoryPath')" />
            <button class="btn btn-sm btn-neutral" type="button" @click="pickWorkspacePath(index)">{{ t('config.tools.modifyWorkspaceDir') }}</button>
          </div>
        </div>
        <div v-if="isWindowsHost" class="grid gap-2">
          <div class="text-[12px] font-medium">{{ t("config.tools.terminalRuntime") }}</div>
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
          <div class="text-[11px] opacity-70">
            {{ t("config.tools.terminalRuntimeHint") }}
          </div>
          <div v-if="showGitInstallHintInWorkspace" class="text-[11px] bg-warning/10 text-base-content rounded px-2 py-1 flex items-center gap-2">
            <span>{{ t("config.tools.gitRequiredHint") }}</span>
            <button class="btn btn-sm bg-base-100" @click="openGitDownloadLink">
              {{ t("config.tools.installGit") }}
            </button>
          </div>
        </div>
      </div>
      <div class="mt-3 px-4 pb-4 text-[11px] opacity-70">
        {{ t('config.tools.workspaceHint') }}
      </div>
      <div v-if="shellWorkspaceStatus" class="px-4 pb-4 text-[11px]" :class="shellWorkspaceStatusError ? 'text-error' : 'opacity-70'">
        {{ shellWorkspaceStatus }}
      </div>
    </div>

    <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
      <div class="px-4 py-3 border-b border-base-300 flex items-center justify-between gap-3">
        <div>
          <div class="font-medium">{{ t("config.tools.systemCatalogTitle") }}</div>
          <div class="text-[11px] opacity-60">{{ t("config.tools.systemCatalogReadonly") }}</div>
        </div>
      </div>

      <div v-if="toolDefinitions.length" class="divide-y divide-base-300/60">
        <div
          v-for="item in toolDefinitions"
          :key="item.function.name"
          class="px-4 py-3"
        >
          <div class="min-w-0">
            <div class="font-medium">{{ item.function.name }}</div>
            <div class="text-[11px] opacity-60 whitespace-pre-wrap">{{ item.function.description || t("config.mcpToolList.noDescription") }}</div>
            <div v-if="toolParameterSummary(item.function.name).length" class="mt-1 flex flex-wrap gap-1">
              <span
                v-for="paramText in toolParameterSummary(item.function.name)"
                :key="`${item.function.name}-param-${paramText}`"
                class="text-[10px] px-1.5 py-0.5 rounded bg-base-200 border border-base-300/70 opacity-80"
              >
                {{ paramText }}
              </span>
            </div>
            <div v-if="toolParameterExamples(item.function.name).length" class="mt-1 grid gap-1">
              <pre
                v-for="example in toolParameterExamples(item.function.name)"
                :key="`${item.function.name}-example-${example}`"
                class="text-[10px] leading-4 px-2 py-1 rounded bg-base-200 border border-base-300/70 opacity-90 whitespace-pre-wrap overflow-x-auto"
              >{{ example }}</pre>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="text-sm opacity-50 text-center py-4">{{ t("config.mcpToolList.empty") }}</div>
    </div>

    <dialog ref="initializeWorkspaceDialog" class="modal">
      <div class="modal-box max-w-md p-4">
        <h3 class="text-sm font-semibold">{{ t("config.tools.initializeWorkspace") }}</h3>
        <p class="mt-3 text-sm whitespace-pre-wrap">{{ t("config.tools.initializeWorkspaceConfirm") }}</p>
        <div class="modal-action mt-4">
          <button class="btn btn-sm btn-ghost" type="button" @click="cancelInitializeWorkspace">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-primary" type="button" @click="confirmInitializeWorkspace">
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click="cancelInitializeWorkspace">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type {
  AppConfig,
  FrontendToolDefinition,
  ToolLoadStatus,
} from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import { open } from "@tauri-apps/plugin-dialog";

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
  toolStatuses: ToolLoadStatus[];
  savingConfig: boolean;
}>();

defineEmits<{
  (e: "saveApiConfig"): void;
}>();

const { t } = useI18n();
const toolDefinitions = ref<FrontendToolDefinition[]>([]);
const shellWorkspaceInitializing = ref(false);
const shellWorkspacePathResetting = ref(false);
const shellWorkspaceStatus = ref("");
const shellWorkspaceStatusError = ref(false);
const initializeWorkspaceDialog = ref<HTMLDialogElement | null>(null);
let resolveInitializeWorkspaceConfirm: ((value: boolean) => void) | null = null;
const terminalShellOptionsLoading = ref(false);
const terminalShellOptions = ref<TerminalShellCandidate[]>([]);
const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";
const isWindowsHost = typeof navigator !== "undefined" && /windows/i.test(String(navigator.userAgent || ""));
const terminalShellKindValue = computed(() => String(props.config.terminalShellKind || "auto"));
function setShellWorkspaceStatus(text: string, isError = false) {
  shellWorkspaceStatus.value = text;
  shellWorkspaceStatusError.value = isError;
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

async function loadToolCatalog() {
  try {
    const list = await invokeTauri<FrontendToolDefinition[]>("list_tool_catalog");
    toolDefinitions.value = Array.isArray(list) ? list : [];
  } catch {
    toolDefinitions.value = [];
  }
}

function onTerminalShellKindChange(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  const next = String(target?.value || "auto").trim() || "auto";
  props.config.terminalShellKind = next;
}

async function openShellWorkspaceDir() {
  try {
    const opened = await invokeTauri<string>("open_chat_shell_workspace_dir", {
      input: { workspacePath: props.config.shellWorkspaces[0]?.path || "" },
    });
    setShellWorkspaceStatus(t("config.tools.openDirOpened", { path: opened }));
  } catch (error) {
    setShellWorkspaceStatus(t("config.tools.openDirFailed", { err: toErrorMessage(error) }), true);
  }
}

async function initializeShellWorkspace() {
  if (shellWorkspaceInitializing.value) return;
  const confirmed = await requestInitializeWorkspaceConfirm();
  if (!confirmed) return;
  shellWorkspaceInitializing.value = true;
  try {
    const root = await invokeTauri<string>("reset_chat_shell_workspace", {
      input: { workspacePath: props.config.shellWorkspaces[0]?.path || "" },
    });
    setShellWorkspaceStatus(t("config.tools.initializeWorkspaceDone", { path: root }));
  } catch (error) {
    setShellWorkspaceStatus(t("config.tools.initializeWorkspaceFailed", { err: toErrorMessage(error) }), true);
  } finally {
    shellWorkspaceInitializing.value = false;
  }
}

function requestInitializeWorkspaceConfirm(): Promise<boolean> {
  const dialog = initializeWorkspaceDialog.value;
  if (!dialog) return Promise.resolve(false);
  return new Promise<boolean>((resolve) => {
    resolveInitializeWorkspaceConfirm = resolve;
    dialog.showModal();
  });
}

function finishInitializeWorkspaceConfirm(value: boolean) {
  const dialog = initializeWorkspaceDialog.value;
  if (dialog?.open) {
    dialog.close();
  }
  resolveInitializeWorkspaceConfirm?.(value);
  resolveInitializeWorkspaceConfirm = null;
}

function confirmInitializeWorkspace() {
  finishInitializeWorkspaceConfirm(true);
}

function cancelInitializeWorkspace() {
  finishInitializeWorkspaceConfirm(false);
}

async function resetShellWorkspacePath() {
  if (shellWorkspacePathResetting.value) return;
  shellWorkspacePathResetting.value = true;
  try {
    const defaultPath = await invokeTauri<string>("get_default_chat_shell_workspace_path");
    if (!Array.isArray(props.config.shellWorkspaces) || props.config.shellWorkspaces.length === 0) {
      props.config.shellWorkspaces = [{
        id: "system-workspace",
        name: defaultWorkspaceNameFromPath(defaultPath) || "默认工作空间",
        path: defaultPath,
        level: "system",
        access: "full_access",
        builtIn: true,
      }];
    } else {
      const target = props.config.shellWorkspaces[0];
      target.id = String(target.id || "").trim() || "system-workspace";
      target.path = defaultPath;
      target.level = "system";
      target.access = "full_access";
      if (!String(target.name || "").trim()) {
        target.name = defaultWorkspaceNameFromPath(defaultPath) || "默认工作空间";
      }
      target.builtIn = true;
    }
    setShellWorkspaceStatus(t("config.tools.resetWorkspacePathDone", { path: defaultPath }));
  } catch (error) {
    setShellWorkspaceStatus(t("config.tools.resetWorkspacePathFailed", { err: toErrorMessage(error) }), true);
  } finally {
    shellWorkspacePathResetting.value = false;
  }
}

function defaultWorkspaceNameFromPath(path: string): string {
  const raw = String(path || "").trim();
  if (!raw) return "";
  const normalized = raw.replace(/\\/g, "/").replace(/\/+$/, "");
  const part = normalized.split("/").pop() || "";
  return part.trim();
}

async function pickWorkspacePath(index: number) {
  const item = props.config.shellWorkspaces[index];
  if (!item) return;
  const picked = await open({
    directory: true,
    multiple: false,
    defaultPath: item.path || undefined,
  });
  if (!picked || Array.isArray(picked)) return;
  item.path = String(picked);
  if (!String(item.name || "").trim()) {
    item.name = defaultWorkspaceNameFromPath(item.path) || `workspace-${index + 1}`;
  }
}

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

const showGitInstallHintInWorkspace = computed(
  () => isWindowsHost && toolStatusById("exec")?.status === "unavailable",
);

function definitionById(id: string): FrontendToolDefinition | undefined {
  return toolDefinitions.value.find((item) => item.function?.name === id);
}

function toolParameterSummary(id: string): string[] {
  const definition = definitionById(id);
  const parameters = definition?.function?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  const requiredRaw = Array.isArray(root.required) ? root.required : [];
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  const properties = propertiesRaw as Record<string, unknown>;
  return Object.entries(properties).map(([name, schema]) => {
    const shape = schema && typeof schema === "object" ? (schema as Record<string, unknown>) : {};
    const typeValue = String(shape.type || "any");
    const required = requiredRaw.includes(name) ? "*" : "";
    const enumValues = Array.isArray(shape.enum) ? ` [${shape.enum.map(String).join(", ")}]` : "";
    const minText = shape.minimum !== undefined ? ` >= ${shape.minimum}` : "";
    const maxText = shape.maximum !== undefined ? ` <= ${shape.maximum}` : "";
    const desc = String(shape.description || "").trim();
    const rangeText = `${enumValues}${minText}${maxText}`.trim();
    const base = `${required}${name}: ${typeValue}${rangeText ? ` ${rangeText}` : ""}`;
    return desc ? `${base} (${desc})` : base;
  });
}

function toolParameterExamples(id: string): string[] {
  const definition = definitionById(id);
  const parameters = definition?.function?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  const properties = propertiesRaw as Record<string, unknown>;
  const examples: string[] = [];
  for (const [name, schema] of Object.entries(properties)) {
    const shape = schema && typeof schema === "object" ? (schema as Record<string, unknown>) : {};
    const singleExample = shape.example;
    if (typeof singleExample === "string" && singleExample.trim()) {
      examples.push(`${name} 示例:\n${singleExample.trim()}`);
    }
    const exampleList = Array.isArray(shape.examples) ? shape.examples : [];
    for (const rawExample of exampleList) {
      if (typeof rawExample === "string" && rawExample.trim()) {
        examples.push(`${name} 示例:\n${rawExample.trim()}`);
      }
    }
  }
  return Array.from(new Set(examples));
}

function openGitDownloadLink() {
  void invokeTauri("open_external_url", { url: GIT_DOWNLOAD_URL });
}

onMounted(() => {
  void loadTerminalShellCandidates();
  void loadToolCatalog();
});
</script>
