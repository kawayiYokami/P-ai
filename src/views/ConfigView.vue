<template>
  <div class="grid gap-2">
    <div class="tabs tabs-boxed tabs-sm">
      <a class="tab" :class="{ 'tab-active': configTab === 'hotkey' }" @click="$emit('update:configTab', 'hotkey')">快捷键</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'api' }" @click="$emit('update:configTab', 'api')">API</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'tools' }" @click="$emit('update:configTab', 'tools')">工具</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'agent' }" @click="$emit('update:configTab', 'agent')">智能体</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'chatSettings' }" @click="$emit('update:configTab', 'chatSettings')">对话</a>
    </div>

    <template v-if="configTab === 'hotkey'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">Hotkey</span></div>
        <input v-model="config.hotkey" class="input input-bordered input-sm" placeholder="Alt+·" />
      </label>
      <div class="form-control">
        <div class="label py-1"><span class="label-text text-xs">主题</span></div>
        <button class="btn btn-sm btn-ghost bg-base-100 w-full flex items-center justify-center gap-2" @click="$emit('toggleTheme')">
          <Sun v-if="currentTheme === 'light'" class="h-4 w-4" />
          <Moon v-else class="h-4 w-4" />
          <span>{{ currentTheme === "light" ? "浅色模式" : "深色模式" }}</span>
        </button>
      </div>
    </template>

    <template v-else-if="configTab === 'api'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">当前API配置</span></div>
        <div class="flex gap-1">
          <select v-model="config.selectedApiConfigId" class="select select-bordered select-sm flex-1">
            <option v-for="a in config.apiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
          </select>
          <button class="btn btn-sm btn-square btn-ghost bg-base-100" title="新增API配置" @click="$emit('addApiConfig')">
            <Plus class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-square btn-ghost bg-base-100"
            title="删除当前API配置"
            :disabled="config.apiConfigs.length <= 1"
            @click="$emit('removeSelectedApiConfig')"
          >
            <Trash2 class="h-3.5 w-3.5" />
          </button>
        </div>
      </label>

      <div v-if="selectedApiConfig" class="grid gap-2">
        <input v-model="selectedApiConfig.name" class="input input-bordered input-sm" placeholder="配置名称" />
        <select v-model="selectedApiConfig.requestFormat" class="select select-bordered select-sm">
          <option value="openai">openai</option>
          <option value="openai_tts">openai_tts</option>
          <option value="gemini">gemini</option>
          <option value="deepseek/kimi">deepseek/kimi</option>
        </select>
        <input v-model="selectedApiConfig.baseUrl" class="input input-bordered input-sm" :placeholder="baseUrlReference" />
        <input v-model="selectedApiConfig.apiKey" type="password" class="input input-bordered input-sm" placeholder="api key" />
        <div class="flex gap-1">
          <input v-model="selectedApiConfig.model" class="input input-bordered input-sm flex-1" placeholder="model" />
          <div class="dropdown dropdown-end">
            <button tabindex="0" class="btn btn-sm btn-square btn-ghost bg-base-100" :disabled="modelOptions.length === 0" title="选择模型">
              <ChevronsUpDown class="h-3.5 w-3.5" />
            </button>
            <ul tabindex="0" class="dropdown-content z-[1] menu p-1 shadow bg-base-100 rounded-box w-52 max-h-56 overflow-auto">
              <li v-for="modelName in modelOptions" :key="modelName">
                <button @click="selectedApiConfig.model = modelName">{{ modelName }}</button>
              </li>
            </ul>
          </div>
          <button
            class="btn btn-sm btn-square btn-ghost bg-base-100"
            :class="{ loading: refreshingModels }"
            :disabled="refreshingModels"
            title="刷新模型列表"
            @click="$emit('refreshModels')"
          >
            <RefreshCw class="h-3.5 w-3.5" />
          </button>
        </div>
        <div class="flex gap-2">
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">文本</span><input v-model="selectedApiConfig.enableText" type="checkbox" class="toggle toggle-sm" /></label>
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">图片</span><input v-model="selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">语音</span><input v-model="selectedApiConfig.enableAudio" type="checkbox" class="toggle toggle-sm" /></label>
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">工具调用</span><input v-model="selectedApiConfig.enableTools" type="checkbox" class="toggle toggle-sm" /></label>
        </div>
      </div>

    </template>

    <template v-else-if="configTab === 'tools'">
      <div v-if="!selectedApiConfig" class="text-xs opacity-70">未选择 API 配置</div>
      <template v-else>
        <div class="flex items-center justify-between">
          <div class="text-xs font-medium">当前 API 配置：{{ selectedApiConfig.name }}</div>
          <button class="btn btn-sm btn-ghost bg-base-100" :class="{ loading: checkingToolsStatus }" @click="$emit('refreshToolsStatus')">
            刷新状态
          </button>
        </div>
        <div v-if="!selectedApiConfig.enableTools" class="text-xs opacity-70">此 API 配置未启用工具调用。</div>
        <div v-else class="grid gap-2">
          <div
            v-for="tool in selectedApiConfig.tools"
            :key="tool.id"
            class="rounded border border-base-300 bg-base-100 p-2"
          >
            <div class="flex items-center justify-between gap-2">
              <div class="text-xs font-medium">{{ tool.id }}</div>
              <div
                class="badge badge-xs"
                :class="statusBadgeClass(tool.id)"
              >
                {{ statusText(tool.id) }}
              </div>
            </div>
            <div class="text-[11px] opacity-80 mt-1">{{ tool.command }} {{ tool.args.join(" ") }}</div>
            <div class="text-[11px] opacity-70 mt-1">{{ statusDetail(tool.id) }}</div>
          </div>
        </div>
      </template>
    </template>

    <template v-else-if="configTab === 'agent'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">当前智能体</span></div>
        <select :value="selectedAgentId" class="select select-bordered select-sm" @change="$emit('update:selectedAgentId', ($event.target as HTMLSelectElement).value)">
          <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <div v-if="selectedAgent" class="grid gap-2">
        <input v-model="selectedAgent.name" class="input input-bordered input-sm" placeholder="智能体名称" />
        <textarea v-model="selectedAgent.systemPrompt" class="textarea textarea-bordered textarea-sm" rows="4" placeholder="系统提示词"></textarea>
      </div>
      <div class="flex gap-1">
        <button class="btn btn-sm" @click="$emit('addAgent')">新增</button>
        <button class="btn btn-sm" :disabled="agents.length <= 1" @click="$emit('removeSelectedAgent')">删除</button>
      </div>
    </template>

    <template v-else-if="configTab === 'chatSettings'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">对话AI</span></div>
        <select v-model="config.chatApiConfigId" class="select select-bordered select-sm">
          <option v-for="a in textCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">音转文AI（可选）</span></div>
        <select
          :value="config.sttApiConfigId ?? ''"
          class="select select-bordered select-sm"
          @change="config.sttApiConfigId = (($event.target as HTMLSelectElement).value || undefined)"
        >
          <option value="">不配置</option>
          <option v-for="a in audioCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">图转文AI（可选）</span></div>
        <select
          :value="config.visionApiConfigId ?? ''"
          class="select select-bordered select-sm"
          @change="config.visionApiConfigId = (($event.target as HTMLSelectElement).value || undefined)"
        >
          <option value="">不配置</option>
          <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">默认智能体</span></div>
        <select :value="selectedAgentId" class="select select-bordered select-sm" @change="$emit('update:selectedAgentId', ($event.target as HTMLSelectElement).value)">
          <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">用户称谓</span></div>
        <input :value="userAlias" class="input input-bordered input-sm" placeholder="用户" @input="$emit('update:userAlias', ($event.target as HTMLInputElement).value)" />
      </label>
      <div class="flex gap-1">
        <button class="btn btn-sm" @click="$emit('openCurrentHistory')">查看当前未归档记录</button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { AgentProfile, ApiConfigItem, AppConfig, ToolLoadStatus } from "../types/app";
import { ChevronsUpDown, Moon, Plus, RefreshCw, Sun, Trash2 } from "lucide-vue-next";

type ConfigTab = "hotkey" | "api" | "tools" | "agent" | "chatSettings";

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  currentTheme: "light" | "forest";
  selectedApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  checkingToolsStatus: boolean;
  modelOptions: string[];
  toolStatuses: ToolLoadStatus[];
  agents: AgentProfile[];
  selectedAgentId: string;
  selectedAgent: AgentProfile | null;
  userAlias: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  audioCapableApiConfigs: ApiConfigItem[];
}>();

defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:selectedAgentId", value: string): void;
  (e: "update:userAlias", value: string): void;
  (e: "toggleTheme"): void;
  (e: "refreshModels"): void;
  (e: "refreshToolsStatus"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "addAgent"): void;
  (e: "removeSelectedAgent"): void;
  (e: "openCurrentHistory"): void;
}>();

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? "unknown";
}

function statusDetail(id: string): string {
  return toolStatusById(id)?.detail ?? "尚未检查";
}

function statusBadgeClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "loaded") return "badge-success";
  if (status === "failed" || status === "timeout") return "badge-error";
  if (status === "disabled") return "badge-ghost";
  return "badge-outline";
}
</script>
