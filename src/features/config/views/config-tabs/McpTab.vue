<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <div class="text-sm opacity-70">{{ t('config.mcp.serverList') }}</div>
        <select
          v-if="servers.length > 0"
          v-model="selectedServerId"
          class="select select-sm select-bordered w-[clamp(14rem,40vw,34rem)] max-w-full"
          :disabled="loading"
        >
          <option v-for="server in servers" :key="server.id" :value="server.id">
            {{ server.name || server.id }}
          </option>
        </select>
      </div>
      <div class="flex items-center gap-2">
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200" type="button" @click="reloadServers" :disabled="loading">{{ t('config.mcp.refresh') }}</button>
        <button class="btn btn-sm btn-primary" type="button" @click="openMcpDir" :disabled="loading">{{ t('config.mcp.openDir') }}</button>
        <button class="btn btn-sm btn-primary" type="button" @click="addServer">{{ t('config.mcp.add') }}</button>
      </div>
    </div>

    <div class="divider" v-if="servers.length > 0"></div>

    <div v-if="loading" class="text-sm opacity-70">{{ t('config.mcp.loading') }}</div>

    <McpServerCard
      v-if="selectedServer"
      :key="selectedServer.id"
      :server="selectedServer"
      :disabled="loading"
      @remove="removeServer"
      @validate="validateDefinition"
      @toggle-deploy="toggleDeploy"
      @toggle-tool="onToggleTool"
      @refresh-tools="refreshTools"
    />

    <div v-if="statusText" class="text-sm" :class="statusError ? 'text-error' : 'opacity-70'">
      {{ statusText }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import type {
  McpDefinitionValidateResult,
  McpListServerToolsResult,
  McpServerConfig,
  McpToolDescriptor,
} from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";
import McpServerCard from "./mcp/McpServerCard.vue";

const { t } = useI18n();

type McpServerView = McpServerConfig & {
  toolItems: McpToolDescriptor[];
  lastElapsedMs: number;
  isDraft: boolean;
  isDirty: boolean;
};

const loading = ref(false);
const statusText = ref("");
const statusError = ref(false);
const servers = ref<McpServerView[]>([]);
const selectedServerId = ref("");

const selectedServer = computed(() =>
  servers.value.find((s) => s.id === selectedServerId.value) ?? null,
);

function setStatus(text: string, isError = false) {
  statusText.value = text;
  statusError.value = isError;
}

function toView(server: McpServerConfig): McpServerView {
  return {
    ...server,
    toolItems: [],
    lastElapsedMs: 0,
    isDraft: false,
    isDirty: false,
  };
}

function upsertServer(local: McpServerView) {
  const idx = servers.value.findIndex((s) => s.id === local.id);
  if (idx >= 0) {
    servers.value[idx] = {
      ...servers.value[idx],
      ...local,
    };
    return;
  }
  servers.value.unshift(local);
  ensureSelectedServer();
}

function ensureSelectedServer() {
  if (servers.value.length === 0) {
    selectedServerId.value = "";
    return;
  }
  if (!servers.value.some((s) => s.id === selectedServerId.value)) {
    selectedServerId.value = servers.value[0].id;
  }
}

async function reloadServers() {
  loading.value = true;
  try {
    const list = await invokeTauri<McpServerConfig[]>("mcp_list_servers");
    servers.value = list.map(toView);
    ensureSelectedServer();
    const enabledServers = servers.value.filter((s) => s.enabled);
    if (enabledServers.length > 0) {
      const results = await Promise.allSettled(
        enabledServers.map((server) =>
          invokeTauri<McpListServerToolsResult>("mcp_list_server_tools_cached", {
            input: { serverId: server.id },
          }),
        ),
      );
      for (let i = 0; i < enabledServers.length; i++) {
        const target = enabledServers[i];
        const result = results[i];
        if (result.status !== "fulfilled") continue;
        target.toolItems = result.value.tools;
        target.lastElapsedMs = result.value.elapsedMs;
      }
    }
    setStatus(t('config.mcp.loadedCount', { count: servers.value.length }));
  } catch (error) {
    setStatus(`${t('config.mcp.loadFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

function addServer() {
  const seed = Date.now();
  const next: McpServerView = {
    id: `mcp-${seed}`,
    name: `MCP ${servers.value.length + 1}`,
    enabled: false,
    definitionJson: '{\n  "transport": "stdio",\n  "command": "npx",\n  "args": ["-y", "@upstash/context7-mcp"]\n}',
    toolPolicies: [],
    cachedTools: [],
    lastStatus: "",
    lastError: "",
    updatedAt: "",
    toolItems: [],
    lastElapsedMs: 0,
    isDraft: true,
    isDirty: true,
  };
  servers.value.unshift(next);
  selectedServerId.value = next.id;
}

async function removeServer(serverId: string) {
  loading.value = true;
  try {
    await invokeTauri<boolean>("mcp_remove_server", {
      input: { serverId },
    });
    servers.value = servers.value.filter((s) => s.id !== serverId);
    ensureSelectedServer();
    setStatus(t('config.mcp.deleted', { id: serverId }));
  } catch (error) {
    setStatus(`${t('config.mcp.deleteFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function validateDefinition(server: McpServerView) {
  loading.value = true;
  try {
    const result = await invokeTauri<McpDefinitionValidateResult>("mcp_validate_definition", {
      input: { definitionJson: server.definitionJson },
    });
    if (!result.ok) {
      const detailText = Array.isArray(result.details) && result.details.length > 0
        ? ` | ${result.details.join(" ; ")}`
        : "";
      const codeText = result.errorCode ? ` [${result.errorCode}]` : "";
      setStatus(`${t('config.mcp.validateFailed')}${codeText}: ${result.message}${detailText}`, true);
      return;
    }
    if (result.migratedDefinitionJson) {
      server.definitionJson = result.migratedDefinitionJson;
    }
    setStatus(`${t('config.mcp.validateSuccess')}: ${t('config.mcp.transport', { transport: result.transport || "-" })}`);
  } catch (error) {
    setStatus(`${t('config.mcp.validateFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function toggleDeploy(server: McpServerView) {
  loading.value = true;
  try {
    if (server.enabled) {
      const updated = await invokeTauri<McpServerConfig>("mcp_undeploy_server", {
        input: { serverId: server.id },
      });
      upsertServer({ ...server, ...updated });
      setStatus(`${t('config.mcp.stopped')}: ${server.name}`);
      return;
    }

    const savedBeforeDeploy = await _saveServerCore(server);
    upsertServer({ ...server, ...savedBeforeDeploy });
    const deployResult = await invokeTauri<McpListServerToolsResult>("mcp_deploy_server", {
      input: { serverId: server.id },
    });
    const saved = await invokeTauri<McpServerConfig[]>("mcp_list_servers");
    const latest = saved.find((s) => s.id === server.id);
    if (latest) {
      upsertServer({
        ...server,
        ...latest,
        toolItems: deployResult.tools,
        lastElapsedMs: deployResult.elapsedMs,
      });
    }
    setStatus(`${t('config.mcp.deploySuccess')}: ${server.name}（tools=${deployResult.tools.length}）`);
  } catch (error) {
    setStatus(`${t('config.mcp.deployFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function _saveServerCore(server: McpServerView): Promise<McpServerConfig> {
  return invokeTauri<McpServerConfig>("mcp_save_server", {
    input: {
      id: server.id,
      name: server.name,
      enabled: server.enabled,
      definitionJson: server.definitionJson,
    },
  });
}

async function onToggleTool(payload: { serverId: string; toolName: string; enabled: boolean }) {
  loading.value = true;
  try {
    await invokeTauri<McpServerConfig>("mcp_set_tool_enabled", {
      input: payload,
    });
    const server = servers.value.find((s) => s.id === payload.serverId);
    if (server) {
      const tool = server.toolItems.find((t) => t.toolName === payload.toolName);
      if (tool) {
        tool.enabled = payload.enabled;
      }
    }
    setStatus(`${payload.enabled ? t('config.mcp.toolEnabled') : t('config.mcp.toolDisabled')}: ${payload.toolName}`);
  } catch (error) {
    setStatus(`${t('config.mcp.toolSwitchFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function refreshTools(serverId: string) {
  loading.value = true;
  try {
    const result = await invokeTauri<McpListServerToolsResult>("mcp_list_server_tools_cached", {
      input: { serverId },
    });
    const server = servers.value.find((s) => s.id === serverId);
    if (server) {
      server.toolItems = result.tools;
      server.lastElapsedMs = result.elapsedMs;
    }
    setStatus(t('config.mcp.loadedCount', { count: servers.value.length }));
  } catch (error) {
    setStatus(`${t('config.mcp.loadFailed')}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function openMcpDir() {
  if (loading.value) return;
  loading.value = true;
  try {
    const opened = await invokeTauri<string>("mcp_open_workspace_dir");
    setStatus(t("config.mcp.openDirOpened", { path: opened }));
  } catch (error) {
    setStatus(t("config.mcp.openDirFailed", { err: toErrorMessage(error) }), true);
  } finally {
    loading.value = false;
  }
}

void reloadServers();
</script>
