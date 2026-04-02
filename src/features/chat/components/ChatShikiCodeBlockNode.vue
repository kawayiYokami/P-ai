<template>
  <div class="ecall-shiki-codeblock markstream-vue ecall-shiki-codeblock-dark">
    <MarkdownCodeBlockNode v-bind="resolvedProps" />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { MarkdownCodeBlockNode } from "markstream-vue";
import type { CodeBlockNodeProps } from "markstream-vue";

const props = defineProps<CodeBlockNodeProps>();

const resolvedNode = computed(() => {
  if (!props.node?.diff) return props.node;
  return {
    ...props.node,
    language: "diff",
    code: String(props.node.raw ?? ""),
    diff: false,
    originalCode: undefined,
    updatedCode: undefined,
  };
});

const resolvedProps = computed(() => ({
  ...props,
  node: resolvedNode.value,
  darkTheme: "github-dark",
  lightTheme: "github-dark",
  isDark: true,
  stream: false,
  loading: false,
}));
</script>

<style scoped>
.ecall-shiki-codeblock {
  min-width: 0;
}

.ecall-shiki-codeblock-dark {
  color: #e5e7eb;
}

.ecall-shiki-codeblock-dark :deep(pre),
.ecall-shiki-codeblock-dark :deep(.shiki) {
  background: #101828 !important;
}

</style>
