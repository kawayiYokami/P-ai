<template>
  <div v-if="!toolApiConfig" class="text-xs opacity-70">{{ t("config.tools.noChatApi") }}</div>
  <template v-else>
    <div class="grid gap-2">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.tools.maxIterations") }}</span></div>
        <input v-model.number="config.toolMaxIterations" type="number" min="1" max="100" step="1" class="input input-bordered input-sm" />
      </label>
    </div>
    <div v-if="!toolApiConfig.enableTools" class="text-xs opacity-70">{{ t("config.tools.disabledHint") }}</div>
    <div v-else class="grid gap-2">
      <div v-for="tool in toolApiConfig.tools" :key="tool.id" class="card card-compact bg-base-100 border border-base-300 relative">
        <div class="absolute left-2 top-2 w-2.5 h-2.5 rounded-full" :class="statusDotClass(tool.id)" :title="statusText(tool.id)"></div>
        <div v-if="isToolRunning(tool.id)" class="absolute left-6 top-[3px]">
          <span class="loading loading-spinner loading-xs"></span>
        </div>
        <div class="card-body py-2 px-3">
          <div class="flex items-center justify-between gap-2">
            <div class="min-w-0">
              <div class="text-xs font-medium pl-3">{{ tool.id }}</div>
              <div class="text-[11px] opacity-70 pl-3">{{ toolDescription(tool.id) }}</div>
              <div v-if="isImageBoundTool(tool.id) && !toolApiConfig.enableImage" class="text-[11px] text-warning pl-3 mt-1">
                {{ t("config.tools.imageCapabilityRequired") }}
              </div>
            </div>
            <div class="flex items-center gap-2">
              <label class="label cursor-pointer py-0 gap-2">
                <span class="label-text text-[11px] opacity-70">{{ t("config.tools.toolEnabled") }}</span>
                <input
                  v-model="tool.enabled"
                  type="checkbox"
                  class="toggle toggle-xs"
                  :disabled="toolSwitchDisabled(tool.id)"
                  @change="$emit('toolSwitchChanged')"
                />
              </label>
              <button v-if="tool.id === 'memory-save'" class="btn btn-xs btn-ghost bg-base-100" @click="$emit('openMemoryViewer')">{{ t("config.tools.viewMemory") }}</button>
            </div>
          </div>
          <div v-if="tool.id === 'desktop-screenshot'" class="mt-2 pl-3">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[11px] opacity-70">{{ t("config.tools.desktopScreenshotDesc") }}</div>
              <button class="btn btn-xs btn-primary" :disabled="screenshotRunning || !toolApiConfig?.enableImage" @click="runDesktopScreenshot">
                {{ t("config.tools.runOnce") }}
              </button>
            </div>
            <div v-if="screenshotResult" class="mt-2 text-[11px] opacity-80 break-all">{{ screenshotResult }}</div>
          </div>
          <div v-if="tool.id === 'desktop-wait'" class="mt-2 pl-3">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[11px] opacity-70">{{ t("config.tools.desktopWaitDesc") }}</div>
              <div class="flex items-center gap-2">
                <input v-model.number="waitMs" type="number" min="1" max="120000" step="100" class="input input-bordered input-xs w-24" />
                <button class="btn btn-xs btn-primary" :disabled="waitRunning || !toolApiConfig?.enableImage" @click="runDesktopWait">
                  {{ t("config.tools.runOnce") }}
                </button>
              </div>
            </div>
            <div v-if="waitResult" class="mt-2 text-[11px] opacity-80 break-all">{{ waitResult }}</div>
          </div>
        </div>
      </div>
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
import { nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ToolLoadStatus } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";

const props = defineProps<{
  config: AppConfig;
  toolApiConfig: ApiConfigItem | null;
  toolStatuses: ToolLoadStatus[];
}>();

defineEmits<{
  (e: "openMemoryViewer"): void;
  (e: "toolSwitchChanged"): void;
}>();

const { t } = useI18n();
const screenshotRunning = ref(false);
const waitRunning = ref(false);
const screenshotResult = ref("");
const waitResult = ref("");
const waitMs = ref(800);
const screenshotPreviewDataUrl = ref("");
const screenshotDialogRef = ref<HTMLDialogElement | null>(null);

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? "unknown";
}

function statusDotClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "loaded") return "bg-success";
  if (status === "failed" || status === "timeout") return "bg-error";
  if (status === "disabled") return "bg-base-content/30";
  return "bg-base-content/20";
}

function toolDescription(id: string): string {
  if (id === "fetch") return t("config.tools.descFetch");
  if (id === "bing-search") return t("config.tools.descBingSearch");
  if (id === "memory-save") return t("config.tools.descMemorySave");
  if (id === "desktop-screenshot") return t("config.tools.descDesktopScreenshot");
  if (id === "desktop-wait") return t("config.tools.descDesktopWait");
  return t("config.tools.descGeneric");
}

function isImageBoundTool(id: string): boolean {
  return id === "desktop-screenshot" || id === "desktop-wait";
}

function toolSwitchDisabled(id: string): boolean {
  if (!isImageBoundTool(id)) return false;
  return !props.toolApiConfig?.enableImage;
}

function isToolRunning(id: string): boolean {
  if (id === "desktop-screenshot") return screenshotRunning.value;
  if (id === "desktop-wait") return waitRunning.value;
  return false;
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
