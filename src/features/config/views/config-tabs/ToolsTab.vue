<template>
  <template v-if="selectedPersona">
    <div class="grid gap-3">
      <!-- Shell 工作区 -->
      <div class="card bg-base-100 border border-base-300">
        <div class="flex items-center justify-between gap-3 p-4">
          <span class="text-sm font-medium">{{ t('config.tools.shellWorkspace') }}</span>
          <div class="flex items-center gap-2">
            <button class="btn btn-sm" type="button" @click="openShellWorkspaceDir">{{ t('config.tools.openDir') }}</button>
            <button class="btn btn-sm" type="button" :disabled="shellWorkspaceResetting" @click="resetShellWorkspace">{{ t('config.tools.resetWorkspace') }}</button>
            <button class="btn btn-sm btn-primary" :disabled="savingConfig" @click="$emit('saveApiConfig')">
              {{ t('config.tools.save') }}
            </button>
          </div>
        </div>
        <div class="grid gap-3 px-4 pb-4">
          <div v-for="(ws, index) in config.shellWorkspaces" :key="`ws-${index}-${ws.name}`">
            <div class="flex items-center gap-2 mb-3">
              <input v-model.trim="ws.name" class="input input-bordered input-sm flex-1" :placeholder="t('config.tools.workspaceName')" />
              <button class="btn btn-sm btn-neutral" type="button" @click="pickWorkspacePath(index)">{{ t('config.tools.modifyWorkspace') }}</button>
            </div>
            <input v-model.trim="ws.path" class="input input-bordered input-sm w-full font-mono" :placeholder="t('config.tools.directoryPath')" />
          </div>
        </div>
        <div class="mt-3 px-4 pb-4 text-[11px] opacity-70">
          {{ t('config.tools.workspaceHint') }}
        </div>
        <div v-if="shellWorkspaceStatus" class="px-4 pb-4 text-[11px]" :class="shellWorkspaceStatusError ? 'text-error' : 'opacity-70'">
          {{ shellWorkspaceStatus }}
        </div>
      </div>
    </div>
    <div class="mt-4"></div>
    <div v-if="toolApiConfig && !toolApiConfig.enableTools" class="text-sm opacity-70">{{ t("config.tools.disabledHint") }}</div>
    <div v-else class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
      <!-- 头部：人格选择 + 最大迭代 + 标题 -->
      <div class="flex items-center gap-3 px-3 py-2 border-b border-base-300/70 flex-wrap">
        <div class="flex items-center gap-2 shrink-0">
          <div class="text-sm font-bold text-base-content whitespace-nowrap">{{ t("config.tools.personaLabel") }}</div>
          <select
            :value="personaEditorId"
            class="select select-bordered select-sm min-w-32"
            @change="emit('update:personaEditorId', String(($event.target as HTMLSelectElement).value || ''))"
          >
            <option v-for="persona in personas" :key="persona.id" :value="persona.id">{{ persona.name }}</option>
          </select>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <div class="text-sm font-bold text-base-content whitespace-nowrap">{{ t("config.tools.maxIterations") }}</div>
          <input v-model.number="config.toolMaxIterations" type="number" min="1" max="100" step="1" class="input input-bordered input-sm w-20" />
        </div>
        <div class="font-medium ml-auto">{{ t('config.mcpToolList.toolList') }}<span v-if="toolListItems.length">（{{ toolListItems.length }}）</span></div>
      </div>
      <!-- 当前编辑状态提示 -->
      <div class="px-3 py-1.5 bg-base-200/30 text-[11px] opacity-70">
        {{ t("config.tools.editingLabel") }}{{ selectedPersona.name }}
        <template v-if="currentDepartment">
          · {{ t("config.tools.currentDepartmentLabel") }}{{ currentDepartment.name }}
        </template>
        <template v-if="toolApiConfig">
          · {{ t("config.tools.currentModelLabel") }}{{ toolApiConfig.name }}
        </template>
        <template v-else>
          · {{ t("config.tools.unassignedHint") }}
        </template>
      </div>
      <!-- 工具列表内容 -->
      <div v-if="toolListItems.length" class="divide-y divide-base-300/60">
        <div
          v-for="item in toolListItems"
          :key="item.id"
          class="px-3 py-2"
        >
          <div class="flex items-start gap-3">
            <input
              type="checkbox"
              class="toggle toggle-sm toggle-success mt-1 shrink-0"
              :checked="item.enabled"
              :disabled="item.toggleDisabled"
              @change="onToggle($event, item.id)"
            />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <div v-if="item.statusClass" class="w-2.5 h-2.5 rounded-full shrink-0" :class="item.statusClass" :title="item.statusTitle || ''"></div>
                <div class="font-medium">{{ item.name }}</div>
                <span v-if="item.running" class="loading loading-spinner loading-sm"></span>
              </div>
              <div class="text-[11px] opacity-60">{{ item.description || t("config.mcpToolList.noDescription") }}</div>
              <div v-if="statusDetail(item.id)" class="text-[11px] mt-1 rounded px-2 py-1" :class="statusMessageClass(item.id)">
                {{ statusDetail(item.id) }}
              </div>
              <!-- 额外信息 -->
              <div v-if="isImageBoundTool(item.id) && !toolApiConfig?.enableImage" class="text-[11px] bg-warning/10 text-base-content mt-1 rounded px-2 py-1">
                        {{ t("config.tools.imageCapabilityRequired") }}
                      </div>
                      <div v-if="showGitInstallLink(item.id)" class="text-[11px] bg-warning/10 text-base-content mt-1 rounded px-2 py-1 flex items-center gap-2">
                        <span>{{ t("config.tools.gitRequiredHint") }}</span>                <button class="btn btn-sm bg-base-100" @click="openGitDownloadLink">
                  {{ t("config.tools.installGit") }}
                </button>
              </div>
              <!-- 调试操作 -->
              <div v-if="item.id === 'screenshot'" class="mt-2">
                <div class="flex items-center justify-between gap-2">
                  <div class="text-[11px] opacity-70">{{ t("config.tools.desktopScreenshotDesc") }}</div>
                  <button class="btn btn-sm btn-primary" :disabled="screenshotRunning || !toolApiConfig?.enableImage || toolSwitchDisabled(item.id)" @click="runDesktopScreenshot">
                    {{ t("config.tools.runOnce") }}
                  </button>
                </div>
                <div v-if="screenshotResult" class="mt-2 text-[11px] opacity-80 break-all">{{ screenshotResult }}</div>
              </div>
              <div v-if="item.id === 'wait'" class="mt-2">
                <div class="flex items-center justify-between gap-2">
                  <div class="text-[11px] opacity-70">{{ t("config.tools.desktopWaitDesc") }}</div>
                  <div class="flex items-center gap-2">
                    <input v-model.number="waitMs" type="number" min="1" max="120000" step="100" class="input input-bordered input-sm w-24" />
                    <button class="btn btn-sm btn-primary" :disabled="waitRunning || !toolApiConfig?.enableImage || toolSwitchDisabled(item.id)" @click="runDesktopWait">
                      {{ t("config.tools.runOnce") }}
                    </button>
                  </div>
                </div>
                <div v-if="waitResult" class="mt-2 text-[11px] opacity-80 break-all">{{ waitResult }}</div>
              </div>
              <div v-if="item.id === 'exec'" class="mt-2">
                <div class="flex items-center justify-between gap-2">
                  <div class="text-[11px] opacity-70">{{ t("config.tools.terminalSelfCheckDesc") }}</div>
                  <button class="btn btn-sm btn-primary" :disabled="terminalSelfCheckRunning || toolSwitchDisabled(item.id)" @click="runTerminalSelfCheck">
                    {{ t("config.tools.terminalSelfCheck") }}
                  </button>
                </div>
                <pre
                  v-if="terminalSelfCheckResult"
                  class="mt-2 text-[11px] opacity-80 whitespace-pre-wrap break-all font-mono bg-base-200 border border-base-300 rounded p-2"
                >{{ terminalSelfCheckResult }}</pre>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="text-sm opacity-50 text-center py-4">{{ t("config.mcpToolList.empty") }}</div>
    </div>
  </template>
  <div v-else class="text-sm opacity-70">{{ t("config.tools.noChatLlmProvider") }}</div>
  <dialog ref="screenshotDialogRef" class="modal">
    <div class="modal-box max-w-5xl">
      <div class="text-sm font-medium mb-2">{{ t("config.tools.desktopScreenshotTitle") }}</div>
      <img v-if="screenshotPreviewDataUrl" :src="screenshotPreviewDataUrl" alt="desktop screenshot preview" class="w-full rounded border border-base-300" />
      <div class="modal-action">
        <form method="dialog">
          <button class="btn btn-sm">{{ t("common.close") }}</button>
        </form>
      </div>
    </div>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, PersonaProfile, ToolLoadStatus } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import { open } from "@tauri-apps/plugin-dialog";
