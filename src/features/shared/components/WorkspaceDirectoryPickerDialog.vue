<template>
  <Teleport to="body">
    <dialog
      ref="dialogRef"
      class="modal z-[1100]"
      :class="{ 'modal-open': open }"
      :open="open"
      @close="onDialogClose"
      @cancel.prevent="onDialogClose"
      @keydown.esc.prevent="onDialogClose"
    >
    <div
      class="modal-box flex h-[92vh] max-h-[92vh] w-[92vw] max-w-[92vw] flex-col overflow-hidden p-0"
    >
      <div class="border-b border-base-300 px-4 py-3 flex items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <div class="text-sm font-semibold">{{ displayTitle }}</div>
          <div v-if="displayHint" class="mt-1 text-xs opacity-70">{{ displayHint }}</div>
        </div>
        <button
          v-if="isDesktop"
          type="button"
          class="btn btn-xs btn-ghost shrink-0"
          @click="onUseSystemPicker"
        >
          {{ t('chat.directoryPicker.useSystemPicker') }}
        </button>
      </div>

      <div class="flex min-h-0 flex-1 flex-col gap-3 px-4 py-3">
        <div class="flex flex-wrap items-center gap-3">
          <label class="flex cursor-pointer items-center gap-1.5 text-xs">
            <input v-model="filterHidden" type="checkbox" class="checkbox checkbox-primary checkbox-xs" />
            <span>{{ t('chat.directoryPicker.filterHidden') }}</span>
          </label>
          <label class="flex cursor-pointer items-center gap-1.5 text-xs">
            <input v-model="filterGit" type="checkbox" class="checkbox checkbox-primary checkbox-xs" />
            <span>{{ t('chat.directoryPicker.filterGit') }}</span>
          </label>
          <span class="ml-auto text-xs opacity-60">{{ t('chat.directoryPicker.itemsCount', { count: filteredDirectories.length }) }}</span>
          <div class="flex items-center gap-0.5 rounded-box border border-base-300 p-0.5">
            <button
              type="button"
              class="btn btn-xs btn-square"
              :class="viewMode === 'list' ? 'btn-ghost bg-base-300' : 'btn-ghost'"
              :title="t('chat.directoryPicker.viewList')"
              :aria-label="t('chat.directoryPicker.viewList')"
              :aria-pressed="viewMode === 'list' ? 'true' : 'false'"
              @click="viewMode = 'list'"
            >
              <List class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="btn btn-xs btn-square"
              :class="viewMode === 'grid' ? 'btn-ghost bg-base-300' : 'btn-ghost'"
              :title="t('chat.directoryPicker.viewGrid')"
              :aria-label="t('chat.directoryPicker.viewGrid')"
              :aria-pressed="viewMode === 'grid' ? 'true' : 'false'"
              @click="viewMode = 'grid'"
            >
              <LayoutGrid class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>

        <div class="flex items-center gap-1">
          <button
            type="button"
            class="btn btn-xs btn-ghost btn-square"
            :disabled="!canGoBack || loading"
            :title="t('chat.directoryPicker.back')"
            :aria-label="t('chat.directoryPicker.back')"
            @click="goBack"
          >
            <ArrowLeft class="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            class="btn btn-xs btn-ghost btn-square"
            :disabled="!canGoForward || loading"
            :title="t('chat.directoryPicker.forward')"
            :aria-label="t('chat.directoryPicker.forward')"
            @click="goForward"
          >
            <ArrowRight class="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            class="btn btn-xs btn-ghost btn-square"
            :disabled="loading || !parentPath"
            :title="t('chat.directoryPicker.up')"
            :aria-label="t('chat.directoryPicker.up')"
            @click="onUp"
          >
            <ArrowUp class="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            class="btn btn-xs btn-ghost btn-square"
            :disabled="loading"
            :title="t('chat.directoryPicker.refresh')"
            :aria-label="t('chat.directoryPicker.refresh')"
            @click="onRefresh"
          >
            <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
          </button>
          <BreadcrumbAddressBar
            class="min-w-0 flex-1"
            :path="currentPath"
            :switchable="props.switchable"
            @navigate="onBreadcrumb"
            @submit="onAddressSubmit"
          />
        </div>

        <div class="flex min-h-0 flex-1 rounded-box border border-base-300 bg-base-200/30">
          <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full" orientation="vertical">
            <div class="py-1">
              <div v-if="loading" class="flex items-center gap-2 px-3 py-3 text-sm opacity-65">
                <span class="loading loading-spinner loading-xs"></span>
                {{ t('chat.directoryPicker.loading') }}
              </div>
              <div v-else-if="errorText" class="px-3 py-3 text-sm text-error">{{ errorText }}</div>
              <div v-else-if="filteredDirectories.length === 0" class="px-3 py-3 text-sm opacity-55">
                {{ currentPath ? t('chat.directoryPicker.emptyNoSubDirs') : t('chat.directoryPicker.emptyNoDrives') }}
              </div>
              <template v-else>
                <template v-if="viewMode === 'list'">
                  <button
                    v-for="item in filteredDirectories"
                    :key="item.path"
                    type="button"
                    class="flex min-h-8 w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-base-300/60"
                    :title="item.path"
                    @click="onEntryClick(item.path)"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class="h-4 w-4 shrink-0" fill="#facc15" aria-hidden="true"><path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z" /></svg>
                    <span class="min-w-0 flex-1 truncate">{{ item.name }}</span>
                  </button>
                </template>
                <div v-else class="flex flex-wrap content-start gap-2 p-2">
                  <button
                    v-for="item in filteredDirectories"
                    :key="item.path"
                    type="button"
                    class="flex w-[96px] flex-col items-center gap-1 rounded-box border border-transparent px-2 py-2.5 text-center hover:border-base-300 hover:bg-base-300/40"
                    :title="item.path"
                    @click="onEntryClick(item.path)"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class="h-7 w-7 shrink-0" fill="#facc15" aria-hidden="true"><path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z" /></svg>
                    <span class="line-clamp-2 w-full break-all text-xs leading-tight">{{ item.name }}</span>
                  </button>
                </div>
              </template>
            </div>
          </OverlayScrollArea>
        </div>

        <div class="text-xs opacity-60">
          {{ t('chat.directoryPicker.selectedLabel') }} <span class="font-mono break-all">{{ currentPath || t('chat.directoryPicker.selectedEmpty') }}</span>
        </div>
      </div>

      <div class="flex shrink-0 items-center justify-end gap-2 border-t border-base-300 px-4 py-3">
        <button class="btn btn-sm btn-ghost" type="button" @click="onDialogClose">{{ t('chat.directoryPicker.cancel') }}</button>
        <button
          class="btn btn-sm btn-primary"
          type="button"
          :disabled="!canConfirm"
          @click="onConfirm"
        >
          {{ displayConfirmLabel }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeft, ArrowRight, ArrowUp, LayoutGrid, List, RefreshCw } from "@lucide/vue";
