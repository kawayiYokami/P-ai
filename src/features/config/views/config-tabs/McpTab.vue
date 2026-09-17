<template>
  <SettingsPageShell :breadcrumb="mcpBreadcrumb" header-class="">
    <template #left>
      <div v-if="!(inDetailMode && selectedServer)" class="relative w-full min-w-0 sm:w-60 sm:min-w-60 sm:flex-none">
        <input
          v-model="searchQuery"
          type="text"
          class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
          :placeholder="t('config.mcp.searchPlaceholder')"
        />
        <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
        <button
          v-if="searchQuery"
          type="button"
          class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
          :title="t('config.mcp.clearSearch')"
          @click="searchQuery = ''"
        >
          ✕
        </button>
      </div>
    </template>

    <template #actions>
      <template v-if="inDetailMode && selectedServer">
        <div class="flex flex-wrap items-center gap-2">
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="validateDefinition(selectedServer)"
            >
              <CheckCircle class="h-4 w-4" />
              <span>{{ t('config.mcpServerCard.validate') }}</span>
            </button>
            <button
              v-if="issueList.length > 0"
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-warning gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="fixDefinition(selectedServer)"
            >
              <Wrench class="h-4 w-4" />
              <span>{{ t('config.mcp.fixFormat') }}</span>
            </button>
            <button
              v-if="selectedServer.isDirty"
              class="btn btn-sm min-h-[2.25rem] btn-primary gap-1.5 px-3.5"
              type="button"
              :disabled="loading"
              @click="saveServer(selectedServer)"
            >
              <Save class="h-4 w-4" />
              <span>{{ t('common.save') }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3"
              :class="selectedServer.enabled ? 'btn-warning' : 'btn-success'"
              type="button"
              :disabled="loading"
              @click="toggleDeploy(selectedServer)"
            >
              <Power class="h-4 w-4" />
              <span>{{ selectedServer.enabled ? t('config.mcp.stop') : t('config.mcp.deploy') }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-error gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="confirmRemoveServer(selectedServer)"
            >
              <Trash2 class="h-4 w-4" />
              <span>{{ t('config.mcpServerCard.delete') }}</span>
            </button>
          </div>
      </template>

      <template v-else>
        <div class="flex flex-wrap items-center gap-2">
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="reloadServers"
            >
              <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
              <span>{{ t('config.mcp.refresh') }}</span>
            </button>
            <button
              v-if="localFileSystemAvailable"
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="openMcpDir"
            >
              <FolderOpen class="h-4 w-4" />
              <span>{{ t('config.mcp.openDir') }}</span>
            </button>
          </div>
      </template>
    </template>

    <div class="space-y-4">
      <!-- 缺失 Node.js 提示条 -->
      <div
        v-if="nodeMissing && !nodeInstalling"
        class="card card-border border-warning/40 bg-warning/10 card-sm"
      >
        <div class="card-body flex-row flex-wrap items-center gap-2 px-4 py-3">
          <div class="flex flex-col gap-0.5">
            <span class="text-sm font-medium">{{ t('config.mcp.nodeRequired') }}</span>
            <span class="text-xs opacity-70">{{ t('config.mcp.nodeRequiredHint') }}</span>
          </div>
          <div class="flex-1" />
          <span v-if="nodeInstallError" class="text-xs text-error max-w-56 text-right">{{ nodeInstallError }}</span>
          <button class="btn btn-sm btn-warning min-h-[2rem]" type="button" @click="installNode">
            {{ t('config.mcp.installNode') }}
          </button>
        </div>
      </div>
      <div v-if="nodeInstalling" class="text-sm opacity-70">{{ t('config.mcp.installingNode') }}</div>

      <!-- 校验问题列表 -->
      <div v-if="issueList.length > 0" class="card card-border border-error/40 bg-error/10 card-sm">
        <div class="card-body p-3.5 space-y-1">
          <div v-for="(issue, idx) in issueList" :key="idx" class="flex items-start gap-2 text-xs text-error">
            <span class="mt-0.5">•</span>
            <span>{{ issue }}</span>
          </div>
        </div>
      </div>

      <!-- 状态反馈统一走界面底部的状态提示条 -->
      <Transition name="ecall-config-content" mode="out-in">
      <!-- 二级详情视图 -->
      <div v-if="inDetailMode && selectedServer" key="detail">
        <McpServerCard
          :key="selectedServer.id"
          :server="selectedServer"
          :disabled="loading"
          :has-issues="issueList.length > 0"
          @change="onServerChange"
          @remove="removeServer"
          @validate="validateDefinition"
          @fix="fixDefinition"
          @toggle-deploy="toggleDeploy"
          @toggle-tool="onToggleTool"
          @refresh-tools="refreshTools"
        />
      </div>

      <!-- 一级卡片矩阵总览视图 -->
      <div v-else key="overview" class="space-y-3">
        <div v-if="loading && servers.length === 0" class="text-sm opacity-70 py-8 text-center">
          {{ t('config.mcp.loading') }}
        </div>

        <div
          v-else-if="servers.length > 0 && filteredServers.length === 0"
          class="card card-border border-base-300 bg-base-100 p-8 text-center"
        >
          <div class="text-xs opacity-60">{{ t('config.mcp.noMatches') }}</div>
        </div>

        <div v-else class="config-grid-auto-md">
          <div
            v-for="server in filteredServers"
            :key="server.id"
            role="button"
            tabindex="0"
            class="rounded-box border border-base-300 bg-base-100 p-4 hover:border-primary/50 transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] group"
            :class="{ 'opacity-65 bg-base-100/60': !server.enabled }"
            @click="enterServer(server.id)"
            @keydown.enter.prevent="enterServer(server.id)"
            @keydown.space.prevent="enterServer(server.id)"
          >
            <!-- 头部：名称/命令 + 启用开关 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="card-title-bar text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ server.name || server.id }}
                  </span>
                  <span v-if="server.isDirty" class="badge badge-warning badge-xs shrink-0">
                    {{ t('config.mcp.unsaved') }}
                  </span>
                </div>
                <div v-if="getServerSubtitle(server)" class="font-mono text-caption opacity-50 truncate mt-0.5">
                  {{ getServerSubtitle(server) }}
                </div>
              </div>

              <div @click.stop>
                <input
                  type="checkbox"
                  :checked="server.enabled"
                  class="toggle toggle-sm toggle-success shrink-0"
                  :disabled="loading"
                  :title="server.enabled ? t('config.mcp.stop') : t('config.mcp.deploy')"
                  @change="toggleDeploy(server)"
                />
              </div>
            </div>

            <!-- 多成员聚合提示（仅当存在 >1 个不同子服务时展示，单体服务不占位不复读） -->
            <div v-if="getMultiMemberSummary(server.definitionJson)" class="text-caption opacity-60 truncate font-mono">
              {{ getMultiMemberSummary(server.definitionJson) }}
            </div>

            <!-- 底栏：启用时显示状态与工具数；未启用时左侧留空，右上角 Toggle 已经自明 -->
            <div class="flex items-center justify-between border-t border-base-300 pt-2.5 text-caption">
              <div class="flex items-center gap-1.5 min-h-[1.5rem]">
                <template v-if="server.enabled">
                  <span class="badge badge-sm" :class="getStatusBadgeClass(server.lastStatus)">
                    {{ getStatusLabel(server.lastStatus) }}
                  </span>
                  <span class="badge badge-sm badge-ghost">
                    {{ server.toolItems.length > 0 ? t('config.mcp.toolCount', { count: server.toolItems.length }) : t('config.mcp.probingTools') }}
                  </span>
                </template>
              </div>
              <div class="flex items-center gap-1.5 opacity-70">
                <span v-if="getProtocolBadge(server.definitionJson)" class="font-mono text-caption opacity-60">
                  {{ getProtocolBadge(server.definitionJson) }}
                </span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>

          <!-- 新增连接器卡：网格末位入口 -->
          <button
            type="button"
            class="flex min-h-[7.5rem] flex-col items-center justify-center gap-1.5 rounded-box border border-dashed border-base-300 bg-base-100 p-3.5 text-base-content/50 transition-all hover:border-primary/50 hover:text-primary sm:p-4"
            @click="addServer"
          >
            <Plus class="h-5 w-5" />
            <span class="text-sm font-medium">{{ t('config.mcp.add') }}</span>
          </button>
        </div>
      </div>
      </Transition>
    </div>
  </SettingsPageShell>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  CheckCircle,
  ChevronRight,
  FolderOpen,
  Plus,
  Power,
  RefreshCw,
  Save,
  Search,
  Settings,
  Trash2,
  Wrench,
} from "@lucide/vue";
import {
  getTransportCapabilities,
  getTransportHostRuntimePrerequisites,
  installTransportHostRuntimePrerequisite,
  invokeTauri,
  openTransportMcpWorkspaceDirectory,
} from "../../../../services/tauri-api";
import type {
  McpDefinitionValidateResult,
  McpFixDefinitionResult,
  McpListServerToolsResult,
  McpServerConfig,
  McpToolDescriptor,
  McpValidationIssue,
} from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";
import { formatEndpointDisplay } from "../../utils/api-config-display";
import McpServerCard from "./mcp/McpServerCard.vue";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbItem } from "../../components/SettingsBreadcrumb.vue";
import type { StatusTone } from "../../../shell/composables/use-app-core";

