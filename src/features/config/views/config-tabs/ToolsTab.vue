<template>
  <div v-if="!toolApiConfig" class="text-sm opacity-70">{{ t("config.tools.noChatLlmProvider") }}</div>
  <template v-else>
    <div class="grid gap-2">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-sm">{{ t("config.tools.maxIterations") }}</span></div>
        <input v-model.number="config.toolMaxIterations" type="number" min="1" max="100" step="1" class="input input-bordered input-sm" />
      </label>
      <div class="card bg-base-100 border border-base-300">
        <div class="flex items-center justify-between gap-3 p-4">
          <span class="text-sm font-medium">{{ t('config.tools.shellWorkspace') }}</span>
          <div class="flex items-center gap-2">
            <button class="btn btn-sm" type="button" @click="addWorkspace">{{ t('config.tools.newWorkspace') }}</button>
            <button class="btn btn-sm btn-primary" :disabled="savingConfig" @click="$emit('saveApiConfig')">
              {{ t('config.tools.save') }}
            </button>
          </div>
        </div>
        <div class="grid gap-3 px-4 pb-4">
          <div v-for="(ws, index) in config.shellWorkspaces" :key="`ws-${index}-${ws.name}`" class="rounded-box border border-base-300 p-3 bg-base-200">
            <div class="flex items-center gap-2 mb-3">
              <input v-model.trim="ws.name" class="input input-bordered input-sm flex-1" :placeholder="t('config.tools.workspaceName')" />
              <button class="btn btn-sm bg-base-100" type="button" :disabled="!!ws.builtIn" @click="pickWorkspacePath(index)">{{ t('config.tools.selectPath') }}</button>
              <button class="btn btn-sm btn-ghost" type="button" :disabled="!!ws.builtIn" @click="removeWorkspace(index)">{{ t('config.tools.delete') }}</button>
            </div>
            <input v-model.trim="ws.path" class="input input-bordered input-sm w-full font-mono" :placeholder="t('config.tools.directoryPath')" :disabled="!!ws.builtIn" />
          </div>
        </div>
        <div class="mt-3 px-4 pb-4 text-[11px] opacity-70">
          {{ t('config.tools.workspaceHint') }}
        </div>
      </div>
    </div>
    <div class="mt-4"></div>
    <div v-if="!toolApiConfig.enableTools" class="text-sm opacity-70">{{ t("config.tools.disabledHint") }}</div>
    <div v-else class="grid gap-2">
      <ToolListCard
        :title="t('config.mcpToolList.toolList')"
        :items="toolListItems"
        :no-description-text="t('config.mcpToolList.noDescription')"
        @toggle-item="onToggleToolItem"
      >
        <template #item-extra="{ item }">
          <div v-if="isImageBoundTool(item.id) && !toolApiConfig.enableImage" class="text-[11px] text-warning mt-1">
            {{ t("config.tools.imageCapabilityRequired") }}
          </div>
          <div v-if="showGitInstallLink(item.id)" class="text-[11px] text-warning mt-1 flex items-center gap-2">
            <span>{{ t("config.tools.gitRequiredHint") }}</span>
            <button class="btn btn-sm bg-base-100" @click="openGitDownloadLink">
              {{ t("config.tools.installGit") }}
            </button>
          </div>
        </template>
        <template #item-debug="{ item }">
          <div v-if="item.id === 'desktop-screenshot'" class="mt-2">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[11px] opacity-70">{{ t("config.tools.desktopScreenshotDesc") }}</div>
              <button class="btn btn-sm btn-primary" :disabled="screenshotRunning || !toolApiConfig?.enableImage" @click="runDesktopScreenshot">
                {{ t("config.tools.runOnce") }}
              </button>
            </div>
            <div v-if="screenshotResult" class="mt-2 text-[11px] opacity-80 break-all">{{ screenshotResult }}</div>
          </div>
          <div v-if="item.id === 'desktop-wait'" class="mt-2">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[11px] opacity-70">{{ t("config.tools.desktopWaitDesc") }}</div>
              <div class="flex items-center gap-2">
                <input v-model.number="waitMs" type="number" min="1" max="120000" step="100" class="input input-bordered input-sm w-24" />
                <button class="btn btn-sm btn-primary" :disabled="waitRunning || !toolApiConfig?.enableImage" @click="runDesktopWait">
                  {{ t("config.tools.runOnce") }}
                </button>
              </div>
            </div>
            <div v-if="waitResult" class="mt-2 text-[11px] opacity-80 break-all">{{ waitResult }}</div>
          </div>
          <div v-if="item.id === 'shell-exec'" class="mt-2">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[11px] opacity-70">{{ t("config.tools.terminalSelfCheckDesc") }}</div>
              <button class="btn btn-sm btn-primary" :disabled="terminalSelfCheckRunning" @click="runTerminalSelfCheck">
                {{ t("config.tools.terminalSelfCheck") }}
              </button>
            </div>
            <pre
              v-if="terminalSelfCheckResult"
              class="mt-2 text-[11px] opacity-80 whitespace-pre-wrap break-all font-mono bg-base-200 border border-base-300 rounded p-2"
            >{{ terminalSelfCheckResult }}</pre>
          </div>
        </template>
      </ToolListCard>
    </div>
  </template>
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
import type { ApiConfigItem, AppConfig, ToolLoadStatus } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import { open } from "@tauri-apps/plugin-dialog";
import ToolListCard, { type ToolListItem } from "../../components/ToolListCard.vue";

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
  toolApiConfig: ApiConfigItem | null;
  toolStatuses: ToolLoadStatus[];
  savingConfig: boolean;
}>();

