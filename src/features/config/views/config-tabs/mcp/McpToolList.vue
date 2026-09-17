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
      <div v-if="toolParameterSummary(item.id).length" class="mt-1 flex flex-wrap gap-1">
        <span
          v-for="paramText in toolParameterSummary(item.id)"
          :key="`${item.id}-param-${paramText}`"
          class="text-caption px-1.5 py-0.5 rounded bg-base-200 border border-base-300 opacity-80"
        >
          {{ paramText }}
        </span>
      </div>
      <div v-if="toolParameterExamples(item.id).length" class="mt-1 grid gap-1">
        <pre
          v-for="example in toolParameterExamples(item.id)"
          :key="`${item.id}-example-${example}`"
          class="text-caption leading-4 px-2 py-1 rounded bg-base-200 border border-base-300 opacity-90 whitespace-pre-wrap overflow-x-auto"
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

type ToolSchemaShape = Record<string, unknown>;

function asToolSchemaShape(value: unknown): ToolSchemaShape {
  return value && typeof value === "object" ? (value as ToolSchemaShape) : {};
}

function toolSchemaTypeText(shape: ToolSchemaShape): string {
  const typeValue = shape.type;
  if (Array.isArray(typeValue)) {
    return typeValue.map(String).join(" | ");
  }
  return String(typeValue || "any");
}

function toolSchemaSummaryLine(name: string, shape: ToolSchemaShape, required: boolean): string {
  const requiredText = required ? "*" : "";
  const enumValues = Array.isArray(shape.enum) ? ` [${shape.enum.map(String).join(", ")}]` : "";
  const minText = shape.minimum !== undefined ? ` >= ${shape.minimum}` : "";
  const maxText = shape.maximum !== undefined ? ` <= ${shape.maximum}` : "";
  const rangeText = `${enumValues}${minText}${maxText}`.trim();
  return `${requiredText}${name}: ${toolSchemaTypeText(shape)}${rangeText ? ` ${rangeText}` : ""}`;
}

function collectToolSchemaSummaryLines(
  properties: ToolSchemaShape,
  requiredRaw: string[],
  prefix = "",
): string[] {
  const lines: string[] = [];
  for (const [name, schema] of Object.entries(properties)) {
    const shape = asToolSchemaShape(schema);
    const path = prefix ? `${prefix}.${name}` : name;
    lines.push(toolSchemaSummaryLine(path, shape, requiredRaw.includes(name)));

    const nestedPropertiesRaw = shape.properties;
    if (nestedPropertiesRaw && typeof nestedPropertiesRaw === "object") {
      const nestedRequired = Array.isArray(shape.required) ? shape.required.map(String) : [];
      lines.push(
        ...collectToolSchemaSummaryLines(
          nestedPropertiesRaw as ToolSchemaShape,
          nestedRequired,
          path,
        ),
      );
    }

    const itemsShape = asToolSchemaShape(shape.items);
    const itemPropertiesRaw = itemsShape.properties;
    if (itemPropertiesRaw && typeof itemPropertiesRaw === "object") {
      const nestedRequired = Array.isArray(itemsShape.required) ? itemsShape.required.map(String) : [];
      lines.push(
        ...collectToolSchemaSummaryLines(
          itemPropertiesRaw as ToolSchemaShape,
          nestedRequired,
          `${path}[]`,
        ),
      );
    }
  }
  return lines;
}

function toolParameterSummary(id: string): string[] {
  const parameters = toolById(id)?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  const requiredRaw = Array.isArray(root.required) ? root.required : [];
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return collectToolSchemaSummaryLines(
    propertiesRaw as ToolSchemaShape,
    requiredRaw.map(String),
  );
}

function formatSchemaExample(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function collectToolSchemaExamples(
  properties: ToolSchemaShape,
  prefix = "",
): string[] {
  const examples: string[] = [];
  for (const [name, schema] of Object.entries(properties)) {
    const shape = asToolSchemaShape(schema);
    const path = prefix ? `${prefix}.${name}` : name;

    if (shape.example !== undefined && shape.example !== null) {
      const singleExample = formatSchemaExample(shape.example);
      if (singleExample) {
        examples.push(`${path} 示例:\n${singleExample}`);
      }
    }

    const exampleList = Array.isArray(shape.examples) ? shape.examples : [];
    for (const rawExample of exampleList) {
      if (rawExample === undefined || rawExample === null) continue;
      const text = formatSchemaExample(rawExample);
      if (text) {
        examples.push(`${path} 示例:\n${text}`);
      }
    }

    const nestedPropertiesRaw = shape.properties;
    if (nestedPropertiesRaw && typeof nestedPropertiesRaw === "object") {
      examples.push(...collectToolSchemaExamples(nestedPropertiesRaw as ToolSchemaShape, path));
    }

    const itemsShape = asToolSchemaShape(shape.items);
    const itemPropertiesRaw = itemsShape.properties;
    if (itemPropertiesRaw && typeof itemPropertiesRaw === "object") {
      examples.push(...collectToolSchemaExamples(itemPropertiesRaw as ToolSchemaShape, `${path}[]`));
    }
  }
  return examples;
}

function toolParameterExamples(id: string): string[] {
  const parameters = toolById(id)?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return Array.from(new Set(collectToolSchemaExamples(propertiesRaw as ToolSchemaShape)));
}
</script>