const props = withDefaults(defineProps<{
  setStatusAction?: (text: string, tone?: StatusTone) => void;
}>(), {
  setStatusAction: undefined,
});

const { t, te } = useI18n();

type McpServerView = McpServerConfig & {
  toolItems: McpToolDescriptor[];
  lastElapsedMs: number;
  isDraft: boolean;
  isDirty: boolean;
};

const loading = ref(false);
const inDetailMode = ref(false);
const searchQuery = ref("");
const servers = ref<McpServerView[]>([]);
const selectedServerId = ref("");
const localFileSystemAvailable = getTransportCapabilities().localFileSystem;

const nodeMissing = ref(false);
const nodeInstalling = ref(false);
const nodeInstallError = ref("");

const issueList = ref<string[]>([]);

const selectedServer = computed(() =>
  servers.value.find((s) => s.id === selectedServerId.value) ?? null,
);

const filteredServers = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return servers.value;
  return servers.value.filter((s) => {
    const nameMatch = (s.name || "").toLowerCase().includes(query);
    const idMatch = (s.id || "").toLowerCase().includes(query);
    const memberMatch = parseMemberNames(s.definitionJson).some((m) =>
      m.toLowerCase().includes(query),
    );
    return nameMatch || idMatch || memberMatch;
  });
});

