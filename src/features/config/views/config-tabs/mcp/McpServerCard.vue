<template>
  <div class="space-y-4">
    <!-- 基本信息 -->
    <div class="card card-sm card-border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold">{{ t('config.mcp.basicSection') }}</span>
          <div class="flex items-center gap-2">
            <span class="text-xs opacity-70">{{ t('config.mcp.statusLabel') }}</span>
            <span v-if="draft.lastStatus === 'ready' || draft.lastStatus === 'deployed'" class="badge badge-sm badge-success">
              {{ t('config.mcp.statusReady') }}
            </span>
            <span v-else-if="draft.lastStatus === 'stopped'" class="badge badge-sm badge-neutral">
              {{ t('config.mcp.statusStopped') }}
            </span>
            <span v-else-if="draft.lastStatus === 'starting' || draft.lastStatus === 'deploying'" class="badge badge-sm badge-warning">
              {{ t('config.mcp.statusStarting') }}
            </span>
            <span v-else-if="draft.lastStatus === 'stale'" class="badge badge-sm badge-warning">
              {{ t('config.mcp.statusStale') }}
            </span>
            <span v-else-if="draft.lastStatus === 'timeout'" class="badge badge-sm badge-error">
              {{ t('config.mcp.statusTimeout') }}
            </span>
            <span v-else-if="draft.lastStatus === 'disabled'" class="badge badge-sm badge-neutral">
              {{ t('config.mcp.statusDisabled') }}
            </span>
            <span v-else-if="draft.lastStatus === 'failed'" class="badge badge-sm badge-error">
              {{ t('config.mcp.statusFailed') }}
            </span>
            <span v-else class="badge badge-sm badge-ghost">
              {{ draft.lastStatus || t('config.mcp.statusUnknown') }}
            </span>
          </div>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div class="space-y-1">
            <label class="text-xs font-medium opacity-70">{{ t('config.mcp.serverId') }}</label>
            <div class="font-mono text-xs px-3 py-2 bg-base-200/60 rounded-field border border-base-300 select-all truncate">
              {{ draft.id }}
            </div>
          </div>
          <div class="space-y-1">
            <label class="text-xs font-medium opacity-70">{{ t('config.mcp.serverName') }}</label>
            <input
              v-model="draft.name"
              type="text"
              class="input input-bordered input-sm h-9 w-full text-xs"
              :placeholder="t('config.mcp.serverNamePlaceholder')"
              :disabled="disabled"
              @input="emitChange"
            />
          </div>
        </div>

        <div v-if="draft.lastError" class="p-2.5 rounded-field bg-error/10 border border-error/20 text-xs text-error">
          {{ draft.lastError }}
        </div>
      </div>
    </div>

    <!-- 组内成员（仅当多于 1 个成员时展示，单体服务不占位不复读） -->
    <div v-if="members.length > 1" class="card card-sm card-border border-base-300 bg-base-100">
      <div class="card-body gap-2.5 p-4">
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold">{{ t('config.mcp.membersSection') }}</span>
          <span class="badge badge-sm badge-neutral">{{ t('config.mcp.memberCount', { count: members.length }) }}</span>
        </div>
        <div class="divide-y divide-base-200">
          <div v-for="m in members" :key="m.name" class="flex items-center justify-between gap-2 py-2 text-xs">
            <div class="flex items-center gap-2 min-w-0">
              <span class="font-mono font-medium truncate">{{ m.name }}</span>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span v-if="m.toolCount > 0" class="opacity-70">{{ t('config.mcp.toolCount', { count: m.toolCount }) }}</span>
              <span class="badge badge-sm badge-ghost font-mono">{{ m.transport }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 配置定义 JSON -->
    <div class="card card-sm card-border border-base-300 bg-base-100">
      <div class="card-body gap-2 p-4">
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold">{{ t('config.mcp.configSection') }}</span>
        </div>
        <textarea
          v-model="draft.definitionJson"
          class="textarea textarea-bordered textarea-sm font-mono min-h-48 w-full bg-base-200/30 text-xs leading-relaxed"
          :placeholder="t('config.mcpServerCard.configPlaceholder')"
          :disabled="disabled"
          @input="emitChange"
        ></textarea>
      </div>
    </div>

    <!-- 工具列表 -->
    <McpToolList
      :tools="draft.toolItems"
      :elapsed-ms="draft.lastElapsedMs"
      :disabled="disabled"
      @toggle-tool="(payload) => $emit('toggleTool', { serverId: draft.id, ...payload })"
      @refresh-tools="$emit('refreshTools', draft.id)"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from "vue";
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

type McpMemberView = {
  name: string;
  transport: string;
  toolCount: number;
};

const props = defineProps<{
  server: McpServerView;
  disabled?: boolean;
  hasIssues?: boolean;
}>();

const emit = defineEmits<{
  (e: "change", server: McpServerView): void;
  (e: "remove", serverId: string): void;
  (e: "validate", server: McpServerView): void;
  (e: "fix", server: McpServerView): void;
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

function inferTransport(obj: Record<string, unknown>): string {
  const transport = String(obj.transport ?? obj.type ?? "").toLowerCase();
  if (transport === "sse") return "sse";
  if (obj.command) return "stdio";
  if (obj.url) return "streamable_http";
  return "-";
}

const members = computed<McpMemberView[]>(() => {
  const list: McpMemberView[] = [];
  try {
    const parsed = JSON.parse(draft.definitionJson) as unknown;
    const push = (name: string, obj: Record<string, unknown>) => {
      list.push({ name, transport: inferTransport(obj), toolCount: 0 });
    };
    if (Array.isArray(parsed)) {
      for (const item of parsed) {
        if (item && typeof item === "object") {
          push(String((item as Record<string, unknown>).name ?? "(未命名)"), item as Record<string, unknown>);
        }
      }
    } else if (parsed && typeof parsed === "object") {
      const root = parsed as Record<string, unknown>;
      const mcpServers = root.mcpServers;
      if (Array.isArray(mcpServers)) {
        for (const item of mcpServers) {
          if (item && typeof item === "object") {
            push(String((item as Record<string, unknown>).name ?? "(未命名)"), item as Record<string, unknown>);
          }
        }
      } else if (mcpServers && typeof mcpServers === "object") {
        for (const [name, obj] of Object.entries(mcpServers as Record<string, unknown>)) {
          if (obj && typeof obj === "object") push(name, obj as Record<string, unknown>);
        }
      } else {
        const hasDirectField = ["command", "url", "transport", "type", "args", "env", "cwd", "headers", "httpHeaders", "envHttpHeaders", "bearerTokenEnvVar", "enabledTools", "disabledTools"].some(
          (key) => key in root,
        );
        if (hasDirectField) {
          push(String(root.name ?? "(未命名)"), root);
        } else {
          for (const [name, obj] of Object.entries(root)) {
            if (obj && typeof obj === "object") push(name, obj as Record<string, unknown>);
          }
        }
      }
    }
  } catch {
    // JSON 未解析时保持空列表
  }
  for (const tool of draft.toolItems) {
    const idx = tool.toolName.lastIndexOf("_");
    if (idx <= 0) continue;
    const memberName = tool.toolName.slice(0, idx);
    const member = list.find((m) => m.name === memberName);
    if (member) member.toolCount += 1;
  }
  return list;
});

function emitChange() {
  draft.isDirty = true;
  emit("change", { ...draft });
}
</script>
