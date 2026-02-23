<template>
  <div v-if="tools.length > 0" class="rounded-box border border-base-300 p-3 bg-base-200">
    <div class="text-xs font-medium mb-3">{{ t('config.mcpToolList.toolList') }}（{{ tools.length }}）</div>
    <div class="space-y-3">
      <div v-for="tool in tools" :key="tool.toolName" class="flex items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <details class="collapse bg-base-100 border-base-300 border">
            <summary class="collapse-title min-h-0 py-1 px-2">{{ tool.toolName }}</summary>
            <div class="collapse-content py-1 px-2">
              {{ tool.description || t('config.mcpToolList.noDescription') }}
            </div>
          </details>
        </div>
        <input
          type="checkbox"
          class="toggle toggle-xs mt-1"
          :checked="tool.enabled"
          :disabled="disabled"
          @change="onToggle($event, tool.toolName)"
        />
      </div>
    </div>
    <div class="text-[11px] opacity-70 mt-3">{{ t('config.mcpToolList.recentElapsed') }}: {{ elapsedMs }}ms</div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { McpToolDescriptor } from "../../../../../types/app";

const { t } = useI18n();

const props = defineProps<{
  tools: McpToolDescriptor[];
  elapsedMs: number;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "toggleTool", payload: { toolName: string; enabled: boolean }): void;
}>();

function onToggle(event: Event, toolName: string) {
  const target = event.target as HTMLInputElement | null;
  emit("toggleTool", {
    toolName,
    enabled: !!target?.checked,
  });
}
</script>