import { type ToolListItem } from "../../components/ToolListCard.vue";

type TerminalSelfCheckStep = {
  name: string;
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
};

type TerminalSelfCheckResult = {
  ok: boolean;
  blockedReason?: string;
  message?: string;
  sessionId?: string;
  rootPath?: string;
  cwd?: string;
  shellKind?: string;
  shellPath?: string;
  allowedProjectRoots?: string[];
  steps?: TerminalSelfCheckStep[];
};

const props = defineProps<{
  config: AppConfig;
  personas: PersonaProfile[];
  personaEditorId: string;
  selectedPersona: PersonaProfile | null;
  toolApiConfig: ApiConfigItem | null;
  toolStatuses: ToolLoadStatus[];
  savingConfig: boolean;
}>();

const emit = defineEmits<{
  (e: "openMemoryViewer"): void;
  (e: "toolSwitchChanged"): void;
  (e: "update:personaEditorId", value: string): void;
  (e: "saveApiConfig"): void;
}>();

const { t } = useI18n();
const screenshotRunning = ref(false);
const waitRunning = ref(false);
const terminalSelfCheckRunning = ref(false);
const screenshotResult = ref("");
const waitResult = ref("");
const terminalSelfCheckResult = ref("");
const waitMs = ref(800);
const screenshotPreviewDataUrl = ref("");
const screenshotDialogRef = ref<HTMLDialogElement | null>(null);
const shellWorkspaceResetting = ref(false);
const shellWorkspaceStatus = ref("");
const shellWorkspaceStatusError = ref(false);
const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";

