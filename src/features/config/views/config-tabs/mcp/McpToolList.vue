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
  >
    <template #item-extra="{ item }">
      <div v-if="toolCompatibilityError(item.id)" class="mt-1 text-xs text-error break-all">
        {{ toolCompatibilityError(item.id) }}
      </div>

      <!-- 纯排版参数字段列表 -->
      <div v-if="toolParameters(item.id).length" class="mt-2 space-y-1 text-xs leading-relaxed">
        <div
          v-for="param in toolParameters(item.id)"
          :key="`${item.id}-param-${param.name}`"
          class="text-base-content/75"
        >
          <span class="font-mono font-medium text-base-content">{{ param.name }}</span><span v-if="param.required" class="text-error font-bold ml-0.5" title="必填">*</span>
          <span class="font-mono text-[11px] text-base-content/50 ml-1.5">{{ param.type }}</span>
          <span v-if="param.constraints" class="font-mono text-[11px] text-base-content/40 ml-1">({{ param.constraints }})</span>
          <span v-if="param.defaultValue !== undefined" class="font-mono text-[11px] text-base-content/40 ml-1">{{ t('config.mcpToolList.default', { value: param.defaultValue }) }}</span>
          <template v-if="param.description || param.enumValues?.length">
            <span class="text-base-content/30 mx-1.5">—</span>
            <span v-if="param.description">{{ param.description }}</span>
            <span v-if="param.enumValues?.length" class="text-base-content/50 font-mono ml-1.5 text-[11px]">
              [{{ t('config.mcpToolList.enumValues') }}: {{ param.enumValues.join(', ') }}]
            </span>
          </template>
        </div>
      </div>

      <!-- 示例 -->
      <div v-if="toolParameterExamples(item.id).length" class="mt-2 grid gap-1">
        <pre
          v-for="example in toolParameterExamples(item.id)"
          :key="`${item.id}-example-${example}`"
          class="text-caption leading-4 px-2 py-1.5 rounded bg-base-200/60 border border-base-300/40 opacity-90 whitespace-pre-wrap overflow-x-auto"
        >{{ example }}</pre>
      </div>
    </template>
  </ToolListCard>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { McpToolDescriptor } from "../../../../../types/app";
import ToolListCard, { type ToolListItem } from "../../../components/ToolListCard.vue";
import {
  parseToolParameters,
  parseToolExamples,
  type ToolParameterDetail,
} from "./mcp-tool-parameters";

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
  props.tools.map((tool) => {
    const compatibilityError = String(tool.compatibilityError || "").trim();
    return {
      id: tool.toolName,
      name: tool.toolName,
      description: tool.description,
      enabled: tool.enabled,
      statusClass: compatibilityError ? "bg-error" : tool.enabled ? "bg-success" : "bg-base-content/30",
      statusTitle: compatibilityError || (tool.enabled ? t("config.mcp.toolEnabled") : t("config.mcp.toolDisabled")),
    };
  }),
);

function onToggleItem(payload: { id: string; enabled: boolean }) {
  emit("toggleTool", {
    toolName: payload.id,
    enabled: payload.enabled,
  });
}

function toolById(id: string): McpToolDescriptor | undefined {
  return props.tools.find((tool) => tool.toolName === id);
}

function toolCompatibilityError(id: string): string {
  return String(toolById(id)?.compatibilityError || "").trim();
}

function toolParameters(id: string): ToolParameterDetail[] {
  const tool = toolById(id);
  if (!tool) return [];
  return parseToolParameters(tool.parameters);
}

function toolParameterExamples(id: string): string[] {
  const tool = toolById(id);
  if (!tool) return [];
  return parseToolExamples(tool.parameters);
}
</script>
