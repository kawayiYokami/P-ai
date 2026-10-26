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

      <!-- 参数字段：字段名 / 类型 / 说明 三列对齐，靠排版压出层级 -->
      <div
        v-if="toolParameters(item.id).length"
        class="mt-1.5 grid items-baseline gap-x-3 gap-y-[3px] border-t border-base-300/50 pt-1.5 text-caption leading-relaxed"
        :style="{ gridTemplateColumns: 'minmax(6.5rem, auto) auto minmax(0, 1fr)' }"
      >
        <template v-for="param in toolParameters(item.id)" :key="`${item.id}-param-${param.name}`">
          <span class="font-mono text-base-content/85 whitespace-nowrap">
            {{ param.name }}<span
              v-if="param.required"
              class="text-error/60 text-micro ml-px"
              :title="t('config.mcpToolList.required')"
            >*</span>
          </span>
          <span class="font-mono text-micro text-base-content/35 whitespace-nowrap">{{ param.type }}</span>
          <span class="text-base-content/60" :title="param.tooltipText">
            <span v-if="param.constraints" class="font-mono text-micro text-base-content/40 mr-1">({{ param.constraints }})</span>
            <span v-if="param.defaultValue !== undefined" class="font-mono text-micro text-base-content/40 mr-1">{{ t('config.mcpToolList.default', { value: param.defaultValue }) }}</span>
            <span>{{ param.description }}</span>
            <span v-if="param.enumValues?.length" class="font-mono text-micro text-base-content/45 ml-1">
              [{{ t('config.mcpToolList.enumValues') }}: {{ param.enumValues.join(', ') }}]
            </span>
          </span>
        </template>
      </div>

      <!-- 示例 -->
      <div v-if="toolParameterExamples(item.id).length" class="mt-1.5 grid gap-1">
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

// 参数结构按工具缓存，避免模板里多次取值时重复解析同一份 schema。
const parametersByToolId = computed(() => {
  const map = new Map<string, ToolParameterDetail[]>();
  for (const tool of props.tools) {
    map.set(tool.toolName, parseToolParameters(tool.parameters));
  }
  return map;
});

const examplesByToolId = computed(() => {
  const map = new Map<string, string[]>();
  for (const tool of props.tools) {
    map.set(tool.toolName, parseToolExamples(tool.parameters));
  }
  return map;
});

function toolParameters(id: string): ToolParameterDetail[] {
  return parametersByToolId.value.get(id) ?? [];
}

function toolParameterExamples(id: string): string[] {
  return examplesByToolId.value.get(id) ?? [];
}
</script>