import OverlayScrollArea from "./OverlayScrollArea.vue";
import BreadcrumbAddressBar from "./BreadcrumbAddressBar.vue";
import { invokeTauri, isDesktopTauriHost, openTransportFileDialog } from "../../../services/tauri-api";

type DirectoryItem = {
  path: string;
  name: string;
};

const props = withDefaults(
  defineProps<{
    open: boolean;
    initialPath?: string;
    title?: string;
    hint?: string;
    confirmLabel?: string;
    switchable?: boolean;
  }>(),
  {
    initialPath: "",
    title: "",
    hint: "",
    confirmLabel: "",
    switchable: true,
  },
);

const emit = defineEmits<{
  (e: "close"): void;
  (e: "select", path: string): void;
}>();

const { t } = useI18n();

const dialogRef = ref<HTMLDialogElement | null>(null);

const isDesktop = isDesktopTauriHost();

const displayTitle = computed(() => {
  const raw = String(props.title || "").trim();
  return raw || t('chat.directoryPicker.title');
});
const displayHint = computed(() => {
  const raw = String(props.hint || "").trim();
  return raw || t('chat.directoryPicker.hint');
});
const displayConfirmLabel = computed(() => {
  const raw = String(props.confirmLabel || "").trim();
  return raw || t('chat.directoryPicker.confirm');
});

const currentPath = ref("");
const directories = ref<DirectoryItem[]>([]);
const loading = ref(false);
const errorText = ref("");
const filterHidden = ref(true);
const filterGit = ref(true);
const viewMode = ref<'list' | 'grid'>('list');
const historyStack = ref<string[]>([]);
const historyIndex = ref(-1);

let seq = 0;

const canGoBack = computed(() => historyIndex.value > 0);
const canGoForward = computed(() => historyIndex.value >= 0 && historyIndex.value < historyStack.value.length - 1);

const canConfirm = computed(() => {
  const p = String(currentPath.value || "").trim();
  if (!p) return false;
  return true;
});

const filteredDirectories = computed(() => {
  const list = directories.value || [];
  return list.filter((item) => {
    const name = String(item.name || "").trim();
    if (!name) return false;
    if (filterGit.value && name === ".git") return false;
    if (filterHidden.value && name.startsWith(".")) return false;
    return true;
  });
});

const parentPath = computed(() => {
  const normalized = String(currentPath.value || "").trim().replace(/[\\/]+$/, "");
  if (!normalized) return "";
  if (/^[A-Za-z]:\/?$/.test(normalized) || normalized === "/") return "";
  const lastSlash = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  if (lastSlash < 0) return "";
  if (lastSlash === 0) return normalized.slice(0, 1);
  const candidate = normalized.slice(0, lastSlash);
  if (/^[A-Za-z]:$/.test(candidate)) return `${candidate}/`;
  if (candidate === "") return "/";
  if (/^[A-Za-z]:\/$/.test(normalized.slice(0, lastSlash + 1))) {
    return normalized.slice(0, lastSlash + 1);
  }
  return candidate;
});