function setStatus(text: string, isError = false) {
  props.setStatusAction?.(text, isError ? "error" : "default");
}

function enterServer(id: string) {
  selectedServerId.value = id;
  inDetailMode.value = true;
  clearIssues();
}

const mcpBreadcrumb = computed<SettingsBreadcrumbItem[]>(() => {
  const server = selectedServer.value;
  if (!inDetailMode.value || !server) return [{ label: t("config.tabs.mcp") }];
  return [
    { label: t("config.tabs.mcp"), title: t("config.mcp.backToList"), onClick: backToList },
    {
      label: server.name || server.id,
      badge: server.isDirty ? t("config.mcp.unsaved") : undefined,
    },
  ];
});

function backToList() {
  inDetailMode.value = false;
  clearIssues();
}

function getStatusBadgeClass(status?: string): string {
  if (status === "ready" || status === "deployed") return "badge-success";
  if (status === "starting" || status === "deploying") return "badge-warning";
  if (status === "stale") return "badge-warning";
  if (status === "timeout" || status === "failed") return "badge-error";
  if (status === "stopped" || status === "disabled") return "badge-neutral";
  return "badge-ghost";
}

function getStatusLabel(status?: string): string {
  if (status === "ready" || status === "deployed") return t("config.mcp.statusReady");
  if (status === "stopped") return t("config.mcp.statusStopped");
  if (status === "starting" || status === "deploying") return t("config.mcp.statusStarting");
  if (status === "stale") return t("config.mcp.statusStale");
  if (status === "timeout") return t("config.mcp.statusTimeout");
  if (status === "disabled") return t("config.mcp.statusDisabled");
  if (status === "failed") return t("config.mcp.statusFailed");
  return status || t("config.mcp.statusUnknown");
}

