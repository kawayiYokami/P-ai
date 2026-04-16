<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-2">
    <div v-if="title">{{ title }}</div>
    <TerminalApprovalPatchSample
      v-if="mode === 'patch'"
      class="h-full min-h-0"
      :lines="normalizedLines"
      :diff-only="true"
      highlight-style="background"
      :show-prefixes="false"
    />
    <pre v-else class="h-full min-h-0 overflow-auto rounded-box border border-base-300 bg-base-200/40 p-3 text-[12px] leading-6"><code>{{ code }}</code></pre>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import TerminalApprovalPatchSample from "../../shell/components/TerminalApprovalPatchSample.vue";

const props = defineProps<{
  title?: string;
  code: string;
  mode?: "plain" | "patch";
}>();

const normalizedLines = computed(() =>
  String(props.code || "").replace(/\r/g, "").split("\n"),
);
</script>

<style scoped>
:deep(.mockup-code) {
  height: 100%;
  max-height: none;
  border-radius: 0;
}
</style>
