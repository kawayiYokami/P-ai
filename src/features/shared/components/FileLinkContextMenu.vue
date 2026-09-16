<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  getTransportCapabilities,
  openTransportExternalUrl,
  openTransportFileInVscode,
  revealTransportLocalFile,
  saveTransportLocalFileAs,
} from "../../../services/tauri-api";
import type { FileLinkMenuState } from "../composables/use-file-link-context-menu";

const props = withDefaults(
  defineProps<{
    state: FileLinkMenuState | null;
    /** 当前窗口是否提供「在侧边打开」；没有阅读器时置 false。 */
    canOpenInSidebar?: boolean;
    /** 当前窗口是否提供需要本机文件的动作（定位/VS Code/另存为/在侧边打开）；关掉只留复制。 */
    canUseLocalFileActions?: boolean;
  }>(),
  { canOpenInSidebar: true, canUseLocalFileActions: true },
);

const emit = defineEmits<{
  (e: "openInSidebar", payload: { path: string; line?: number }): void;
  (e: "close"): void;
}>();

const { t } = useI18n();
// 定位与「在 VS Code 中打开」依赖本机文件系统，仅桌面宿主提供。
const localFileSystem = getTransportCapabilities().localFileSystem;
const error = ref("");

watch(
  () => props.state,
  () => {
    error.value = "";
  },
);

function close() {
  emit("close");
}

async function run(action: () => Promise<boolean>) {
  try {
    await action();
    close();
  } catch (err) {
    error.value = t("chat.fileLinkMenu.actionFailed", { err: String(err) });
  }
}

function fileState() {
  const state = props.state;
  return state && state.kind === "file" ? state : null;
}

function urlState() {
  const state = props.state;
  return state && state.kind === "url" ? state : null;
}

function copyText(text: string) {
  navigator.clipboard
    .writeText(text)
    .then(() => close())
    .catch((err) => {
      error.value = t("chat.fileLinkMenu.actionFailed", { err: String(err) });
    });
}

function openInSidebar() {
  const state = fileState();
  if (!state) return;
  emit("openInSidebar", { path: state.path, line: state.line });
  close();
}

function revealInFolder() {
  const state = fileState();
  if (!state) return;
  void run(() => revealTransportLocalFile(state.path));
}

function openInVscode() {
  const state = fileState();
  if (!state) return;
  void run(() => openTransportFileInVscode(state.path));
}

function saveAs() {
  const state = fileState();
  if (!state) return;
  void run(() => saveTransportLocalFileAs(state.path));
}

function copyPath() {
  const state = fileState();
  if (state) copyText(state.raw);
}

function openInBrowser() {
  const state = urlState();
  if (!state) return;
  void run(() => openTransportExternalUrl(state.url));
}

function copyLink() {
  const state = urlState();
  if (state) copyText(state.url);
}

function handleGlobalPointerDown(event: PointerEvent) {
  const target = event.target as Element | null;
  if (target?.closest?.('[data-file-link-menu="true"]')) return;
  close();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") close();
}

onMounted(() => {
  window.addEventListener("pointerdown", handleGlobalPointerDown, true);
  window.addEventListener("keydown", handleKeydown, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", handleGlobalPointerDown, true);
  window.removeEventListener("keydown", handleKeydown, true);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="state"
      data-file-link-menu="true"
      class="fixed z-1300 w-52 rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
      :style="{ left: `${state.x}px`, top: `${state.y}px` }"
      @pointerdown.stop
      @contextmenu.prevent.stop
    >
      <template v-if="state.kind === 'file'">
        <button
          v-if="canUseLocalFileActions && canOpenInSidebar"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="openInSidebar"
        >
          {{ t("chat.fileLinkMenu.openInSidebar") }}
        </button>
        <button
          v-if="canUseLocalFileActions && localFileSystem"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="openInVscode"
        >
          {{ t("chat.fileLinkMenu.openInVscode") }}
        </button>
        <button
          v-if="canUseLocalFileActions && localFileSystem"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="revealInFolder"
        >
          {{ t("chat.fileLinkMenu.revealInFolder") }}
        </button>
        <button
          v-if="canUseLocalFileActions"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="saveAs"
        >
          {{ t("chat.fileLinkMenu.saveAs") }}
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="copyPath"
        >
          {{ t("chat.fileLinkMenu.copyPath") }}
        </button>
      </template>
      <template v-else>
        <button
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="openInBrowser"
        >
          {{ t("chat.fileLinkMenu.openInBrowser") }}
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="copyLink"
        >
          {{ t("chat.fileLinkMenu.copyLink") }}
        </button>
      </template>
      <div v-if="error" class="px-2 py-1 text-caption leading-4 text-error">{{ error }}</div>
    </div>
  </Teleport>
</template>