interface McpServerEntry {
  name: string;
  command?: string;
  args?: string[];
  url?: string;
  transport?: string;
  type?: string;
}

/**
 * 从 definitionJson 中解析出所有有效的 server 配置条目（支持多格式展开）
 * 1. { "mcpServers": { "<name>": { ... } } } （最主流标准格式）
 * 2. { "mcpServers": [ { "name": "...", ... } ] }
 * 3. 根对象为直接字段单服务：{ "url": "...", "type": "streamable-http" } 或 { "command": "..." }
 * 4. 根对象为命名集合：{ "<name>": { ... } }
 * 5. 根对象为数组：[ { "name": "...", ... } ]
 */
function parseMcpServerEntries(definitionJson: string): McpServerEntry[] {
  try {
    const parsed = JSON.parse(definitionJson) as unknown;
    if (!parsed || typeof parsed !== "object") return [];

    // 格式 5：根级数组
    if (Array.isArray(parsed)) {
      return parsed
        .filter((item): item is Record<string, unknown> => !!item && typeof item === "object")
        .map((item, idx) => ({
          name: String(item.name || `server-${idx + 1}`),
          command: item.command ? String(item.command) : undefined,
          args: Array.isArray(item.args) ? item.args.map(String) : undefined,
          url: item.url ? String(item.url) : undefined,
          transport: item.transport ? String(item.transport) : undefined,
          type: item.type ? String(item.type) : undefined,
        }));
    }

    const root = parsed as Record<string, unknown>;

    // 格式 1 & 2：包含 mcpServers
    if (root.mcpServers && typeof root.mcpServers === "object") {
      const ms = root.mcpServers;
      if (Array.isArray(ms)) {
        return ms
          .filter((item): item is Record<string, unknown> => !!item && typeof item === "object")
          .map((item, idx) => ({
            name: String(item.name || `server-${idx + 1}`),
            command: item.command ? String(item.command) : undefined,
            args: Array.isArray(item.args) ? item.args.map(String) : undefined,
            url: item.url ? String(item.url) : undefined,
            transport: item.transport ? String(item.transport) : undefined,
            type: item.type ? String(item.type) : undefined,
          }));
      }
      return Object.entries(ms as Record<string, unknown>)
        .filter((entry): entry is [string, Record<string, unknown>] => !!entry[1] && typeof entry[1] === "object")
        .map(([name, item]) => ({
          name,
          command: item.command ? String(item.command) : undefined,
          args: Array.isArray(item.args) ? item.args.map(String) : undefined,
          url: item.url ? String(item.url) : undefined,
          transport: item.transport ? String(item.transport) : undefined,
          type: item.type ? String(item.type) : undefined,
        }));
    }

    // 格式 3：单服务直接字段
    const directKeys = ["command", "args", "url", "transport", "type", "env", "cwd"];
    if (directKeys.some((k) => k in root)) {
      return [{
        name: String(root.name || ""),
        command: root.command ? String(root.command) : undefined,
        args: Array.isArray(root.args) ? root.args.map(String) : undefined,
        url: root.url ? String(root.url) : undefined,
        transport: root.transport ? String(root.transport) : undefined,
        type: root.type ? String(root.type) : undefined,
      }];
    }

    // 格式 4：平铺命名集合
    const entries = Object.entries(root)
      .filter((entry): entry is [string, Record<string, unknown>] => !!entry[1] && typeof entry[1] === "object");
    if (entries.length > 0) {
      return entries.map(([name, item]) => ({
        name,
        command: item.command ? String(item.command) : undefined,
        args: Array.isArray(item.args) ? item.args.map(String) : undefined,
        url: item.url ? String(item.url) : undefined,
        transport: item.transport ? String(item.transport) : undefined,
        type: item.type ? String(item.type) : undefined,
      }));
    }
  } catch {
    // ignore
  }
  return [];
}

