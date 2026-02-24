<template>
  <div v-if="tools.length > 0" class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
    <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
      <div class="font-medium">{{ t('config.mcpToolList.toolList') }}（{{ tools.length }}）</div>
      <div class="ml-auto flex items-center gap-2">
        <span class="text-[11px] opacity-70">{{ t('config.mcpToolList.recentElapsed') }}: {{ elapsedMs }}ms</span>
        <button
          type="button"
          class="btn btn-xs btn-ghost"
          :disabled="disabled"
          @click="emit('refreshTools')"
        >
          {{ t('config.mcp.refresh') }}
        </button>
      </div>
    </div>
    <div class="divide-y divide-base-300/60">
      <div
        v-for="tool in tools"
        :key="tool.toolName"
        class="flex items-start gap-3 px-3 py-2"
      >
        <input
          type="checkbox"
          class="toggle toggle-xs mt-1 shrink-0"
          :checked="tool.enabled"
          :disabled="disabled"
          @change="onToggle($event, tool.toolName)"
        />
        <div class="min-w-0 flex-1">
          <div class="font-medium">{{ tool.toolName }}</div>
          <div class="text-[11px] opacity-60">{{ tool.description || t('config.mcpToolList.noDescription') }}</div>
        </div>
      </div>
    </div>
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
  (e: "refreshTools"): void;
}>();

function onToggle(event: Event, toolName: string) {
  const target = event.target as HTMLInputElement | null;
  emit("toggleTool", {
    toolName,
    enabled: !!target?.checked,
  });
}
</script>