const emit = defineEmits<{
  (e: "openMemoryViewer"): void;
  (e: "toolSwitchChanged"): void;
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
const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";
function addWorkspace() {
  if (!Array.isArray(props.config.shellWorkspaces)) props.config.shellWorkspaces = [];
  props.config.shellWorkspaces.push({
    name: "",
    path: "",
    builtIn: false,
  });
}

function removeWorkspace(index: number) {
  const item = props.config.shellWorkspaces[index];
  if (!item || item.builtIn) return;
  props.config.shellWorkspaces.splice(index, 1);
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

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? t("config.tools.statusUnknown");
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
  if (id === "bing-search") return t("config.tools.descBingSearch");
  if (id === "memory-save") return t("config.tools.descMemorySave");
  if (id === "desktop-screenshot") return t("config.tools.descDesktopScreenshot");
  if (id === "desktop-wait") return t("config.tools.descDesktopWait");
  if (id === "shell-exec") return t("config.tools.descTerminalExec");
  if (id === "shell-switch-workspace") return t("config.tools.descTerminalPathAccess");
  if (id === "refresh-mcp-skills") return t("config.tools.descRefreshMcpSkills");
  return t("config.tools.descGeneric");
}

function isImageBoundTool(id: string): boolean {
  return id === "desktop-screenshot" || id === "desktop-wait";
}

function toolSwitchDisabled(_id: string): boolean {
  return false;
}

function isToolRunning(id: string): boolean {
  if (id === "desktop-screenshot") return screenshotRunning.value;
  if (id === "desktop-wait") return waitRunning.value;
  if (id === "shell-exec") return terminalSelfCheckRunning.value;
  return false;
}

const toolListItems = computed<ToolListItem[]>(() =>
  (props.toolApiConfig?.tools ?? []).map((tool) => ({
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

function onToggleToolItem(payload: { id: string; enabled: boolean }) {
  const target = props.toolApiConfig?.tools.find((tool) => tool.id === payload.id);
  if (!target) return;
  target.enabled = payload.enabled;
  emit("toolSwitchChanged");
}

function showGitInstallLink(id: string): boolean {
  if (id !== "shell-exec") return false;
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
