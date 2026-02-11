<template>
  <div class="grid gap-2">
    <div class="tabs tabs-boxed tabs-sm">
      <a class="tab" :class="{ 'tab-active': configTab === 'hotkey' }" @click="$emit('update:configTab', 'hotkey')">{{ t("config.tabs.hotkey") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'api' }" @click="$emit('update:configTab', 'api')">{{ t("config.tabs.api") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'tools' }" @click="$emit('update:configTab', 'tools')">{{ t("config.tabs.tools") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'persona' }" @click="$emit('update:configTab', 'persona')">{{ t("config.tabs.persona") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'chatSettings' }" @click="$emit('update:configTab', 'chatSettings')">{{ t("config.tabs.chatSettings") }}</a>
    </div>

    <template v-if="configTab === 'hotkey'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.language.label") }}</span></div>
        <select :value="uiLanguage" class="select select-bordered select-sm" @change="$emit('update:uiLanguage', ($event.target as HTMLSelectElement).value)">
          <option v-for="item in localeOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.label") }}</span></div>
        <div class="flex items-center gap-2">
          <input :value="config.hotkey" class="input input-bordered input-sm flex-1" placeholder="Alt+·" readonly />
          <button
            class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200"
            :class="{ 'btn-primary': hotkeyCapturing }"
            @click="toggleHotkeyCapture"
          >
            {{ hotkeyCapturing ? t("config.hotkey.recording") : t("config.hotkey.recordButton") }}
          </button>
        </div>
        <div class="label py-1">
          <span class="label-text-alt text-[11px] opacity-70">{{ hotkeyCaptureHint }}</span>
        </div>
        <div v-if="hotkeyRecordConflict" class="text-xs text-error mt-1">{{ t("config.hotkey.conflict") }}</div>
      </label>
      <div class="grid grid-cols-3 gap-2">
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.recordKey") }}</span></div>
          <select v-model="config.recordHotkey" class="select select-bordered select-sm">
            <option value="Alt">Alt</option>
            <option value="Ctrl">Ctrl</option>
            <option value="Shift">Shift</option>
          </select>
        </label>
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.minRecordSeconds") }}</span></div>
          <input v-model.number="config.minRecordSeconds" type="number" min="1" max="30" class="input input-bordered input-sm" />
        </label>
        <label class="form-control col-span-1">
          <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.maxRecordSeconds") }}</span></div>
          <input v-model.number="config.maxRecordSeconds" type="number" min="1" max="600" class="input input-bordered input-sm" />
        </label>
      </div>
      <div class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.recordTest") }}</span></div>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-sm btn-ghost bg-base-100"
            :class="{ 'btn-error text-error-content': hotkeyTestRecording }"
            :title="hotkeyTestRecording ? t('config.hotkey.releaseToStop') : t('config.hotkey.holdToRecord')"
            @mousedown.prevent="$emit('startHotkeyRecordTest')"
            @mouseup.prevent="$emit('stopHotkeyRecordTest')"
            @mouseleave.prevent="hotkeyTestRecording && $emit('stopHotkeyRecordTest')"
            @touchstart.prevent="$emit('startHotkeyRecordTest')"
            @touchend.prevent="$emit('stopHotkeyRecordTest')"
          >
            {{ hotkeyTestRecording ? t("chat.recording", { seconds: Math.max(1, Math.round(hotkeyTestRecordingMs / 1000)) }) : t("config.hotkey.holdRecordButton") }}
          </button>
          <button
            class="btn btn-sm btn-ghost bg-base-100"
            :disabled="!hotkeyTestAudioReady"
            @click="$emit('playHotkeyRecordTest')"
          >
            {{ t("config.hotkey.playRecord") }}
          </button>
        </div>
      </div>
      <div class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.theme") }}</span></div>
        <button class="btn btn-sm btn-ghost bg-base-100 w-full flex items-center justify-center gap-2" @click="$emit('toggleTheme')">
          <Sun v-if="currentTheme === 'light'" class="h-4 w-4" />
          <Moon v-else class="h-4 w-4" />
          <span>{{ currentTheme === "light" ? t("config.hotkey.lightTheme") : t("config.hotkey.darkTheme") }}</span>
        </button>
      </div>
    </template>

    <template v-else-if="configTab === 'api'">
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

    <template v-else-if="configTab === 'tools'">
      <div v-if="!toolApiConfig" class="text-xs opacity-70">{{ t("config.tools.noChatApi") }}</div>
      <template v-else>
        <div class="grid gap-2">
          <label class="form-control">
            <div class="label py-1"><span class="label-text text-xs">{{ t("config.tools.maxIterations") }}</span></div>
            <input v-model.number="config.toolMaxIterations" type="number" min="1" max="100" step="1" class="input input-bordered input-sm" />
          </label>
        </div>
        <div v-if="!toolApiConfig.enableTools" class="text-xs opacity-70">{{ t("config.tools.disabledHint") }}</div>
        <div v-else class="grid gap-2">
          <div v-for="tool in toolApiConfig.tools" :key="tool.id" class="card card-compact bg-base-100 border border-base-300">
            <div class="card-body py-2 px-3">
              <div class="flex items-center justify-between gap-2">
                <div class="text-xs font-medium">{{ tool.id }}</div>
                <div class="flex items-center gap-2">
                  <button v-if="tool.id === 'memory-save'" class="btn btn-xs btn-ghost bg-base-100" @click="$emit('openMemoryViewer')">{{ t("config.tools.viewMemory") }}</button>
                  <div class="badge" :class="statusBadgeClass(tool.id)">{{ statusText(tool.id) }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>
    </template>

    <template v-else-if="configTab === 'persona'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.title") }}</span></div>
        <div class="flex gap-1">
          <select :value="personaEditorId" class="select select-bordered select-sm flex-1" @change="$emit('update:personaEditorId', ($event.target as HTMLSelectElement).value)">
            <option v-for="p in personas" :key="p.id" :value="p.id">{{ p.name }}{{ p.isBuiltInUser ? `（${t("config.persona.userTag")}）` : "" }}</option>
          </select>
          <button class="btn btn-sm btn-square text-primary bg-base-100" :title="t('config.persona.add')" @click="$emit('addPersona')">
            <Plus class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-square text-error bg-base-100"
            :title="t('config.persona.remove')"
            :disabled="!selectedPersona || selectedPersona.isBuiltInUser || assistantPersonas.length <= 1"
            @click="$emit('removeSelectedPersona')"
          >
            <Trash2 class="h-3.5 w-3.5" />
          </button>
        </div>
      </label>
      <div class="divider my-0"></div>

      <div v-if="selectedPersona" class="grid gap-2">
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.name") }}</span></div>
          <div class="flex items-center gap-2">
            <input v-model="selectedPersona.name" class="input input-bordered input-sm flex-1" :placeholder="t('config.persona.name')" />
            <button
              class="btn btn-ghost btn-circle p-0 min-h-0 h-auto w-auto"
              :disabled="avatarSaving"
              :title="avatarSaving ? t('config.persona.avatarSaving') : t('config.persona.editAvatar')"
              @click="openAvatarEditorForSelected"
            >
              <div v-if="selectedPersonaAvatarUrl" class="avatar">
                <div class="w-10 rounded-full">
                  <img :src="selectedPersonaAvatarUrl" :alt="selectedPersona.name" :title="selectedPersona.name" />
                </div>
              </div>
              <div v-else class="avatar placeholder">
                <div class="bg-neutral text-neutral-content w-10 rounded-full">
                  <span>{{ avatarInitial(selectedPersona.name) }}</span>
                </div>
              </div>
            </button>
          </div>
          <div v-if="avatarError" class="label py-1"><span class="label-text-alt text-error break-all">{{ avatarError }}</span></div>
        </label>
        <label class="form-control">
          <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.prompt") }}</span></div>
          <textarea
            v-model="selectedPersona.systemPrompt"
            class="textarea textarea-bordered textarea-sm"
            rows="4"
            :placeholder="selectedPersona.isBuiltInUser ? t('config.persona.userPlaceholder') : t('config.persona.assistantPlaceholder')"
          ></textarea>
        </label>
      </div>

    </template>

    <template v-else-if="configTab === 'chatSettings'">
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.chatApi") }}</span></div>
        <select v-model="config.chatApiConfigId" class="select select-bordered select-sm">
          <option v-for="a in textCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.visionApi") }}</span></div>
        <select :value="config.visionApiConfigId ?? ''" class="select select-bordered select-sm" @change="config.visionApiConfigId = (($event.target as HTMLSelectElement).value || undefined)">
          <option value="">{{ t("config.chatSettings.noVision") }}</option>
          <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </label>
      <label class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.assistantPersona") }}</span></div>
        <select :value="selectedPersonaId" class="select select-bordered select-sm" @change="$emit('update:selectedPersonaId', ($event.target as HTMLSelectElement).value)">
          <option v-for="p in assistantPersonas" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
      </label>
      <div class="form-control">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.responseStyle") }}</span></div>
        <div class="join w-full">
          <button
            v-for="style in responseStyleOptions"
            :key="style.id"
            class="btn btn-sm join-item flex-1"
            :class="responseStyleId === style.id ? 'btn-primary' : 'btn-ghost bg-base-100'"
            @click="$emit('update:responseStyleId', style.id)"
          >
            {{ t(`responseStyle.${style.id}`) }}
          </button>
        </div>
      </div>
      <div class="grid grid-cols-3 gap-1 min-w-0">
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openCurrentHistory')">{{ t("config.chatSettings.openCurrentHistory") }}</button>
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openPromptPreview')">{{ t("config.chatSettings.previewRequest") }}</button>
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openSystemPromptPreview')">{{ t("config.chatSettings.previewSystemPrompt") }}</button>
      </div>
      <div class="rounded border border-base-300 bg-base-100 p-2 text-xs">
        <div class="flex items-center justify-between">
          <span class="font-medium">{{ t("config.chatSettings.imageCacheTitle") }}</span>
          <div class="flex gap-1">
            <button class="btn btn-xs btn-ghost" :class="{ loading: cacheStatsLoading }" @click="$emit('refreshImageCacheStats')">{{ t("common.refresh") }}</button>
            <button class="btn btn-xs btn-ghost" :disabled="cacheStats.entries === 0" @click="$emit('clearImageCache')">{{ t("config.chatSettings.clearCache") }}</button>
          </div>
        </div>
        <div class="mt-1 opacity-80">{{ t("config.chatSettings.cacheEntries", { entries: cacheStats.entries, chars: cacheStats.totalChars }) }}</div>
        <div class="mt-1 opacity-70">{{ t("config.chatSettings.cacheUpdatedAt", { value: cacheStats.latestUpdatedAt || "-" }) }}</div>
        <div class="mt-1 opacity-60">{{ t("config.chatSettings.cacheHint") }}</div>
      </div>
    </template>
  </div>

  <input ref="avatarFileInput" type="file" accept="image/*" class="hidden" @change="onAvatarFilePicked" />
  <dialog ref="avatarEditorDialog" class="modal">
    <div class="modal-box p-3 max-w-sm">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.editAvatar") }}</h3>
      <div class="rounded border border-base-300 bg-base-100 p-3">
        <div class="flex items-center gap-3">
          <div v-if="avatarEditorAvatarUrl" class="avatar">
            <div class="w-14 rounded-full">
              <img :src="avatarEditorAvatarUrl" :alt="avatarEditorName" :title="avatarEditorName" />
            </div>
          </div>
          <div v-else class="avatar placeholder">
            <div class="bg-neutral text-neutral-content w-14 rounded-full">
              <span>{{ avatarInitial(avatarEditorName) }}</span>
            </div>
          </div>
          <div class="text-xs opacity-70 break-all">{{ avatarEditorName }}</div>
        </div>
        <div class="mt-3 flex gap-2">
          <button class="btn btn-xs" :disabled="!avatarEditorTargetId || avatarSaving" @click="openAvatarPickerForEditor">{{ t("config.persona.uploadAvatar") }}</button>
          <button class="btn btn-xs btn-ghost" :disabled="!avatarEditorTargetHasAvatar || avatarSaving" @click="clearAvatarFromEditor">{{ t("config.persona.clearAvatar") }}</button>
        </div>
        <div v-if="avatarError" class="mt-2 text-xs text-error break-all">{{ avatarError }}</div>
      </div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeAvatarEditor">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
  </dialog>
  <dialog ref="cropDialog" class="modal" @close="destroyCropper">
    <div class="modal-box p-3 max-w-md">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.cropAvatar") }}</h3>
      <div class="rounded border border-base-300 bg-base-100 p-2 min-h-64">
        <img ref="cropImageEl" :src="cropSource" alt="crop source" class="max-w-full block" />
      </div>
      <div v-if="localCropError || avatarError" class="mt-2 text-xs text-error break-all">{{ localCropError || avatarError }}</div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeCropDialog">{{ t("common.cancel") }}</button>
        <button class="btn btn-sm btn-primary" :disabled="!cropperReady || avatarSaving" @click="confirmCrop">
          {{ avatarSaving ? t("config.api.saving") : t("config.persona.saveAvatar") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ResponseStyleOption, ToolLoadStatus } from "../types/app";
import { ChevronsUpDown, Moon, Plus, RefreshCw, Sun, Trash2 } from "lucide-vue-next";
import Cropper from "cropperjs";

type ConfigTab = "hotkey" | "api" | "tools" | "persona" | "chatSettings";
type AvatarTarget = { agentId: string };

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  uiLanguage: "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "ja-JP" | "ko-KR"; label: string }>;
  currentTheme: "light" | "forest";
  selectedApiConfig: ApiConfigItem | null;
  toolApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshError: string;
  toolStatuses: ToolLoadStatus[];
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  userPersona: PersonaProfile | null;
  personaEditorId: string;
  selectedPersonaId: string;
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  selectedPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  userPersonaAvatarUrl: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
  avatarSaving: boolean;
  avatarError: string;
  configDirty: boolean;
  savingConfig: boolean;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
}>();

