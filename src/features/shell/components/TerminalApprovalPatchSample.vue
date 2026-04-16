<script setup lang="ts">
import { computed } from "vue";

type ApprovalLineKind = "add" | "remove" | "warning" | "normal";
type ParsedLine = {
  line: string;
  kind: ApprovalLineKind;
  prefix?: string;
};

const props = defineProps<{
  lines: string[];
  diffOnly?: boolean;
  highlightStyle?: "text" | "background";
  showPrefixes?: boolean;
}>();

const lineClassMap: Record<ApprovalLineKind, string> = {
  add: "text-success",
  remove: "text-error",
  warning: "bg-warning text-warning-content",
  normal: "",
};

const lineRowClassMap: Record<ApprovalLineKind, string> = {
  add: "bg-success/10",
  remove: "bg-error/10",
  warning: "bg-warning/20",
  normal: "",
};

function getPatchLineNumbers(hunkLine: string) {
  const match = hunkLine.match(/^@@\s+-([0-9]+)(?:,[0-9]+)?\s+\+([0-9]+)(?:,[0-9]+)?\s+@@/);
  if (!match) {
    return null;
  }
  return {
    oldLine: Number.parseInt(match[1], 10),
    newLine: Number.parseInt(match[2], 10),
  };
}

const parsedLines = computed<ParsedLine[]>(() => {
  const rawLines = props.lines.map((item) => String(item || "").replace(/\r/g, ""));
  const oldLineRef = { value: 0 };
  const newLineRef = { value: 0 };
  const diffOnly = props.diffOnly !== false;
  return rawLines.flatMap<ParsedLine>((line) => {
    if (line.startsWith("@@")) {
      const range = getPatchLineNumbers(line);
      if (range) {
        oldLineRef.value = range.oldLine;
        newLineRef.value = range.newLine;
      }
      return [];
    }

    if (
      line.startsWith("*** Begin Patch")
      || line.startsWith("*** End Patch")
      || line.startsWith("*** Update File:")
      || line.startsWith("*** Add File:")
      || line.startsWith("*** Delete File:")
    ) {
      return [];
    }

    if (line.trim() === "Error!") {
      return [{ line, kind: "warning" as const }];
    }

    if (line.startsWith("+") && line[1] !== "+") {
      const prefix = newLineRef.value > 0 ? String(newLineRef.value) : "";
      newLineRef.value += 1;
      return [{ line, kind: "add" as const, prefix }];
    }

    if (line.startsWith("-") && line[1] !== "-") {
      const prefix = oldLineRef.value > 0 ? String(oldLineRef.value) : "";
      oldLineRef.value += 1;
      return [{ line, kind: "remove" as const, prefix }];
    }

    if (oldLineRef.value > 0 || newLineRef.value > 0) {
      oldLineRef.value += 1;
      newLineRef.value += 1;
    }

    if (diffOnly) {
      return [];
    }

    return [{
      line,
      kind: "normal" as const,
      prefix: undefined,
    }];
  });
});
</script>

<template>
  <div
    class="mockup-code w-full max-h-[60vh] overflow-x-auto overflow-y-auto"
    :class="{ 'approval-patch-sample--no-prefix': props.showPrefixes === false }"
  >
    <pre
      v-for="(item, idx) in parsedLines"
      :key="idx"
      :data-prefix="item.prefix"
      :class="props.highlightStyle === 'background' ? lineRowClassMap[item.kind] : ''"
    ><code :class="props.highlightStyle === 'background' ? '' : lineClassMap[item.kind]">{{ item.line }}</code></pre>
  </div>
</template>

<style scoped>
.approval-patch-sample--no-prefix pre::before {
  display: none;
}
</style>