/**
 * 解析 MCP 运行协议/执行器标签（第一性原理精准识别）
 * 1. 网络服务：
 *    - sse -> "sse"
 *    - streamable_http / streamable-http / http / url 端点 -> "流式 http"
 * 2. 本地子进程 (stdio)：
 *    - uvx / uv -> "uvx" / "uv"
 *    - npx / npm / bunx / bun / pnpm -> "npx" / "bunx" 等
 *    - node -> "node"
 *    - python / python3 / py -> "python"
 *    - docker -> "docker"
 *    - 其他本地命令 -> "stdio"
 */
function getProtocolBadge(definitionJson: string): string {
  const entries = parseMcpServerEntries(definitionJson);
  if (entries.length === 0) return "";

  const first = entries[0];
  const transport = String(first.transport ?? first.type ?? "").toLowerCase();

  // 1. 网络协议判断
  if (transport === "sse") return "sse";
  if (
    transport === "streamable_http" ||
    transport === "streamable-http" ||
    transport === "http" ||
    transport === "https" ||
    first.url
  ) {
    return "流式 http";
  }

  // 2. 本地执行器/环境判断
  if (first.command) {
    const rawCmd = first.command.trim().toLowerCase();
    const baseName = rawCmd.split(/[/\\]/).pop()?.replace(/\.(exe|cmd|bat|ps1)$/, "") || "";
    const args = (first.args ?? []).map((a) => a.toLowerCase());

    if (baseName === "uvx") return "uvx";
    if (baseName === "uv") {
      return args.includes("run") ? "uv run" : "uv";
    }
    if (baseName === "npx") return "npx";
    if (baseName === "node") return "node";
    if (baseName === "bunx" || baseName === "bun") return baseName;
    if (baseName === "pnpm" || baseName === "npm" || baseName === "yarn") return baseName;
    if (baseName === "docker") return "docker";
    if (baseName === "python" || baseName === "python3" || baseName === "py") return "python";

    return "stdio";
  }

  return "";
}

/** 获取卡片副标题（优先展示命令/URL；若无且名称与 ID 相同或为时间戳 ID，则不展示无意义复读） */
function getServerSubtitle(server: McpServerConfig): string {
  const entries = parseMcpServerEntries(server.definitionJson);
  if (entries.length > 0) {
    const first = entries[0];
    if (first.command) {
      const cmd = first.command;
      const args = first.args ? first.args.join(" ") : "";
      const full = args ? `${cmd} ${args}` : cmd;
      return full.length > 50 ? full.slice(0, 47) + "..." : full;
    }
    if (first.url) {
      const url = formatEndpointDisplay(first.url);
      return url.length > 50 ? url.slice(0, 47) + "..." : url;
    }
  }
  if (server.id && server.name && server.id !== server.name && !server.id.startsWith("mcp-")) {
    return server.id;
  }
  return "";
}

/** 从 definitionJson 解析组内成员名（用于跨卡片重名检测与概览展示） */
function parseMemberNames(definitionJson: string): string[] {
  return parseMcpServerEntries(definitionJson)
    .map((e) => e.name)
    .filter(Boolean);
}

/** 获取多成员聚合服务的概览（仅在包含 >1 个不同子服务时展示） */
function getMultiMemberSummary(definitionJson: string): string {
  const members = parseMemberNames(definitionJson);
  if (members.length > 1) {
    return `${members.length} 个子服务: ${members.join(", ")}`;
  }
  return "";
}

function issueText(issue: McpValidationIssue): string {
  const params: Record<string, string> = {
    serverName: issue.serverName ?? "",
    field: issue.field ?? "",
    index: issue.params?.index ?? "",
    message: issue.message,
  };
  const key = `config.mcp.issues.${issue.code}`;
  if (te(key)) {
    return t(key, params);
  }
  return t("config.mcp.issues.fallback", params);
}