const emit = defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:uiLanguage", value: string): void;
  (e: "update:personaEditorId", value: string): void;
  (e: "update:selectedPersonaId", value: string): void;
  (e: "update:responseStyleId", value: string): void;
  (e: "toggleTheme"): void;
  (e: "refreshModels"): void;
  (e: "openMemoryViewer"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "saveApiConfig"): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
  (e: "captureHotkey", value: string): void;
  (e: "saveAgentAvatar", value: { agentId: string; mime: string; bytesBase64: string }): void;
  (e: "clearAgentAvatar", value: { agentId: string }): void;
}>();
const { t } = useI18n();

const avatarFileInput = ref<HTMLInputElement | null>(null);
const avatarEditorDialog = ref<HTMLDialogElement | null>(null);
const cropDialog = ref<HTMLDialogElement | null>(null);
const cropImageEl = ref<HTMLImageElement | null>(null);
const cropSource = ref("");
const cropperReady = ref(false);
const localCropError = ref("");
const avatarEditorTargetId = ref("");
let cropper: Cropper | null = null;
let cropTarget: AvatarTarget | null = null;
const hotkeyCapturing = ref(false);
const hotkeyCaptureHint = ref(t("config.hotkey.captureDefaultHint"));
let hotkeyCaptureHandler: ((event: KeyboardEvent) => void) | null = null;
const hotkeyRecordConflict = computed(() => {
  const hotkey = String(props.config.hotkey || "").trim().toUpperCase();
  const recordHotkey = String(props.config.recordHotkey || "").trim().toUpperCase();
  if (!hotkey || !recordHotkey) return false;
  return hotkey === recordHotkey;
});