function pushHistory(path: string) {
  const normalized = String(path || "").trim();
  const current = historyIndex.value >= 0 ? String(historyStack.value[historyIndex.value] || "").trim() : "";
  if (normalized === current && historyStack.value.length > 0) return;
  const next = historyStack.value.slice(0, historyIndex.value + 1);
  next.push(normalized);
  historyStack.value = next;
  historyIndex.value = next.length - 1;
}

function onDialogClose() {
  if (loading.value) return;
  emit("close");
}

async function loadDirectory(target: string, options: { pushHistory?: boolean } = {}) {
  const nextSeq = ++seq;
  const raw = String(target || "").trim();
  loading.value = true;
  errorText.value = "";
  try {
    const payload = await invokeTauri<{ path: string; name: string; directories: DirectoryItem[]; entries?: DirectoryItem[] }>(
      "workspace.directory.list",
      { path: raw },
    );
    if (nextSeq !== seq) return;
    const resolvedPath = String((payload as unknown as { path: string })?.path || raw || "").trim();
    const rawEntries = Array.isArray((payload as unknown as { directories: unknown })?.directories)
      ? (payload as unknown as { directories: DirectoryItem[] }).directories
      : Array.isArray((payload as unknown as { entries: unknown })?.entries)
        ? (payload as unknown as { entries: DirectoryItem[] }).entries.filter((e) => (e as DirectoryItem).path)
        : [];
    const dirs = rawEntries
      .map((e) => ({
        path: String(e.path || "").trim().replace(/\\/g, "/"),
        name: String(e.name || "").trim(),
      }))
      .filter((e) => e.path && e.name);
    directories.value = dirs;
    const nextPath = resolvedPath ? resolvedPath.replace(/\\/g, "/") : "";
    currentPath.value = nextPath;
    if (options.pushHistory !== false) {
      pushHistory(nextPath);
    }
  } catch (error) {
    if (nextSeq !== seq) return;
    const message = error instanceof Error ? error.message : String(error);
    errorText.value = message || t('chat.directoryPicker.loading');
    directories.value = [];
  } finally {
    if (nextSeq === seq) loading.value = false;
  }
}

function onEntryClick(path: string) {
  const normalized = String(path || "").trim();
  if (!normalized) return;
  void loadDirectory(normalized, { pushHistory: true });
}

function onBreadcrumb(path: string) {
  const normalized = String(path || "").trim();
  if (!normalized) return;
  void loadDirectory(normalized, { pushHistory: true });
}

function onUp() {
  const p = parentPath.value;
  if (!p) return;
  void loadDirectory(p, { pushHistory: true });
}

function onRefresh() {
  void loadDirectory(currentPath.value || "", { pushHistory: false });
}

function onAddressSubmit(path: string) {
  const normalized = String(path || "").trim();
  void loadDirectory(normalized, { pushHistory: true });
}

function goBack() {
  if (!canGoBack.value || loading.value) return;
  const nextIndex = historyIndex.value - 1;
  const target = String(historyStack.value[nextIndex] || "").trim();
  historyIndex.value = nextIndex;
  void loadDirectory(target, { pushHistory: false });
}

function goForward() {
  if (!canGoForward.value || loading.value) return;
  const nextIndex = historyIndex.value + 1;
  const target = String(historyStack.value[nextIndex] || "").trim();
  historyIndex.value = nextIndex;
  void loadDirectory(target, { pushHistory: false });
}

function onConfirm() {
  const p = String(currentPath.value || "").trim();
  if (!p) return;
  emit("select", p);
}

async function onUseSystemPicker() {
  if (loading.value) return;
  try {
    const picked = await openTransportFileDialog({
      directory: true,
      title: String(displayTitle.value || t('chat.directoryPicker.title')),
      defaultPath: String(currentPath.value || props.initialPath || "").trim() || undefined,
    });
    const raw = Array.isArray(picked) ? String(picked[0] || "").trim() : String(picked || "").trim();
    if (!raw) return;
    const normalized = raw.replace(/\\/g, "/");
    emit("select", normalized);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    errorText.value = message || t('chat.directoryPicker.systemPickerFailed');
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      const init = String(props.initialPath || "").trim();
      currentPath.value = init;
      directories.value = [];
      errorText.value = "";
      historyStack.value = [];
      historyIndex.value = -1;
      void loadDirectory(init, { pushHistory: true });
    } else {
      seq += 1;
    }
  },
  { immediate: true },
);

watch(
  () => props.initialPath,
  (next) => {
    if (props.open) {
      const normalized = String(next || "").trim();
      if (normalized !== String(currentPath.value || "").trim()) {
        void loadDirectory(normalized, { pushHistory: true });
      }
    }
  },
);
</script>
