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
      <div v-for="tool in toolApiConfig.tools" :key="tool.id" class="card card-compact bg-base-100 border border-base-300">
        <div class="card-body py-2 px-3">
          <div class="flex items-center justify-between gap-2">
            <div class="text-xs font-medium">{{ tool.id }}</div>
            <div class="flex items-center gap-2">
              <button v-if="tool.id === 'memory-save'" class="btn btn-xs btn-ghost bg-base-100" @click="$emit('openMemoryViewer')">{{ t("config.tools.viewMemory") }}</button>
              <div class="badge" :class="statusBadgeClass(tool.id)">{{ statusText(tool.id) }}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </template>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ToolLoadStatus } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  toolApiConfig: ApiConfigItem | null;
  toolStatuses: ToolLoadStatus[];
}>();

defineEmits<{
  (e: "openMemoryViewer"): void;
}>();

const { t } = useI18n();

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? "unknown";
}

function statusBadgeClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "loaded") return "badge-success";
  if (status === "failed" || status === "timeout") return "badge-error";
  if (status === "disabled") return "badge-ghost";
  return "badge-outline";
}
</script>