function isModifierKey(code: string): boolean {
  return code === "AltLeft"
    || code === "AltRight"
    || code === "ControlLeft"
    || code === "ControlRight"
    || code === "ShiftLeft"
    || code === "ShiftRight"
    || code === "MetaLeft"
    || code === "MetaRight";
}

function mainKeyFromEvent(event: KeyboardEvent): string {
  const code = event.code || "";
  if (code === "Backquote") return "·";
  if (code.startsWith("Key") && code.length === 4) return code.slice(3).toUpperCase();
  if (code.startsWith("Digit") && code.length === 6) return code.slice(5);
  if (/^F\\d{1,2}$/.test(code)) return code;
  if (code === "Minus") return "-";
  if (code === "Equal") return "=";
  if (code === "BracketLeft") return "[";
  if (code === "BracketRight") return "]";
  if (code === "Backslash") return "\\";
  if (code === "Semicolon") return ";";
  if (code === "Quote") return "'";
  if (code === "Comma") return ",";
  if (code === "Period") return ".";
  if (code === "Slash") return "/";
  if (code === "Space") return "Space";
  const key = event.key || "";
  if (key.length === 1) return key.toUpperCase();
  return key;
}

function stopHotkeyCapture() {
  hotkeyCapturing.value = false;
  if (hotkeyCaptureHandler) {
    window.removeEventListener("keydown", hotkeyCaptureHandler, true);
    hotkeyCaptureHandler = null;
  }
}