function applyIssues(issues: McpValidationIssue[] | undefined) {
  issueList.value = (issues ?? []).map(issueText);
}

function clearIssues() {
  issueList.value = [];
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
    setStatus(t("config.mcp.loadedCount", { count: servers.value.length }));
  } catch (error) {
    setStatus(`${t("config.mcp.loadFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

function addServer() {
  const seed = Date.now();
  const next: McpServerView = {
    id: `mcp-${seed}`,
    name: `${t("config.tabs.mcp")} ${servers.value.length + 1}`,
    enabled: false,
    definitionJson:
      '{\n  "name": "mcp-server",\n  "transport": "stdio",\n  "command": "npx",\n  "args": ["-y", "@upstash/context7-mcp"]\n}',
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
  inDetailMode.value = true;
}

function onServerChange(updated: McpServerView) {
  const idx = servers.value.findIndex((s) => s.id === updated.id);
  if (idx >= 0) {
    servers.value[idx] = { ...servers.value[idx], ...updated, isDirty: true };
  }
}

async function saveServer(server: McpServerView) {
  loading.value = true;
  try {
    const saved = await _saveServerCore(server);
    upsertServer({ ...server, ...saved, isDirty: false });
    setStatus(t("config.mcp.saved"));
  } catch (error) {
    setStatus(`${t("config.mcp.saveFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

function confirmRemoveServer(server: McpServerView) {
  if (window.confirm(t("config.mcp.deleteConfirm", { name: server.name || server.id }))) {
    void removeServer(server.id);
  }
}

async function removeServer(serverId: string) {
  loading.value = true;
  try {
    await invokeTauri<boolean>("mcp_remove_server", {
      input: { serverId },
    });
    servers.value = servers.value.filter((s) => s.id !== serverId);
    if (selectedServerId.value === serverId) {
      inDetailMode.value = false;
    }
    ensureSelectedServer();
    setStatus(t("config.mcp.deleted", { id: serverId }));
  } catch (error) {
    setStatus(`${t("config.mcp.deleteFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function validateDefinition(server: McpServerView) {
  loading.value = true;
  clearIssues();
  try {
    const result = await invokeTauri<McpDefinitionValidateResult>("mcp_validate_definition", {
      input: {
        definitionJson: server.definitionJson,
        existingMemberNames: servers.value
          .filter((s) => s.id !== server.id)
          .flatMap((s) => parseMemberNames(s.definitionJson)),
      },
    });
    if (!result.ok) {
      applyIssues(result.issues);
      const detailText =
        result.issues && result.issues.length > 0
          ? ""
          : Array.isArray(result.details) && result.details.length > 0
            ? ` | ${result.details.join(" ; ")}`
            : "";
      const codeText = result.errorCode ? ` [${result.errorCode}]` : "";
      setStatus(
        `${t("config.mcp.validateFailed")}${codeText}: ${result.message}${detailText}`,
        true,
      );
      return;
    }
    const serverCountText = result.serverName
      ? ` (${result.serverName}${result.transport ? `, ${t("config.mcp.transport", { transport: result.transport })}` : ""})`
      : "";
    setStatus(`${t("config.mcp.validateSuccess")}${serverCountText}`);
  } catch (error) {
    setStatus(`${t("config.mcp.validateFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function fixDefinition(server: McpServerView) {
  loading.value = true;
  clearIssues();
  try {
    const result = await invokeTauri<McpFixDefinitionResult>("mcp_fix_definition", {
      input: { definitionJson: server.definitionJson },
    });
    if (result.fixedDefinitionJson) {
      server.definitionJson = result.fixedDefinitionJson;
      server.isDirty = true;
    }
    if (result.ok) {
      if (result.fixedDefinitionJson === server.definitionJson && result.issues.length === 0) {
        setStatus(t("config.mcp.fixNoNeed"));
      } else {
        applyIssues(result.issues);
        setStatus(
          `${t("config.mcp.fixSuccess")}${result.modelName ? `（${result.modelName}）` : ""}`,
        );
      }
      return;
    }
    applyIssues(result.issues);
    setStatus(
      `${t("config.mcp.fixStillIssues")}${result.modelName ? `（${result.modelName}）` : ""}: ${result.message}`,
      true,
    );
  } catch (error) {
    setStatus(`${t("config.mcp.fixFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function checkNodeInstalled(): Promise<boolean> {
  try {
    const prerequisites = await getTransportHostRuntimePrerequisites<{ nodeInstalled?: boolean }>();
    nodeMissing.value = prerequisites.nodeInstalled === false;
    return prerequisites.nodeInstalled === true;
  } catch {
    nodeMissing.value = false;
    return true;
  }
}

async function installNode() {
  if (nodeInstalling.value) return;
  nodeInstalling.value = true;
  nodeInstallError.value = "";
  try {
    await installTransportHostRuntimePrerequisite<{ installed: boolean; message: string }>("node");
    await checkNodeInstalled();
    if (!nodeMissing.value) {
      setStatus(t("config.mcp.nodeInstalled"));
    }
  } catch (error) {
    nodeInstallError.value = toErrorMessage(error);
  } finally {
    nodeInstalling.value = false;
  }
}

async function toggleDeploy(server: McpServerView) {
  loading.value = true;
  try {
    if (server.enabled) {
      const updated = await invokeTauri<McpServerConfig>("mcp_undeploy_server", {
        input: { serverId: server.id },
      });
      upsertServer({
        ...server,
        ...updated,
        toolItems: [],
        lastElapsedMs: 0,
      });
      setStatus(`${t("config.mcp.stopped")}: ${server.name}`);
      return;
    }

    const savedBeforeDeploy = await _saveServerCore(server);
    upsertServer({ ...server, ...savedBeforeDeploy, isDirty: false });
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
    if (deployResult.tools.length === 0) {
      setStatus(`${t("config.mcp.deploySuccess")}: ${server.name}（${t("config.mcp.probingTools")}）`);
      void pollServerTools(server.id);
    } else {
      setStatus(`${t("config.mcp.deploySuccess")}: ${server.name}（tools=${deployResult.tools.length}）`);
    }
  } catch (error) {
    setStatus(`${t("config.mcp.deployFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function pollServerTools(serverId: string) {
  for (let attempt = 0; attempt < 6; attempt++) {
    await new Promise((resolve) => setTimeout(resolve, 1500));
    try {
      const result = await invokeTauri<McpListServerToolsResult>("mcp_list_server_tools_cached", {
        input: { serverId },
      });
      const target = servers.value.find((s) => s.id === serverId);
      if (target) {
        target.toolItems = result.tools;
        target.lastElapsedMs = result.elapsedMs;
      }
      if (result.tools.length > 0) {
        setStatus(
          `${t("config.mcp.deploySuccess")}: ${target?.name ?? serverId}（tools=${result.tools.length}）`,
        );
        return;
      }
    } catch {
      return;
    }
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
    setStatus(
      `${payload.enabled ? t("config.mcp.toolEnabled") : t("config.mcp.toolDisabled")}: ${payload.toolName}`,
    );
  } catch (error) {
    setStatus(`${t("config.mcp.toolSwitchFailed")}: ${toErrorMessage(error)}`, true);
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
    setStatus(t("config.mcp.loadedCount", { count: servers.value.length }));
  } catch (error) {
    setStatus(`${t("config.mcp.loadFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function openMcpDir() {
  if (!localFileSystemAvailable || loading.value) return;
  loading.value = true;
  try {
    const opened = await openTransportMcpWorkspaceDirectory();
    setStatus(t("config.mcp.openDirOpened", { path: opened }));
  } catch (error) {
    setStatus(t("config.mcp.openDirFailed", { err: toErrorMessage(error) }), true);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void reloadServers();
  void checkNodeInstalled();
});
</script>
