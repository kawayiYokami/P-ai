<template>
  <div class="space-y-5">
    <div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
      <div class="flex min-w-0 items-start gap-3">
        <div class="flex shrink-0 items-center justify-center rounded-box bg-primary/15 p-3 text-primary">
          <AppIcon name="hard-drive" class="h-6 w-6" />
        </div>
        <div class="min-w-0">
          <div class="text-sm font-semibold">{{ t("config.storage.pageTitle") }}</div>
        </div>
      </div>
      <button
        class="btn btn-sm bg-base-100 shrink-0"
        :disabled="storageLoading || !!cleanupBusyKind"
        @click="refreshStorageOverview"
      >
        <AppIcon name="refresh-cw" class="h-4 w-4" :class="{ 'animate-spin': storageLoading }" />
        <span>{{ t("config.storage.refreshAction") }}</span>
      </button>
    </div>

    <ConfigCard :title="t('config.storage.usageTitle')">
      <template #actions>
        <div v-if="storageOverview" class="text-xs opacity-60 break-all md:max-w-sm md:text-right">
          {{ t("config.storage.rootPath", { path: storageOverview.rootPath }) }}
        </div>
      </template>
      <div class="grid gap-4 py-3">

        <div v-if="storageOverview" class="grid gap-4 rounded-box bg-base-200/70 p-3 lg:grid-cols-[auto,1fr]">
          <div class="flex items-center gap-4">
            <div
              class="relative h-28 w-28 shrink-0 rounded-full border border-base-300 shadow-inner"
              :style="storagePieChartStyle"
              :title="t('config.storage.totalSize')"
            >
              <div class="absolute inset-6 flex items-center justify-center rounded-full bg-base-100 text-center text-xs font-semibold shadow-sm">
                {{ formatBytes(storageOverview.totalBytes) }}
              </div>
            </div>
            <div class="grid gap-1 text-sm">
              <div class="font-medium">{{ t("config.storage.totalSize") }}：{{ formatBytes(storageOverview.totalBytes) }}</div>
              <div class="opacity-70">{{ t("config.storage.reclaimableSize") }}：{{ formatBytes(storageOverview.reclaimableBytes) }}</div>
              <div class="opacity-70">{{ t("config.storage.categoryCount") }}：{{ visibleStorageItems.length }}</div>
            </div>
          </div>
          <div class="grid content-center gap-2 sm:grid-cols-2 xl:grid-cols-3">
            <div
              v-for="item in storagePieLegendItems"
              :key="item.id"
              class="flex min-w-0 items-center gap-2 text-xs"
            >
              <span class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{ backgroundColor: storagePieColor(item.index) }"></span>
              <span class="min-w-0 flex-1 truncate">{{ storageCategoryLabel(item.id) }}</span>
              <span class="shrink-0 opacity-70">{{ formatBytes(item.bytes) }}</span>
            </div>
          </div>
        </div>

        <div v-if="storageLoading" class="rounded-box border border-base-300 bg-base-100 p-4">
          <div class="mb-3 text-sm opacity-70">{{ t("config.storage.loading") }}</div>
          <progress class="progress progress-primary w-full"></progress>
        </div>

        <div v-else-if="storageOverview" class="overflow-x-auto rounded-box border border-base-300">
          <table class="table table-sm">
            <thead>
              <tr>
                <th>{{ t("config.storage.tableCategory") }}</th>
                <th class="text-right">{{ t("config.storage.tableSize") }}</th>
                <th class="text-right">{{ t("config.storage.tableCleanable") }}</th>
                <th class="w-36 text-right">{{ t("config.storage.tableAction") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in visibleStorageItems" :key="item.id">
                <td class="min-w-44 font-medium">{{ storageCategoryLabel(item.id) }}</td>
                <td class="whitespace-nowrap text-right">{{ formatBytes(item.bytes) }}</td>
                <td class="whitespace-nowrap text-right" :class="item.cleanableFileCount > 0 ? 'text-error' : 'opacity-50'">
                  {{ item.cleanableFileCount > 0 ? formatBytes(item.cleanableBytes) : "-" }}
                </td>
                <td class="text-right">
                  <div class="flex justify-end gap-1">
                    <button
                      v-if="localFileSystemAvailable"
                      class="btn btn-primary btn-xs"
                      :disabled="storageLoading || openingItemId === item.id"
                      :title="item.targetPath"
                      @click="handleOpenStorageItem(item)"
                    >
                      <AppIcon name="folder-open" class="h-3.5 w-3.5" :class="{ 'animate-pulse': openingItemId === item.id }" />
                      <span>{{ t("config.storage.openAction") }}</span>
                    </button>
                    <button
                      v-if="item.cleanupKind"
                      class="btn btn-error btn-xs"
                      :disabled="storageLoading || !!cleanupBusyKind || item.cleanableFileCount === 0"
                      @click="handleCleanupStorageItem(item)"
                    >
                      <AppIcon name="trash" class="h-3.5 w-3.5" :class="{ 'animate-pulse': cleanupBusyKind === item.cleanupKind }" />
                      <span>{{ t("config.storage.cleanupAction") }}</span>
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div v-else class="rounded-box border border-base-300 bg-base-100 p-4 text-sm opacity-70">
          {{ t("config.storage.empty") }}
        </div>

        <div v-if="storageMessage" class="alert" :class="storageMessageIsError ? 'alert-error' : 'alert-success'">
          <span>{{ storageMessage }}</span>
        </div>
      </div>
    </ConfigCard>

    <div class="flex items-center gap-3">
      <div class="flex items-center justify-center rounded-box bg-info/15 p-2 text-info">
        <AppIcon name="download" class="h-5 w-5" />
      </div>
      <div>
        <div class="text-sm font-semibold">{{ t("config.migration.pageTitle") }}</div>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-5">
      <ConfigCard :title="t('config.migration.exportTitle')">
        <div class="flex flex-col gap-5 py-3">
          <div class="flex items-start gap-4">
            <div class="flex items-center justify-center rounded-box bg-info/15 p-3 text-info">
              <AppIcon name="download" class="h-6 w-6" />
            </div>
          </div>

          <div class="rounded-box border border-info/25 bg-info/10 px-4 py-3 text-sm text-base-content">
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center rounded-full bg-info/20 px-2 py-1 text-sm font-bold text-info">i</div>
              <span>{{ t("config.migration.exportNotice") }}</span>
            </div>
          </div>

          <label class="grid w-full gap-2">
            <span class="text-sm font-medium">{{ t("config.migration.password") }}</span>
            <label class="input input-bordered flex w-full items-center gap-3">
              <input
                v-model.trim="exportPassword"
                :type="showExportPassword ? 'text' : 'password'"
                class="min-w-0 grow"
                :placeholder="t('config.migration.passwordPlaceholder')"
              />
              <button type="button" class="btn btn-ghost btn-sm btn-circle" @click="showExportPassword = !showExportPassword">
                <AppIcon name="eye-off" v-if="showExportPassword" class="h-5 w-5 opacity-60" />
                <AppIcon name="eye" v-else class="h-5 w-5 opacity-60" />
              </button>
            </label>
          </label>

          <button
            class="btn btn-primary w-full"
            :disabled="busy || !canExport"
            @click="handleExport"
          >
            {{ t("config.migration.exportAction") }}
          </button>

          <div v-if="exportMessage" class="alert" :class="exportMessageIsError ? 'alert-error' : 'alert-success'">
            <span>{{ exportMessage }}</span>
          </div>
        </div>
      </ConfigCard>

      <ConfigCard :title="t('config.migration.importTitle')">
        <div class="flex flex-col gap-5 py-3">
          <div class="flex items-start gap-4">
            <div class="flex items-center justify-center rounded-box bg-success/15 p-3 text-success">
              <AppIcon name="upload" class="h-6 w-6" />
            </div>
          </div>

          <div class="rounded-box border border-info/25 bg-info/10 px-4 py-3 text-sm text-base-content">
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center rounded-full bg-info/20 px-2 py-1 text-sm font-bold text-info">i</div>
              <span>{{ t("config.migration.importNotice") }}</span>
            </div>
          </div>

          <button
            type="button"
            class="flex w-full flex-col items-center justify-center rounded-box border-2 border-dashed border-base-300 bg-base-100 px-6 py-8 text-center transition hover:border-primary/50 hover:bg-primary/5 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="busy"
            @click="handleSelectImportPackage"
          >
            <AppIcon name="upload" class="mb-4 h-12 w-12 text-base-content/35" />
            <div class="text-base font-semibold">{{ t("config.migration.importUploadTitle") }}</div>
            <div class="mt-2 whitespace-pre-line text-sm opacity-70">{{ t("config.migration.importUploadHint") }}</div>
          </button>

          <label v-if="needImportPassword" class="grid w-full gap-2">
            <span class="text-sm font-medium">{{ t("config.migration.decryptPassword") }}</span>
            <label class="input input-bordered flex w-full items-center gap-3">
              <input
                v-model.trim="importPassword"
                :type="showImportPassword ? 'text' : 'password'"
                class="min-w-0 grow"
                :placeholder="t('config.migration.decryptPasswordPlaceholder')"
              />
              <button type="button" class="btn btn-ghost btn-sm btn-circle" @click="showImportPassword = !showImportPassword">
                <AppIcon name="eye-off" v-if="showImportPassword" class="h-5 w-5 opacity-60" />
                <AppIcon name="eye" v-else class="h-5 w-5 opacity-60" />
              </button>
            </label>
          </label>

          <button
            v-if="needImportPassword"
            class="btn btn-primary w-full"
            :disabled="busy || importPassword.length === 0"
            @click="handlePreviewImport"
          >
            {{ t("config.migration.previewWithPasswordAction") }}
          </button>

          <div v-if="previewResult" class="rounded-box border border-base-300 bg-base-100 p-5">
            <div class="mb-4 text-base font-semibold">{{ t("config.migration.previewTitle") }}</div>
            <div class="grid grid-cols-1 gap-3 text-sm md:grid-cols-2">
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.packageVersion", { version: previewResult.packageVersion }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.memoryAdded", { count: previewResult.memoryAddedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.memoryMerged", { count: previewResult.memoryMergedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.providerAdded", { count: previewResult.providerAddedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.providerUpdated", { count: previewResult.providerUpdatedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.apiConfigAdded", { count: previewResult.apiConfigAddedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.apiConfigUpdated", { count: previewResult.apiConfigUpdatedCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.oauthFileCount", { count: previewResult.oauthFileCount }) }}</div>
              <div class="rounded-box bg-base-200/60 p-4">{{ t("config.migration.avatarFileCount", { count: previewResult.avatarFileCount }) }}</div>
            </div>

            <div class="alert alert-warning mt-4">
              <span>{{ t("config.migration.importWarning") }}</span>
            </div>

            <div class="mt-4 flex justify-end">
              <button class="btn btn-primary" :disabled="busy" @click="handleApplyImport">
                {{ t("config.migration.applyAction") }}
              </button>
            </div>
          </div>

          <div v-if="importMessage" class="alert" :class="importMessageIsError ? 'alert-error' : 'alert-success'">
            <span>{{ importMessage }}</span>
          </div>
        </div>
      </ConfigCard>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConfigCard from "../../components/ConfigCard.vue";
import {
  applyTransportConfigMigrationPackage,
  exportTransportConfigMigrationPackage,
  getTransportCapabilities,
  invokeTauri,
  openTransportStorageUsageItemDirectory,
  pickTransportConfigMigrationPackage,
  previewTransportConfigMigrationPackage,
  type TransportConfigMigrationPackageSelection,
} from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";

type MigrationPreviewResult = {
  previewId: string;
  packageVersion: string;
  memoryAddedCount: number;
  memoryMergedCount: number;
  providerAddedCount: number;
  providerUpdatedCount: number;
  apiConfigAddedCount: number;
  apiConfigUpdatedCount: number;
  oauthFileCount: number;
  avatarFileCount: number;
};

type StorageUsageItem = {
  id: string;
  targetPath: string;
  bytes: number;
  fileCount: number;
  directoryCount: number;
  cleanableBytes: number;
  cleanableFileCount: number;
  cleanupKind?: string | null;
};

type StorageUsageOverview = {
  rootPath: string;
  totalBytes: number;
  reclaimableBytes: number;
  items: StorageUsageItem[];
};

type OverviewStatus = {
  computeState: "idle" | "running";
  freshness: "never" | "fresh" | "expired";
  generatedAt?: string | null;
  lastError?: string | null;
};

type OverviewSnapshot<T> = {
  status: OverviewStatus;
  data?: T | null;
};

type StorageCleanupResult = {
  deletedFileCount: number;
  skippedFileCount: number;
  freedBytes: number;
};

const { t, te } = useI18n();

const busy = ref(false);
const exportPassword = ref("");
const importPassword = ref("");
const previewResult = ref<MigrationPreviewResult | null>(null);
const exportMessage = ref("");
const exportMessageIsError = ref(false);
const importMessage = ref("");
const importMessageIsError = ref(false);
const showExportPassword = ref(false);
const showImportPassword = ref(false);
const needImportPassword = ref(false);
const storageOverview = ref<StorageUsageOverview | null>(null);
const storageStatus = ref<OverviewStatus | null>(null);
const storageMessage = ref("");
const storageMessageIsError = ref(false);
const cleanupBusyKind = ref("");
const openingItemId = ref("");
const selectedImportPackage = ref<TransportConfigMigrationPackageSelection | null>(null);
const PASSWORD_REQUIRED_CODE = "MIGRATION_PASSWORD_REQUIRED";
const localFileSystemAvailable = getTransportCapabilities().localFileSystem;
const storageLoading = computed(() => storageStatus.value?.computeState === "running");
let storagePollTimer: number | null = null;
let storageTabUnmounted = false;
const STORAGE_PIE_COLORS = [
  "#2563eb",
  "#16a34a",
  "#f59e0b",
  "#dc2626",
  "#0891b2",
  "#7c3aed",
  "#db2777",
  "#64748b",
  "#65a30d",
  "#ea580c",
];

const canExport = computed(() => exportPassword.value.length >= 6);

const visibleStorageItems = computed(() => {
  const items = storageOverview.value?.items || [];
  const visible = items.filter((item) => item.bytes > 0 || item.cleanableFileCount > 0 || !!item.cleanupKind);
  return visible.length > 0 ? visible : items;
});

const storagePieItems = computed(() => visibleStorageItems.value.filter((item) => item.bytes > 0));

const storagePieLegendItems = computed(() => storagePieItems.value.slice(0, 9).map((item, index) => ({
  ...item,
  index,
})));

const storagePieChartStyle = computed(() => {
  const totalBytes = storageOverview.value?.totalBytes || 0;
  const items = storagePieItems.value;
  if (totalBytes <= 0 || items.length === 0) {
    return { background: "var(--color-base-300)" };
  }
  let cursor = 0;
  const segments = items.map((item, index) => {
    const start = cursor;
    const end = Math.min(100, cursor + (item.bytes / totalBytes) * 100);
    cursor = end;
    return `${storagePieColor(index)} ${start.toFixed(3)}% ${end.toFixed(3)}%`;
  });
  if (cursor < 100) {
    segments.push(`var(--color-base-300) ${cursor.toFixed(3)}% 100%`);
  }
  return { background: `conic-gradient(${segments.join(", ")})` };
});

function setExportMessage(text: string, isError = false) {
  exportMessage.value = text;
  exportMessageIsError.value = isError;
}

function setImportMessage(text: string, isError = false) {
  importMessage.value = text;
  importMessageIsError.value = isError;
}

function setStorageMessage(text: string, isError = false) {
  storageMessage.value = text;
  storageMessageIsError.value = isError;
}

function formatBytes(value: number) {
  const bytes = Number.isFinite(value) && value > 0 ? value : 0;
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const fractionDigits = unitIndex === 0 || size >= 100 ? 0 : size >= 10 ? 1 : 2;
  return `${size.toFixed(fractionDigits)} ${units[unitIndex]}`;
}

function storageCategoryLabel(id: string) {
  const key = `config.storage.categories.${id}`;
  return te(key) ? t(key) : id;
}

function storagePieColor(index: number) {
  return STORAGE_PIE_COLORS[index % STORAGE_PIE_COLORS.length];
}

function stopStoragePolling() {
  if (storagePollTimer != null) {
    window.clearTimeout(storagePollTimer);
    storagePollTimer = null;
  }
}

function scheduleStoragePolling() {
  stopStoragePolling();
  if (storageTabUnmounted || storageStatus.value?.computeState !== "running") return;
  storagePollTimer = window.setTimeout(() => {
    void loadStorageOverview();
  }, 1000);
}

function applyStorageOverviewSnapshot(snapshot: OverviewSnapshot<StorageUsageOverview>) {
  storageStatus.value = snapshot.status;
  if (snapshot.data) {
    storageOverview.value = snapshot.data;
  }
  if (snapshot.status.lastError) {
    setStorageMessage(snapshot.status.lastError, true);
  }
  scheduleStoragePolling();
}

async function loadStorageOverview() {
  storageMessage.value = "";
  try {
    const snapshot = await invokeTauri<OverviewSnapshot<StorageUsageOverview>>("get_storage_usage_overview");
    if (!storageTabUnmounted) {
      applyStorageOverviewSnapshot(snapshot);
    }
  } catch (error) {
    setStorageMessage(toErrorMessage(error), true);
  }
}

async function refreshStorageOverview() {
  storageMessage.value = "";
  try {
    const snapshot = await invokeTauri<OverviewSnapshot<StorageUsageOverview>>("refresh_storage_usage_overview");
    if (!storageTabUnmounted) {
      applyStorageOverviewSnapshot(snapshot);
    }
  } catch (error) {
    setStorageMessage(toErrorMessage(error), true);
  }
}

async function handleCleanupStorageItem(item: StorageUsageItem) {
  const cleanupKind = String(item.cleanupKind || "").trim();
  if (!cleanupKind || item.cleanableFileCount <= 0) return;
  const confirmed = window.confirm(t("config.storage.cleanupConfirm", {
    name: storageCategoryLabel(item.id),
    size: formatBytes(item.cleanableBytes),
  }));
  if (!confirmed) return;
  cleanupBusyKind.value = cleanupKind;
  storageMessage.value = "";
  try {
    const result = await invokeTauri<StorageCleanupResult>("cleanup_storage_legacy_items", {
      input: { cleanupKind },
    });
    setStorageMessage(t("config.storage.cleanupSuccess", { size: formatBytes(result.freedBytes) }));
    await refreshStorageOverview();
  } catch (error) {
    setStorageMessage(toErrorMessage(error), true);
  } finally {
    cleanupBusyKind.value = "";
  }
}

async function handleOpenStorageItem(item: StorageUsageItem) {
  if (!localFileSystemAvailable) return;
  openingItemId.value = item.id;
  storageMessage.value = "";
  try {
    await openTransportStorageUsageItemDirectory(item.id);
  } catch (error) {
    setStorageMessage(toErrorMessage(error), true);
  } finally {
    openingItemId.value = "";
  }
}

async function handleExport() {
  busy.value = true;
  previewResult.value = null;
  exportMessage.value = "";
  try {
    const result = await exportTransportConfigMigrationPackage({
      password: exportPassword.value,
    });
    setExportMessage(t("config.migration.exportSuccess", { path: result.path }));
  } catch (error) {
    setExportMessage(toErrorMessage(error), true);
  } finally {
    busy.value = false;
  }
}

async function handleSelectImportPackage() {
  selectedImportPackage.value = null;
  importPassword.value = "";
  needImportPassword.value = false;
  previewResult.value = null;
  const selected = await pickTransportConfigMigrationPackage();
  if (!selected) {
    setImportMessage(t("config.migration.importCancelled"));
    return;
  }
  selectedImportPackage.value = selected;
  await handlePreviewImport();
}

async function handlePreviewImport() {
  if (!selectedImportPackage.value) {
    setImportMessage(t("config.migration.importCancelled"));
    return;
  }
  busy.value = true;
  importMessage.value = "";
  try {
    const input = {
      ...selectedImportPackage.value,
      password: importPassword.value,
    };
    previewResult.value = await previewTransportConfigMigrationPackage<MigrationPreviewResult>(input);
    needImportPassword.value = false;
    setImportMessage(t("config.migration.previewSuccess"));
  } catch (error) {
    previewResult.value = null;
    const raw = error as { code?: string; type?: string } | undefined;
    const text = toErrorMessage(error);
    if (
      raw?.code === PASSWORD_REQUIRED_CODE
      || raw?.type === PASSWORD_REQUIRED_CODE
      || text.includes(PASSWORD_REQUIRED_CODE)
      || text.includes("需要密码")
    ) {
      needImportPassword.value = true;
    }
    setImportMessage(text, true);
  } finally {
    busy.value = false;
  }
}

async function handleApplyImport() {
  if (!previewResult.value) return;
  busy.value = true;
  try {
    await applyTransportConfigMigrationPackage(previewResult.value.previewId);
    setImportMessage(t("config.migration.applySuccess"));
  } catch (error) {
    setImportMessage(toErrorMessage(error), true);
  } finally {
    busy.value = false;
  }
}

onMounted(() => {
  void loadStorageOverview();
});

onBeforeUnmount(() => {
  storageTabUnmounted = true;
  stopStoragePolling();
});
</script>