function setShellWorkspaceStatus(text: string, isError = false) {
  shellWorkspaceStatus.value = text;
  shellWorkspaceStatusError.value = isError;
}

async function openShellWorkspaceDir() {
  try {
    const opened = await invokeTauri<string>("open_chat_shell_workspace_dir");
    setShellWorkspaceStatus(t("config.tools.openDirOpened", { path: opened }));
  } catch (error) {
    setShellWorkspaceStatus(t("config.tools.openDirFailed", { err: toErrorMessage(error) }), true);
  }
}

async function resetShellWorkspace() {
  if (shellWorkspaceResetting.value) return;
  if (!window.confirm(t("config.tools.resetWorkspaceConfirm"))) return;
  shellWorkspaceResetting.value = true;
  try {
    const root = await invokeTauri<string>("reset_chat_shell_workspace");
    setShellWorkspaceStatus(t("config.tools.resetWorkspaceDone", { path: root }));
  } catch (error) {
    setShellWorkspaceStatus(t("config.tools.resetWorkspaceFailed", { err: toErrorMessage(error) }), true);
  } finally {
    shellWorkspaceResetting.value = false;
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

const currentDepartment = computed(() =>
  props.config.departments.find((item) => (item.agentIds || []).includes(props.personaEditorId)) ?? null,
);

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? t("config.tools.statusUnknown");
}

function statusDetail(id: string): string {
  return String(toolStatusById(id)?.detail || "").trim();
}

function statusMessageClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "failed") return "text-error bg-error/10";
  if (status === "unavailable") return "text-base-content bg-warning/10";
  return "opacity-70";
}

function statusDotClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "loaded") return "bg-success";
  if (status === "failed" || status === "timeout") return "bg-error";
  if (status === "unavailable") return "bg-warning";
  if (status === "disabled") return "bg-base-content/30";
  return "bg-base-content/20";
}

function toolDescription(id: string): string {
  if (id === "fetch") return t("config.tools.descFetch");
  if (id === "websearch") return t("config.tools.descBingSearch");
  if (id === "remember") return t("config.tools.descRemember");
  if (id === "recall") return t("config.tools.descRecall");
  if (id === "screenshot") return t("config.tools.descDesktopScreenshot");
  if (id === "wait") return t("config.tools.descDesktopWait");
  if (id === "exec") return t("config.tools.descTerminalExec");
  
  if (id === "reload") return t("config.tools.descReload");
  if (id === "task") return t("config.tools.descTask");
  if (id === "delegate") return t("config.tools.descDelegate");
  if (id === "handoff") return t("config.tools.descHandoff");
  return t("config.tools.descGeneric");
}

function isImageBoundTool(id: string): boolean {
  return id === "screenshot" || id === "wait";
}

function toolSwitchDisabled(id: string): boolean {
  return toolStatusById(id)?.status === "unavailable";
}

function isToolRunning(id: string): boolean {
  if (id === "screenshot") return screenshotRunning.value;
  if (id === "wait") return waitRunning.value;
  if (id === "exec") return terminalSelfCheckRunning.value;
  return false;
}

const toolListItems = computed<ToolListItem[]>(() =>
  (props.selectedPersona?.tools ?? []).map((tool) => ({
    id: tool.id,
    name: tool.id,
    description: toolDescription(tool.id),
    enabled: !!tool.enabled,
    toggleDisabled: toolSwitchDisabled(tool.id),
    running: isToolRunning(tool.id),
    statusClass: statusDotClass(tool.id),
    statusTitle: statusText(tool.id),
  })),
);