function startHotkeyCapture() {
  if (hotkeyCapturing.value) return;
  hotkeyCapturing.value = true;
  hotkeyCaptureHint.value = t("config.hotkey.captureListeningHint");
  hotkeyCaptureHandler = (event: KeyboardEvent) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      hotkeyCaptureHint.value = t("config.hotkey.captureCancelledHint");
      stopHotkeyCapture();
      return;
    }

    const modifiers: string[] = [];
    if (event.ctrlKey) modifiers.push("Ctrl");
    if (event.altKey) modifiers.push("Alt");
    if (event.shiftKey) modifiers.push("Shift");
    if (event.metaKey) modifiers.push("Meta");

    if (isModifierKey(event.code)) {
      hotkeyCaptureHint.value = t("config.hotkey.captureNeedMainKeyHint");
      return;
    }
    if (modifiers.length === 0) {
      hotkeyCaptureHint.value = t("config.hotkey.captureNeedModifierHint");
      return;
    }

    const main = mainKeyFromEvent(event).trim();
    if (!main) {
      hotkeyCaptureHint.value = t("config.hotkey.captureUnrecognizedHint");
      return;
    }
    const combo = `${modifiers.join("+")}+${main}`;
    emit("captureHotkey", combo);
    hotkeyCaptureHint.value = t("config.hotkey.captureCapturedHint", { combo });
    stopHotkeyCapture();
  };
  window.addEventListener("keydown", hotkeyCaptureHandler, true);
}

