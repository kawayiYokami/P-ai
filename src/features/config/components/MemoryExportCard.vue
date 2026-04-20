<template>
  <div class="contents">
    <button
      class="btn btn-sm btn-ghost"
      :disabled="loading"
      :title="t('config.memory.exportCardTitle')"
      @click="openDialog"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="17 8 12 3 7 8"/>
        <line x1="12" x2="12" y1="3" y2="15"/>
      </svg>
      <span>{{ t("config.memory.exportShort") }}</span>
    </button>

    <dialog ref="dialogRef" class="modal">
      <div class="modal-box max-w-3xl">
        <h3 class="text-lg font-semibold">{{ t("config.memory.exportDialogTitle") }}</h3>
        <div class="mt-2 text-sm opacity-70">
          {{ t("config.memory.exportDialogHint") }}
        </div>

        <div v-if="message" class="mt-4 text-sm break-all rounded-box bg-base-200/50 px-2 py-1.5">
          {{ message }}
        </div>

        <div class="mt-4 rounded-box border border-base-300 bg-base-200/40 p-4 space-y-3">
          <div class="flex items-center justify-between gap-3">
            <div class="text-sm font-medium">
              {{ t("config.memory.exportDetected", { count: preview?.totalCount ?? 0 }) }}
            </div>
            <button class="btn btn-sm btn-ghost" :disabled="loading" @click="loadPreview">
              {{ t("config.memory.refreshExportScopes") }}
            </button>
          </div>

          <div v-if="!preview?.scopes.length" class="rounded-box border border-base-300 bg-base-100 p-4 text-sm opacity-60">
            {{ t("config.memory.exportNoScopes") }}
          </div>
          <div v-else class="space-y-2">
            <label
              v-for="scopeItem in preview?.scopes || []"
              :key="scopeItem.scope"
              class="flex items-center justify-between gap-3 rounded-box border border-base-300 bg-base-100 px-3 py-3 cursor-pointer"
            >
              <div class="min-w-0">
                <div class="text-sm font-semibold break-all">{{ scopeItem.scope }}</div>
                <div class="text-xs opacity-60">
                  {{ t("config.memory.exportScopeCount", { count: scopeItem.count }) }}
                </div>
              </div>
              <input
                v-model="selectedScopes"
                class="checkbox checkbox-sm"
                type="checkbox"
                :value="scopeItem.scope"
              />
            </label>
          </div>

          <div class="text-xs opacity-60">{{ t("config.memory.exportScopeHint") }}</div>
        </div>

        <div class="modal-action">
          <button class="btn" :disabled="loading" @click="closeDialog">
            {{ t("config.memory.exportCancel") }}
          </button>
          <button class="btn btn-primary" :disabled="loading || !selectedScopes.length" @click="confirmExport">
            {{ t("config.memory.exportConfirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeDialog">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { save } from "@tauri-apps/plugin-dialog";
import { invokeTauri } from "../../../services/tauri-api";

type ExportPreviewScopeItem = {
  scope: string;
  count: number;
};

type ExportPreviewResult = {
  totalCount: number;
  scopes: ExportPreviewScopeItem[];
};

type ExportResult = {
  path: string;
  count: number;
};

const { t } = useI18n();
const emit = defineEmits<{
  (e: "exported", payload: ExportResult): void;
}>();
const dialogRef = ref<HTMLDialogElement | null>(null);
const loading = ref(false);
const message = ref("");
const preview = ref<ExportPreviewResult | null>(null);
const selectedScopes = ref<string[]>([]);

async function withLoading<T>(fn: () => Promise<T>): Promise<T | null> {
  loading.value = true;
  try {
    return await fn();
  } catch (err) {
    message.value = `${t("config.memory.operationFailed")}: ${String(err)}`;
    return null;
  } finally {
    loading.value = false;
  }
}

function resetDialogState() {
  message.value = "";
  preview.value = null;
  selectedScopes.value = [];
}

async function loadPreview() {
  const result = await withLoading(() =>
    invokeTauri<ExportPreviewResult>("preview_export_memories"),
  );
  if (!result) return;
  preview.value = result;
  selectedScopes.value = result.scopes.map((item) => item.scope);
}

async function openDialog() {
  resetDialogState();
  dialogRef.value?.showModal();
  await loadPreview();
}

function closeDialog() {
  dialogRef.value?.close();
  resetDialogState();
}

async function confirmExport() {
  if (!selectedScopes.value.length) return;
  const path = await save({
    defaultPath: "memory_backup.json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path || Array.isArray(path)) return;
  const result = await withLoading(() =>
    invokeTauri<ExportResult>("export_memories_to_path", {
      input: {
        path: String(path),
        scopes: selectedScopes.value,
      },
    }),
  );
  if (!result) return;
  closeDialog();
  emit("exported", result);
}
</script>