function onToggle(event: Event, id: string) {
  const target = event.target as HTMLInputElement | null;
  const payload = { id, enabled: !!target?.checked };
  const tool = props.selectedPersona?.tools.find((t) => t.id === payload.id);
  if (!tool) return;
  if (toolSwitchDisabled(id)) return;
  tool.enabled = payload.enabled;
  emit("toolSwitchChanged");
}

function showGitInstallLink(id: string): boolean {
  if (id !== "exec") return false;
  const status = toolStatusById(id);
  return status?.status === "unavailable";
}

function openGitDownloadLink() {
  void invokeTauri("open_external_url", { url: GIT_DOWNLOAD_URL });
}

function normalizeOutputText(value: unknown): string {
  const text = String(value ?? "").trim();
  return text.length > 0 ? text : "(empty)";
}

function formatTerminalSelfCheckResult(payload: TerminalSelfCheckResult): string {
  if (payload.ok) {
    return `${t("config.tools.lastResult")}: OK`;
  }

  const reasons: string[] = [];
  if (payload.message) reasons.push(`message=${payload.message}`);
  if (payload.blockedReason) reasons.push(`blockedReason=${payload.blockedReason}`);

  const steps = Array.isArray(payload.steps) ? payload.steps : [];
  for (const step of steps) {
    if (step.ok) continue;
    reasons.push(`${step.name}: exit=${step.exitCode}`);
    if (String(step.stderr || "").trim()) {
      reasons.push(`stderr=${normalizeOutputText(step.stderr)}`);
    } else if (String(step.stdout || "").trim()) {
      reasons.push(`stdout=${normalizeOutputText(step.stdout)}`);
    }
    break;
  }

  if (reasons.length === 0) {
    reasons.push("unknown error");
  }
  return `${t("config.tools.lastResult")}: FAILED | ${reasons.join(" | ")}`;
}

async function runTerminalSelfCheck() {
  terminalSelfCheckRunning.value = true;
  try {
    const res = await invokeTauri<TerminalSelfCheckResult>("terminal_self_check");
    terminalSelfCheckResult.value = formatTerminalSelfCheckResult(res);
  } catch (error) {
    terminalSelfCheckResult.value = `${t("config.tools.lastResult")}: ${toErrorMessage(error)}`;
  } finally {
    terminalSelfCheckRunning.value = false;
  }
}

async function runDesktopScreenshot() {
  if (!props.toolApiConfig?.enableImage) return;
  screenshotRunning.value = true;
  try {
    const start = performance.now();
    const res = await invokeTauri<{
      path?: string;
      imageMime: string;
      imageBase64: string;
      width: number;
      height: number;
      elapsedMs: number;
      captureMs: number;
      encodeMs: number;
      saveMs?: number;
    }>("desktop_screenshot", {
      input: { mode: "desktop", webpQuality: 70 },
    });
    const invokeRoundTripMs = Math.round(performance.now() - start);

    const renderStart = performance.now();
    screenshotPreviewDataUrl.value = `data:${res.imageMime};base64,${res.imageBase64}`;
    await nextTick();
    screenshotDialogRef.value?.showModal();
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    const modalRenderMs = Math.round(performance.now() - renderStart);

    const saveInfo = res.path ? `, ${res.path}` : "";
    screenshotResult.value =
      `${t("config.tools.lastResult")}: ${res.width}x${res.height}` +
      ` | backend=${res.elapsedMs}ms (capture=${res.captureMs}ms, encode=${res.encodeMs}ms` +
      `${typeof res.saveMs === "number" ? `, save=${res.saveMs}ms` : ""})` +
      ` | roundTrip=${invokeRoundTripMs}ms | render=${modalRenderMs}ms${saveInfo}`;
  } catch (error) {
    screenshotResult.value = `${t("config.tools.lastResult")}: ${toErrorMessage(error)}`;
  } finally {
    screenshotRunning.value = false;
  }
}

async function runDesktopWait() {
  if (!props.toolApiConfig?.enableImage) return;
  waitRunning.value = true;
  try {
    const ms = Math.max(1, Math.min(120000, Number(waitMs.value || 800)));
    const res = await invokeTauri<{
      waitedMs: number;
      elapsedMs: number;
    }>("desktop_wait", {
      input: { mode: "sleep", ms },
    });
    waitResult.value = `${t("config.tools.lastResult")}: waited=${res.waitedMs}ms, elapsed=${res.elapsedMs}ms`;
  } catch (error) {
    waitResult.value = `${t("config.tools.lastResult")}: ${toErrorMessage(error)}`;
  } finally {
    waitRunning.value = false;
  }
}
</script>
