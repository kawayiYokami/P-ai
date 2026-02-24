<template>
  <ToolListCard
    :title="t('config.mcpToolList.toolList')"
    :items="toolItems"
    :disabled="disabled"
    :refreshable="true"
    :refresh-label="t('config.mcp.refresh')"
    :no-description-text="t('config.mcpToolList.noDescription')"
    :elapsed-label="t('config.mcpToolList.recentElapsed')"
    :elapsed-ms="elapsedMs"
    @toggle-item="onToggleItem"
    @refresh="emit('refreshTools')"
  />
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { McpToolDescriptor } from "../../../../../types/app";
import ToolListCard, { type ToolListItem } from "../../../components/ToolListCard.vue";

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

const toolItems = computed<ToolListItem[]>(() =>
  props.tools.map((tool) => ({
    id: tool.toolName,
    name: tool.toolName,
    description: tool.description,
    enabled: tool.enabled,
    statusClass: tool.enabled ? "bg-success" : "bg-base-content/30",
    statusTitle: tool.enabled ? t("config.mcp.toolEnabled") : t("config.mcp.toolDisabled"),
  })),
);

function onToggleItem(payload: { id: string; enabled: boolean }) {
  emit("toggleTool", {
    toolName: payload.id,
    enabled: payload.enabled,
  });
}
</script>