function toggleHotkeyCapture() {
  if (hotkeyCapturing.value) {
    hotkeyCaptureHint.value = t("config.hotkey.captureCancelledHint");
    stopHotkeyCapture();
    return;
  }
  startHotkeyCapture();
}

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

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function openAvatarPicker(target: AvatarTarget) {
  cropTarget = target;
  if (avatarFileInput.value) {
    avatarFileInput.value.value = "";
    avatarFileInput.value.click();
  }
}

function openAvatarEditorForSelected() {
  if (!props.selectedPersona) return;
  avatarEditorTargetId.value = props.selectedPersona.id;
  avatarEditorDialog.value?.showModal();
}

function closeAvatarEditor() {
  avatarEditorDialog.value?.close();
}

function openAvatarPickerForEditor() {
  if (!avatarEditorTargetId.value) return;
  openAvatarPicker({ agentId: avatarEditorTargetId.value });
}

function clearAvatarFromEditor() {
  if (!avatarEditorTargetId.value) return;
  emit("clearAgentAvatar", { agentId: avatarEditorTargetId.value });
}

function avatarById(id: string): PersonaProfile | null {
  return props.personas.find((p) => p.id === id) ?? null;
}

const avatarEditorTarget = () => avatarById(avatarEditorTargetId.value);

