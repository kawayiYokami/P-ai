<template>
  <div class="grid gap-2">
    <div class="tabs tabs-boxed tabs-sm">
      <a class="tab" :class="{ 'tab-active': configTab === 'hotkey' }" @click="$emit('update:configTab', 'hotkey')">快捷键</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'api' }" @click="$emit('update:configTab', 'api')">API</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'tools' }" @click="$emit('update:configTab', 'tools')">工具</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'persona' }" @click="$emit('update:configTab', 'persona')">人格</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'chatSettings' }" @click="$emit('update:configTab', 'chatSettings')">对话</a>
    </div>

    <template v-if="configTab === 'hotkey'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">呼唤热键</span></div>
        <input v-model="config.hotkey" class="input input-bordered input-sm" placeholder="Alt+·" />
      </label>
      <div class="grid grid-cols-3 gap-2">
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">录音键</span></div>
          <select v-model="config.recordHotkey" class="select select-bordered select-sm">
            <option value="Alt">Alt</option>
            <option value="Ctrl">Ctrl</option>
            <option value="Shift">Shift</option>
          </select>
        </label>
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">最短录音(s)</span></div>
          <input v-model.number="config.minRecordSeconds" type="number" min="1" max="30" class="input input-bordered input-sm" />
        </label>
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">最长录音(s)</span></div>
          <input v-model.number="config.maxRecordSeconds" type="number" min="1" max="600" class="input input-bordered input-sm" />
        </label>
      </div>
      <div class="form-control">
        <div class="label py-1"><span class="label-text text-xs">录音测试</span></div>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-sm btn-ghost bg-base-100"
            :class="{ 'btn-error text-error-content': hotkeyTestRecording }"
            :title="hotkeyTestRecording ? '松开结束录音' : '按住开始录音'"
            @mousedown.prevent="$emit('startHotkeyRecordTest')"
            @mouseup.prevent="$emit('stopHotkeyRecordTest')"
            @mouseleave.prevent="hotkeyTestRecording && $emit('stopHotkeyRecordTest')"
            @touchstart.prevent="$emit('startHotkeyRecordTest')"
            @touchend.prevent="$emit('stopHotkeyRecordTest')"
          >
            {{ hotkeyTestRecording ? `录音中 ${Math.max(1, Math.round(hotkeyTestRecordingMs / 1000))}s` : "按住录音" }}
          </button>
          <button
            class="btn btn-sm btn-ghost bg-base-100"
            :disabled="!hotkeyTestAudioReady"
            @click="$emit('playHotkeyRecordTest')"
          >
            播放录音
          </button>
        </div>
      </div>
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
      <button
        class="btn btn-primary btn-sm w-full"
        :disabled="!configDirty || savingConfig"
        @click="$emit('saveApiConfig')"
      >
        {{ savingConfig ? "保存中..." : configDirty ? "保存配置" : "已保存" }}
      </button>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-sm font-medium">API配置（编辑）</span></div>
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

      <div class="divider my-0"></div>

      <div v-if="selectedApiConfig" class="grid gap-2">
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">配置名称</span></div>
          <input v-model="selectedApiConfig.name" class="input input-bordered input-sm" placeholder="配置名称" />
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">请求格式</span></div>
          <select v-model="selectedApiConfig.requestFormat" class="select select-bordered select-sm">
            <option value="openai">openai</option>
            <option value="gemini">gemini</option>
            <option value="deepseek/kimi">deepseek/kimi</option>
          </select>
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">Base URL</span></div>
          <input v-model="selectedApiConfig.baseUrl" class="input input-bordered input-sm" :placeholder="baseUrlReference" />
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">API Key</span></div>
          <input v-model="selectedApiConfig.apiKey" type="password" class="input input-bordered input-sm" placeholder="api key" />
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">模型</span></div>
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
        </label>
        <div class="form-control">
          <div class="label py-1"><span class="label-text text-sm font-medium">能力开关</span></div>
          <div class="flex gap-2">
            <label class="label cursor-pointer gap-1"><span class="label-text text-xs">文本</span><input v-model="selectedApiConfig.enableText" type="checkbox" class="toggle toggle-sm" /></label>
            <label class="label cursor-pointer gap-1"><span class="label-text text-xs">图片</span><input v-model="selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
            <label class="label cursor-pointer gap-1"><span class="label-text text-xs">工具调用</span><input v-model="selectedApiConfig.enableTools" type="checkbox" class="toggle toggle-sm" /></label>
          </div>
        </div>
      </div>

    </template>

    <template v-else-if="configTab === 'tools'">
      <div v-if="!toolApiConfig" class="text-xs opacity-70">未配置对话AI</div>
      <template v-else>
        <div class="grid gap-2">
          <label class="form-control">
            <div class="label py-1"><span class="label-text text-xs">工具最大调用轮次</span></div>
            <input
              v-model.number="config.toolMaxIterations"
              type="number"
              min="1"
              max="100"
              step="1"
              class="input input-bordered input-sm"
            />
          </label>
        </div>
        <div v-if="!toolApiConfig.enableTools" class="text-xs opacity-70">当前对话AI未启用工具调用。</div>
        <div v-else class="grid gap-2">
          <div
            v-for="tool in toolApiConfig.tools"
            :key="tool.id"
            class="card card-compact bg-base-100 border border-base-300"
          >
            <div class="card-body py-2 px-3">
              <div class="flex items-center justify-between gap-2">
                <div class="text-xs font-medium">{{ tool.id }}</div>
                <div class="flex items-center gap-2">
                  <button
                    v-if="tool.id === 'memory-save'"
                    class="btn btn-xs btn-ghost bg-base-100"
                    @click="$emit('openMemoryViewer')"
                  >
                    查看
                  </button>
                  <div class="badge" :class="statusBadgeClass(tool.id)">
                    {{ statusText(tool.id) }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>
    </template>

    <template v-else-if="configTab === 'persona'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">当前人格</span></div>
        <select :value="selectedPersonaId" class="select select-bordered select-sm" @change="$emit('update:selectedPersonaId', ($event.target as HTMLSelectElement).value)">
          <option v-for="p in personas" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
      </label>
      <div class="divider my-0"></div>
      <div v-if="selectedPersona" class="grid gap-2">
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-xs">人格名称</span></div>
          <input v-model="selectedPersona.name" class="input input-bordered input-sm" placeholder="人格名称" />
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-xs">人格设定</span></div>
          <textarea v-model="selectedPersona.systemPrompt" class="textarea textarea-bordered textarea-sm" rows="4" placeholder="系统提示词"></textarea>
        </label>
      </div>
      <div class="flex gap-1">
        <button class="btn btn-sm" @click="$emit('addPersona')">新增</button>
        <button class="btn btn-sm" :disabled="personas.length <= 1" @click="$emit('removeSelectedPersona')">删除</button>
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
        <div class="label py-1"><span class="label-text text-xs">默认人格</span></div>
        <select :value="selectedPersonaId" class="select select-bordered select-sm" @change="$emit('update:selectedPersonaId', ($event.target as HTMLSelectElement).value)">
          <option v-for="p in personas" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">用户称谓</span></div>
        <input :value="userAlias" class="input input-bordered input-sm" placeholder="用户" @input="$emit('update:userAlias', ($event.target as HTMLInputElement).value)" />
      </label>
      <div class="flex gap-1">
        <button class="btn btn-sm" @click="$emit('openCurrentHistory')">查看当前未归档记录</button>
      </div>
      <div class="rounded border border-base-300 bg-base-100 p-2 text-xs">
        <div class="flex items-center justify-between">
          <span class="font-medium">图片转文缓存</span>
          <div class="flex gap-1">
            <button class="btn btn-xs btn-ghost" :class="{ loading: cacheStatsLoading }" @click="$emit('refreshImageCacheStats')">刷新</button>
            <button class="btn btn-xs btn-ghost" :disabled="cacheStats.entries === 0" @click="$emit('clearImageCache')">清理</button>
          </div>
        </div>
        <div class="mt-1 opacity-80">条目: {{ cacheStats.entries }} | 字符: {{ cacheStats.totalChars }}</div>
        <div class="mt-1 opacity-70">最近更新: {{ cacheStats.latestUpdatedAt || "-" }}</div>
        <div class="mt-1 opacity-60">缓存按“图转文AI配置”隔离，切换图转文AI后会自动使用对应缓存命名空间。</div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ToolLoadStatus } from "../types/app";
