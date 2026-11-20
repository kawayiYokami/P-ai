<template>
  <McpServerDefinitionEditor v-model:definition-json="definitionJson" />
</template>

<script setup lang="ts">
import { ref } from "vue";
import McpServerDefinitionEditor from "../../config/views/config-tabs/mcp/McpServerDefinitionEditor.vue";

const definitionJson = ref(
  JSON.stringify(
    {
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "E:\\github"],
      env: { DEBUG: "0" },
    },
    null,
    2,
  ),
);
</script>
