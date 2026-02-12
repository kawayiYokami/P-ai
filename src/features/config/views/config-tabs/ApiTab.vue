<template>
  <button class="btn btn-primary btn-sm w-full" :disabled="!configDirty || savingConfig" @click="$emit('saveApiConfig')">
    {{ savingConfig ? t("config.api.saving") : configDirty ? t("config.api.saveConfig") : t("config.api.saved") }}
  </button>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.editTitle") }}</span></div>
    <div class="flex gap-1">
      <select v-model="config.selectedApiConfigId" class="select select-bordered select-sm flex-1">
        <option v-for="a in config.apiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
      </select>
      <button class="btn btn-sm btn-square btn-ghost bg-base-100" :title="t('config.api.addConfig')" @click="$emit('addApiConfig')">
        <Plus class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-sm btn-square btn-ghost bg-base-100" :title="t('config.api.removeConfig')" :disabled="config.apiConfigs.length <= 1" @click="$emit('removeSelectedApiConfig')">
        <Trash2 class="h-3.5 w-3.5" />
      </button>
    </div>
  </label>

  <div class="divider my-0"></div>

  <div v-if="selectedApiConfig" class="grid gap-2">
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.configName") }}</span></div>
      <input v-model="selectedApiConfig.name" class="input input-bordered input-sm" :placeholder="t('config.api.configName')" />
    </label>
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.requestFormat") }}</span></div>
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
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.model") }}</span>
        <span class="label-text-alt text-[11px] text-error min-h-4">{{ modelRefreshError || " " }}</span>
      </div>
      <div class="flex gap-1">
        <input v-model="selectedApiConfig.model" class="input input-bordered input-sm flex-1" placeholder="model" />
        <div class="dropdown dropdown-end">
          <button tabindex="0" class="btn btn-sm btn-square btn-ghost bg-base-100" :disabled="modelOptions.length === 0" :title="t('config.api.pickModel')">
            <ChevronsUpDown class="h-3.5 w-3.5" />
          </button>
          <ul tabindex="0" class="dropdown-content z-[1] menu p-1 shadow bg-base-100 rounded-box w-52 max-h-56 overflow-auto">
            <li v-for="modelName in modelOptions" :key="modelName">
              <button @click="selectedApiConfig.model = modelName">{{ modelName }}</button>
            </li>
          </ul>
        </div>
        <button class="btn btn-sm btn-square btn-ghost bg-base-100" :class="{ loading: refreshingModels }" :disabled="refreshingModels" :title="t('config.api.refreshModels')" @click="$emit('refreshModels')">
          <RefreshCw class="h-3.5 w-3.5" />
        </button>
      </div>
    </label>
    <label class="form-control">
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.temperature") }}</span>
        <span class="label-text-alt text-xs opacity-70">{{ Number(selectedApiConfig.temperature ?? 1).toFixed(1) }}</span>
      </div>
      <input v-model.number="selectedApiConfig.temperature" type="range" min="0" max="2" step="0.1" class="range range-xs" />
      <div class="mt-1 flex justify-between text-[10px] opacity-60">
        <span>0.0</span>
        <span>1.0</span>
        <span>2.0</span>
      </div>
    </label>
    <label class="form-control">
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.contextWindow") }}</span>
        <span class="label-text-alt text-xs opacity-70">{{ Math.round(Number(selectedApiConfig.contextWindowTokens ?? 128000)) }}</span>
      </div>
      <input v-model.number="selectedApiConfig.contextWindowTokens" type="range" min="16000" max="200000" step="1000" class="range range-xs" />
      <div class="mt-1 flex justify-between text-[10px] opacity-60">
        <span>16K</span>
        <span>100K</span>
        <span>200K</span>
      </div>
    </label>
    <div class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.capabilities") }}</span></div>
      <div class="flex gap-2">
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capText") }}</span><input v-model="selectedApiConfig.enableText" type="checkbox" class="toggle toggle-sm" /></label>
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capImage") }}</span><input v-model="selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capTools") }}</span><input v-model="selectedApiConfig.enableTools" type="checkbox" class="toggle toggle-sm" /></label>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ChevronsUpDown, Plus, RefreshCw, Trash2 } from "lucide-vue-next";
import type { ApiConfigItem, AppConfig } from "../../../../types/app";

defineProps<{
  config: AppConfig;
  selectedApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshError: string;
  configDirty: boolean;
  savingConfig: boolean;
}>();

defineEmits<{
  (e: "saveApiConfig"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "refreshModels"): void;
}>();

const { t } = useI18n();
</script>