import { ChevronsUpDown, Moon, Plus, RefreshCw, Sun, Trash2 } from "lucide-vue-next";

type ConfigTab = "hotkey" | "api" | "tools" | "persona" | "chatSettings";

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  currentTheme: "light" | "forest";
  selectedApiConfig: ApiConfigItem | null;
  toolApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  toolStatuses: ToolLoadStatus[];
  personas: PersonaProfile[];
  selectedPersonaId: string;
  selectedPersona: PersonaProfile | null;
  userAlias: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
  configDirty: boolean;
  savingConfig: boolean;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
}>();

defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:selectedPersonaId", value: string): void;
  (e: "update:userAlias", value: string): void;
  (e: "toggleTheme"): void;
  (e: "refreshModels"): void;
  (e: "openMemoryViewer"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "saveApiConfig"): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "openCurrentHistory"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
}>();

function toolStatusById(id: string): ToolLoadStatus | undefined {
  return props.toolStatuses.find((s) => s.id === id);
}

function statusText(id: string): string {
  return toolStatusById(id)?.status ?? "unknown";
}

function statusBadgeClass(id: string): string {
  const status = toolStatusById(id)?.status;
  if (status === "loaded") return "badge-success";
  if (status === "failed" || status === "timeout") return "badge-error";
  if (status === "disabled") return "badge-ghost";
  return "badge-outline";
}
</script>