const avatarEditorName = computed(() => avatarEditorTarget()?.name || t("config.persona.avatarFallbackName"));
const avatarEditorAvatarUrl = computed(() => {
  const target = avatarEditorTarget();
  if (!target) return "";
  if (target.id === props.userPersona?.id) return props.userPersonaAvatarUrl;
  if (target.id === props.selectedPersona?.id) return props.selectedPersonaAvatarUrl;
  return "";
});
const avatarEditorTargetHasAvatar = computed(() => !!avatarEditorTarget()?.avatarPath);

async function readFileAsDataUrl(file: File): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function loadImage(dataUrl: string): Promise<HTMLImageElement> {
  return await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("load image failed"));
    img.src = dataUrl;
  });
}

async function downscaleDataUrl(dataUrl: string, maxSide = 1024): Promise<string> {
  const img = await loadImage(dataUrl);
  const w = img.naturalWidth || img.width;
  const h = img.naturalHeight || img.height;
  if (w <= maxSide && h <= maxSide) return dataUrl;
  const scale = Math.min(1, maxSide / Math.max(w, h));
  const targetW = Math.max(1, Math.round(w * scale));
  const targetH = Math.max(1, Math.round(h * scale));
  const canvas = document.createElement("canvas");
  canvas.width = targetW;
  canvas.height = targetH;
  const ctx = canvas.getContext("2d");
  if (!ctx) return dataUrl;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, 0, 0, targetW, targetH);
  return canvas.toDataURL("image/webp", 0.9);
}

function destroyCropper() {
  if (cropper) {
    cropper.destroy();
    cropper = null;
  }
  cropperReady.value = false;
}

function closeCropDialog() {
  cropDialog.value?.close();
  cropSource.value = "";
  cropTarget = null;
  localCropError.value = "";
}

async function onAvatarFilePicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file || !cropTarget) return;
  localCropError.value = "";
  try {
    const dataUrl = await readFileAsDataUrl(file);
    cropSource.value = await downscaleDataUrl(dataUrl, 1024);
    await nextTick();
    destroyCropper();
    if (!cropImageEl.value) {
      localCropError.value = t("config.persona.cropInitFailed");
      return;
    }
    cropper = new Cropper(cropImageEl.value, {
      aspectRatio: 1,
      viewMode: 1,
      dragMode: "move",
      autoCropArea: 1,
      background: false,
      guides: false,
    });
    cropperReady.value = true;
    cropDialog.value?.showModal();
  } catch (e) {
    localCropError.value = t("config.persona.avatarReadFailed", { err: String(e) });
  }
}

function confirmCrop() {
  if (!cropTarget) {
    localCropError.value = t("config.persona.cropMissingTarget");
    return;
  }
  if (!cropper) {
    localCropError.value = t("config.persona.cropperNotReady");
    return;
  }
  localCropError.value = "";
  const canvas = cropper.getCroppedCanvas({
    width: 128,
    height: 128,
    imageSmoothingEnabled: true,
    imageSmoothingQuality: "high",
  });
  const dataUrl = canvas.toDataURL("image/webp", 0.8);
  const marker = "base64,";
  const idx = dataUrl.indexOf(marker);
  if (idx < 0) return;
  const bytesBase64 = dataUrl.slice(idx + marker.length);
  emit("saveAgentAvatar", {
    agentId: cropTarget.agentId,
    mime: "image/webp",
    bytesBase64,
  });
  closeCropDialog();
}

onBeforeUnmount(() => {
  stopHotkeyCapture();
  destroyCropper();
});
</script>
