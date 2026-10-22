export type ToolSchemaShape = Record<string, unknown>;

export type ToolParameterDetail = {
  name: string;
  type: string;
  required: boolean;
  description: string;
  defaultValue?: string;
  enumValues?: string[];
  constraints?: string;
  summaryText: string;
  tooltipText: string;
};

export function asToolSchemaShape(value: unknown): ToolSchemaShape {
  return value && typeof value === "object" ? (value as ToolSchemaShape) : {};
}

export function toolSchemaTypeText(shape: ToolSchemaShape): string {
  const typeValue = shape.type;
  if (Array.isArray(typeValue)) {
    return typeValue.map(String).join(" | ");
  }
  if (typeValue === "array") {
    const itemsShape = asToolSchemaShape(shape.items);
    if (itemsShape.type) {
      return `${toolSchemaTypeText(itemsShape)}[]`;
    }
    return "array";
  }
  return String(typeValue || "any");
}

export function formatSchemaDefault(value: unknown): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

export function formatSchemaConstraints(shape: ToolSchemaShape): string {
  const parts: string[] = [];
  if (shape.minimum !== undefined) {
    parts.push(`>= ${shape.minimum}`);
  }
  if (shape.maximum !== undefined) {
    parts.push(`<= ${shape.maximum}`);
  }
  if (shape.minLength !== undefined) {
    parts.push(`len >= ${shape.minLength}`);
  }
  if (shape.maxLength !== undefined) {
    parts.push(`len <= ${shape.maxLength}`);
  }
  return parts.join(", ");
}

export function toolSchemaSummaryLine(name: string, shape: ToolSchemaShape, required: boolean): string {
  const requiredText = required ? "*" : "";
  const enumValues = Array.isArray(shape.enum) ? ` [${shape.enum.map(String).join(", ")}]` : "";
  const minText = shape.minimum !== undefined ? ` >= ${shape.minimum}` : "";
  const maxText = shape.maximum !== undefined ? ` <= ${shape.maximum}` : "";
  const rangeText = `${enumValues}${minText}${maxText}`.trim();
  return `${requiredText}${name}: ${toolSchemaTypeText(shape)}${rangeText ? ` ${rangeText}` : ""}`;
}

export function collectToolParameters(
  properties: ToolSchemaShape,
  requiredRaw: string[],
  prefix = "",
): ToolParameterDetail[] {
  const list: ToolParameterDetail[] = [];
  for (const [name, schema] of Object.entries(properties)) {
    const shape = asToolSchemaShape(schema);
    const path = prefix ? `${prefix}.${name}` : name;
    const isRequired = requiredRaw.includes(name);
    const typeText = toolSchemaTypeText(shape);
    const description = typeof shape.description === "string" ? shape.description.trim() : "";
    const defaultValue = formatSchemaDefault(shape.default);
    const enumValues = Array.isArray(shape.enum) && shape.enum.length > 0 ? shape.enum.map(String) : undefined;
    const constraints = formatSchemaConstraints(shape) || undefined;
    const summaryText = toolSchemaSummaryLine(path, shape, isRequired);

    // 组合悬停提示文案
    const tooltipParts: string[] = [
      `${path}: ${typeText}${isRequired ? " (必填)" : " (可选)"}`,
    ];
    if (constraints) {
      tooltipParts.push(`范围: ${constraints}`);
    }
    if (defaultValue !== undefined) {
      tooltipParts.push(`默认值: ${defaultValue}`);
    }
    if (enumValues && enumValues.length > 0) {
      tooltipParts.push(`可选值: [${enumValues.join(", ")}]`);
    }
    if (description) {
      tooltipParts.push(`\n${description}`);
    }

    list.push({
      name: path,
      type: typeText,
      required: isRequired,
      description,
      defaultValue,
      enumValues,
      constraints,
      summaryText,
      tooltipText: tooltipParts.join("\n"),
    });

    const nestedPropertiesRaw = shape.properties;
    if (nestedPropertiesRaw && typeof nestedPropertiesRaw === "object") {
      const nestedRequired = Array.isArray(shape.required) ? shape.required.map(String) : [];
      list.push(
        ...collectToolParameters(
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
      list.push(
        ...collectToolParameters(
          itemPropertiesRaw as ToolSchemaShape,
          nestedRequired,
          `${path}[]`,
        ),
      );
    }
  }
  return list;
}

export function parseToolParameters(parameters: unknown): ToolParameterDetail[] {
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  const requiredRaw = Array.isArray(root.required) ? root.required.map(String) : [];
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return collectToolParameters(propertiesRaw as ToolSchemaShape, requiredRaw);
}

export function formatSchemaExample(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function collectToolSchemaExamples(
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

export function parseToolExamples(parameters: unknown): string[] {
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const propertiesRaw = root.properties;
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return Array.from(new Set(collectToolSchemaExamples(propertiesRaw as ToolSchemaShape)));
}
