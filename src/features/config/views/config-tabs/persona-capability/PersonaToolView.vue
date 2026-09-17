<template>
  <div class="grid gap-4">
    <div v-if="loading" class="flex items-center gap-2 text-sm opacity-60">
      <span class="loading loading-spinner loading-xs"></span>
      <span>{{ t("config.persona.capability.loading") }}</span>
    </div>

    <template v-else>
      <!-- 内置工具 -->
      <div class="grid gap-2">
        <div class="text-xs leading-snug text-base-content/60">{{ t("config.persona.capability.toolHint") }}</div>
        <div v-if="builtinTools.length === 0" class="rounded-xl border border-dashed border-base-300 bg-base-100 py-6 text-center text-sm opacity-60">
          {{ t("config.persona.capability.toolEmpty") }}
        </div>
        <div v-else class="overflow-hidden rounded-xl border border-base-200/80 bg-base-100">
          <label
            v-for="tool in builtinTools"
            :key="tool.function.name"
            class="flex cursor-pointer items-center justify-between gap-3 border-b border-base-200/60 px-4 py-3 last:border-b-0"
          >
            <span class="min-w-0 flex-1">
              <span class="block truncate text-sm font-medium">{{ tool.function.name }}</span>
              <span class="mt-0.5 block truncate text-xs opacity-60">{{ tool.function.description }}</span>
            </span>
            <input
              type="checkbox"
              class="checkbox checkbox-primary checkbox-sm shrink-0"
              :checked="builtinChecked(tool.function.name)"
              @change="toggleBuiltin(tool.function.name, ($event.target as HTMLInputElement).checked)"
            />
          </label>
        </div>
      </div>

      <!-- MCP 工具：按组（server）分块 -->
      <div class="grid gap-2">
        <div class="text-sm font-medium">{{ t("config.persona.capability.mcpTools") }}</div>
        <div v-if="servers.length === 0" class="rounded-xl border border-dashed border-base-300 bg-base-100 py-6 text-center text-sm opacity-60">
          {{ t("config.persona.capability.mcpEmpty") }}
        </div>
        <div
          v-for="server in servers"
          :key="server.id"
          class="overflow-hidden rounded-xl border border-base-200/80 bg-base-100"
        >
          <div class="flex items-center gap-2 border-b border-base-200/60 bg-base-200/40 px-4 py-2">
            <span class="truncate text-sm font-medium">{{ server.name || server.id }}</span>
            <span v-if="!server.enabled" class="badge badge-ghost badge-xs shrink-0">
              {{ t("config.persona.capability.mcpDisabled") }}
            </span>
          </div>
          <div v-if="(server.cachedTools || []).length === 0" class="px-4 py-3 text-xs opacity-60">
            {{ t("config.persona.capability.mcpNoTools") }}
          </div>
          <label
            v-for="tool in server.cachedTools || []"
            :key="tool.toolName"
            class="flex cursor-pointer items-center justify-between gap-3 border-b border-base-200/60 px-4 py-2.5 last:border-b-0"
          >
            <span class="min-w-0 flex-1">
              <span class="block truncate text-sm">{{ tool.toolName }}</span>
              <span class="mt-0.5 block truncate text-xs opacity-60">{{ tool.description }}</span>
            </span>
            <input
              type="checkbox"
              class="checkbox checkbox-primary checkbox-sm shrink-0"
              :checked="mcpChecked(server.id, tool.toolName)"
              @change="toggleMcp(server.id, tool.toolName, ($event.target as HTMLInputElement).checked)"
            />
          </label>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { FrontendToolDefinition, McpServerConfig, PersonaProfile } from "../../../../../types/app";
import { ensurePermissionControl, setListMembership } from "../../../utils/persona-capability";

const props = defineProps<{
  persona: PersonaProfile;
  builtinTools: FrontendToolDefinition[];
  servers: McpServerConfig[];
  loading?: boolean;
}>();

const { t } = useI18n();

function mcpFullName(serverId: string, toolName: string): string {
  return `${String(serverId || "").trim()}::${String(toolName || "").trim()}`;
}

function builtinChecked(name: string): boolean {
  const control = props.persona?.permissionControl;
  return (control?.builtinToolNames || []).some((item) => String(item || "").trim() === name);
}

function mcpChecked(serverId: string, toolName: string): boolean {
  const full = mcpFullName(serverId, toolName);
  const control = props.persona?.permissionControl;
  return (control?.mcpToolNames || []).some((item) => String(item || "").trim() === full);
}

// 工具页按「只勾选可用的」表达：一旦动手勾选，就切换到白名单口径并启用控制。
function markWhitelistActive(control: { enabled: boolean; mode: string }) {
  control.enabled = true;
  control.mode = "whitelist";
}

function toggleBuiltin(name: string, checked: boolean) {
  const control = ensurePermissionControl(props.persona);
  control.builtinToolNames = setListMembership(control.builtinToolNames, name, checked);
  markWhitelistActive(control);
}

function toggleMcp(serverId: string, toolName: string, checked: boolean) {
  const control = ensurePermissionControl(props.persona);
  control.mcpToolNames = setListMembership(control.mcpToolNames, mcpFullName(serverId, toolName), checked);
  markWhitelistActive(control);
}
</script>
