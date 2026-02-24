<template>
  <div>
    <div class="space-y-3">
      <div class="flex items-center gap-2">
        <button class="btn btn-xs bg-base-100" type="button" :disabled="disabled" @click="emitValidate">{{ t('config.mcpServerCard.validate') }}</button>
        <button
          class="btn btn-xs"
          :class="draft.enabled ? 'btn-warning' : 'btn-success'"
          type="button"
          :disabled="disabled"
          @click="emitToggleDeploy"
        >
          {{ draft.enabled ? t('config.mcpServerCard.stop') : t('config.mcpServerCard.deploy') }}
        </button>
        <div class="flex-1 rounded-md border border-base-300 bg-base-100 px-3 py-1.5 text-sm leading-5">
          {{ draft.name || t('config.mcpServerCard.displayNamePlaceholder') }}
        </div>
        <button class="btn btn-xs btn-warning" type="button" :disabled="disabled" @click="$emit('remove', draft.id)">{{ t('config.mcpServerCard.delete') }}</button>
      </div>

      <div class="collapse collapse-arrow bg-base-100 border-base-300 border">
        <input type="checkbox" />
        <div class="collapse-title font-semibold">{{ t('config.mcpServerCard.configJson') }}</div>
        <div class="collapse-content">
          <textarea
            v-model="draft.definitionJson"
            class="textarea textarea-xs font-mono min-h-40 w-full bg-base-100"
            :placeholder="t('config.mcpServerCard.configPlaceholder')"
            @input="emitChange"
          ></textarea>
        </div>
      </div>

      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 text-[11px]">
          <span class="opacity-70">{{ t('config.mcpServerCard.status') }}</span>
          <span v-if="draft.lastStatus === 'deployed'" class="badge badge-xs badge-success">已部署</span>
          <span v-else-if="draft.lastStatus === 'stopped'" class="badge badge-xs badge-neutral">已停止</span>
          <span v-else-if="draft.lastStatus === 'deploying'" class="badge badge-xs badge-warning">部署中</span>
          <span v-else-if="draft.lastStatus === 'failed'" class="badge badge-xs badge-error">失败</span>
          <span v-else class="badge badge-xs badge-ghost">{{ draft.lastStatus || "-" }}</span>
          <span v-if="draft.lastError" class="text-error truncate max-w-50" :title="draft.lastError"> | {{ draft.lastError }}</span>
        </div>
        <div></div>
      </div>

      <McpToolList
        :tools="draft.toolItems"
        :elapsed-ms="draft.lastElapsedMs"
        :disabled="disabled"
        @toggle-tool="(payload) => $emit('toggleTool', { serverId: draft.id, ...payload })"
        @refresh-tools="$emit('refreshTools', draft.id)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { McpServerConfig, McpToolDescriptor } from "../../../../../types/app";
import McpToolList from "./McpToolList.vue";

const { t } = useI18n();

type McpServerView = McpServerConfig & {
  toolItems: McpToolDescriptor[];
  lastElapsedMs: number;
  isDraft: boolean;
  isDirty: boolean;
};

const props = defineProps<{
  server: McpServerView;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "change", server: McpServerView): void;
  (e: "remove", serverId: string): void;
  (e: "validate", server: McpServerView): void;
  (e: "toggleDeploy", server: McpServerView): void;
  (e: "toggleTool", payload: { serverId: string; toolName: string; enabled: boolean }): void;
  (e: "refreshTools", serverId: string): void;
}>();

const draft = reactive<McpServerView>({ ...props.server });

watch(
  () => props.server,
  (next) => {
    Object.assign(draft, next);
  },
  { deep: true },
);

function emitChange() {
  emit("change", { ...draft });
}

function emitValidate() {
  emit("validate", { ...draft });
}

function emitToggleDeploy() {
  emit("toggleDeploy", { ...draft });
}
</script>
