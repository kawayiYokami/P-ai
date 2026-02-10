<template>
  <div class="grid gap-2">
    <div class="tabs tabs-boxed tabs-sm">
      <a class="tab" :class="{ 'tab-active': configTab === 'hotkey' }" @click="$emit('update:configTab', 'hotkey')">快捷键</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'api' }" @click="$emit('update:configTab', 'api')">API配置</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'agent' }" @click="$emit('update:configTab', 'agent')">智能体</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'chatSettings' }" @click="$emit('update:configTab', 'chatSettings')">对话设置</a>
    </div>

    <template v-if="configTab === 'hotkey'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">Hotkey</span></div>
        <input v-model="config.hotkey" class="input input-bordered input-sm" placeholder="Alt+C" />
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">主题</span></div>
        <button class="btn btn-sm w-full" @click="$emit('toggleTheme')">
          {{ currentTheme === "light" ? "🌞 浅色模式" : "🌙 深色模式" }}
        </button>
      </label>
      <button class="btn btn-sm" :class="{ loading: loading }" @click="$emit('loadConfig')">重载</button>
    </template>

    <template v-else-if="configTab === 'api'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">当前API配置</span></div>
        <select v-model="config.selectedApiConfigId" class="select select-bordered select-sm">
          <option v-for="a in config.apiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>

      <div v-if="selectedApiConfig" class="grid gap-2">
        <input v-model="selectedApiConfig.name" class="input input-bordered input-sm" placeholder="配置名称" />
        <select v-model="selectedApiConfig.requestFormat" class="select select-bordered select-sm">
          <option value="openai">openai</option>
          <option value="gemini">gemini</option>
          <option value="deepseek/kimi">deepseek/kimi</option>
        </select>
        <input v-model="selectedApiConfig.baseUrl" class="input input-bordered input-sm" :placeholder="baseUrlReference" />
        <input v-model="selectedApiConfig.apiKey" type="password" class="input input-bordered input-sm" placeholder="api key" />
        <div class="flex gap-1">
          <input v-model="selectedApiConfig.model" class="input input-bordered input-sm flex-1" placeholder="model" />
          <button class="btn btn-sm btn-square" :class="{ loading: refreshingModels }" :disabled="refreshingModels" @click="$emit('refreshModels')">刷新</button>
        </div>
        <div class="flex gap-2">
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">文本</span><input v-model="selectedApiConfig.enableText" type="checkbox" class="toggle toggle-sm" /></label>
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">图片</span><input v-model="selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
          <label class="label cursor-pointer gap-1"><span class="label-text text-xs">语音</span><input v-model="selectedApiConfig.enableAudio" type="checkbox" class="toggle toggle-sm" /></label>
        </div>
      </div>

      <div class="flex gap-1">
        <button class="btn btn-sm" @click="$emit('addApiConfig')">新增</button>
        <button class="btn btn-sm" :disabled="config.apiConfigs.length <= 1" @click="$emit('removeSelectedApiConfig')">删除</button>
        <button class="btn btn-sm" :class="{ loading: loading }" @click="$emit('loadConfig')">重载</button>
      </div>
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

    <template v-else>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">默认AI配置</span></div>
        <select v-model="config.selectedApiConfigId" class="select select-bordered select-sm">
          <option v-for="a in config.apiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
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
import type { AgentProfile, ApiConfigItem, AppConfig } from "../types/app";

type ConfigTab = "hotkey" | "api" | "agent" | "chatSettings";

defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  currentTheme: "light" | "forest";
  selectedApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  loading: boolean;
  agents: AgentProfile[];
  selectedAgentId: string;
  selectedAgent: AgentProfile | null;
  userAlias: string;
}>();

defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:selectedAgentId", value: string): void;
  (e: "update:userAlias", value: string): void;
  (e: "toggleTheme"): void;
  (e: "loadConfig"): void;
  (e: "refreshModels"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "addAgent"): void;
  (e: "removeSelectedAgent"): void;
  (e: "openCurrentHistory"): void;
}>();
</script>
