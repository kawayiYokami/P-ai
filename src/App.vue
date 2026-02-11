
<template>
  <div class="window-shell text-sm bg-base-200">
    <div class="navbar min-h-10 h-10 px-2 relative z-20 overflow-visible cursor-move select-none" :class="viewMode === 'chat' ? '' : 'bg-base-200 border-b border-base-300'" @mousedown.left.prevent="startDrag">
      <div class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex items-center px-2">
        <span class="font-semibold text-sm">{{ titleText }}</span>
        <template v-if="viewMode === 'chat'">
          <div class="tooltip tooltip-bottom" data-tip="点击强制归档当前对话">
            <button
              class="btn btn-ghost btn-xs ml-2"
              :disabled="forcingArchive || chatting"
              @mousedown.stop
              @click.stop="forceArchiveNow"
            >
              {{ chatUsagePercent }}%
            </button>
          </div>
        </template>
      </div>
      <div class="flex-none flex gap-1 ml-auto" @mousedown.stop>
        <template v-if="viewMode === 'chat'">
          <button
            class="btn btn-ghost btn-xs"
            :class="{ 'btn-active': alwaysOnTop }"
            :title="alwaysOnTop ? '取消总在最前' : '总在最前窗口'"
            @click.stop="toggleAlwaysOnTop"
            :disabled="!windowReady"
          >
            <Pin class="h-3.5 w-3.5" />
          </button>
          <button class="btn btn-ghost btn-xs hover:bg-error/20" title="Close" @click.stop="closeWindow" :disabled="!windowReady"><X class="h-3.5 w-3.5" /></button>
        </template>
        <template v-else>
          <button class="btn btn-ghost btn-xs hover:bg-error/20" title="Close" @click.stop="closeWindow" :disabled="!windowReady"><X class="h-3.5 w-3.5" /></button>
        </template>
      </div>
    </div>

    <div class="window-content" :class="viewMode === 'chat' ? 'flex flex-col' : 'p-3'">
      <ConfigView
        v-if="viewMode === 'config'"
        :config="config"
        :config-tab="configTab"
        :current-theme="currentTheme"
        :selected-api-config="selectedApiConfig"
        :tool-api-config="toolApiConfig"
        :base-url-reference="baseUrlReference"
        :refreshing-models="refreshingModels"
        :model-options="selectedModelOptions"
        :model-refresh-error="modelRefreshError"
        :tool-statuses="toolStatuses"
        :personas="personas"
        :selected-persona-id="selectedPersonaId"
        :selected-persona="selectedPersona"
        :user-alias="userAlias"
        :text-capable-api-configs="textCapableApiConfigs"
        :image-capable-api-configs="imageCapableApiConfigs"
        :cache-stats="imageCacheStats"
        :cache-stats-loading="imageCacheStatsLoading"
        :config-dirty="configDirty"
        :saving-config="saving"
        :hotkey-test-recording="hotkeyTestRecording"
        :hotkey-test-recording-ms="hotkeyTestRecordingMs"
        :hotkey-test-audio-ready="!!hotkeyTestAudio"
        @update:config-tab="configTab = $event"
        @update:selected-persona-id="selectedPersonaId = $event"
        @update:user-alias="userAlias = $event"
        @toggle-theme="toggleTheme"
        @refresh-models="refreshModels"
        @save-api-config="saveConfig"
        @add-api-config="addApiConfig"
        @remove-selected-api-config="removeSelectedApiConfig"
        @add-persona="addPersona"
        @remove-selected-persona="removeSelectedPersona"
        @open-current-history="openCurrentHistory"
        @open-memory-viewer="openMemoryViewer"
        @refresh-image-cache-stats="refreshImageCacheStats"
        @clear-image-cache="clearImageCache"
        @start-hotkey-record-test="startHotkeyRecordTest"
        @stop-hotkey-record-test="stopHotkeyRecordTest"
        @play-hotkey-record-test="playHotkeyRecordTest"
      />

      <div v-else-if="viewMode === 'chat'" class="relative flex-1">
        <ChatView
          :user-alias="userAlias"
          :persona-name="selectedPersona?.name || '助理'"
          :latest-user-text="latestUserText"
          :latest-user-images="latestUserImages"
          :latest-assistant-text="latestAssistantText"
          :latest-reasoning-standard-text="latestReasoningStandardText"
          :latest-reasoning-inline-text="latestReasoningInlineText"
          :tool-status-text="toolStatusText"
          :tool-status-state="toolStatusState"
          :chat-error-text="chatErrorText"
          :clipboard-images="clipboardImages"
          :chat-input="chatInput"
          :chat-input-placeholder="chatInputPlaceholder"
          :can-record="speechRecognitionSupported"
          :recording="recording"
          :recording-ms="recordingMs"
          :record-hotkey="config.recordHotkey"
          :chatting="chatting"
          :frozen="forcingArchive"
          :turns="visibleTurns"
          :has-more-turns="hasMoreTurns"
          @update:chat-input="chatInput = $event"
          @remove-clipboard-image="removeClipboardImage"
          @start-recording="startRecording"
          @stop-recording="stopRecording(false)"
          @send-chat="sendChat"
          @stop-chat="stopChat"
          @load-more-turns="loadMoreTurns"
        />
        <div
          v-if="forcingArchive"
          class="absolute inset-0 z-20 flex items-center justify-center bg-base-100/60 backdrop-blur-[1px]"
        >
          <div class="rounded-box border border-base-300 bg-base-100 px-4 py-3 shadow-sm flex flex-col items-center gap-1">
            <span class="loading loading-spinner loading-sm"></span>
            <div class="text-sm">正在归档优化上下文...</div>
            <div class="text-xs opacity-70">期间将暂时锁定输入</div>
          </div>
        </div>
      </div>

      <ArchivesView
        v-else
        :archives="archives"
        :archive-messages="archiveMessages"
        :render-message="renderMessage"
        @load-archives="loadArchives"
        @select-archive="selectArchive"
      />

      <dialog ref="historyDialog" class="modal">
        <div class="modal-box max-w-xl">
          <h3 class="font-semibold text-sm mb-2">当前会话记录（未归档）</h3>
          <div class="max-h-96 overflow-auto space-y-2">
            <div v-for="m in currentHistory" :key="m.id" class="text-xs border border-base-300 rounded p-2">
              <div class="font-semibold uppercase text-[11px]">{{ m.role }}</div>
              <div v-if="messageText(m)" class="whitespace-pre-wrap">{{ messageText(m) }}</div>
              <div v-if="extractMessageImages(m).length > 0" class="mt-2 grid gap-1">
                <img
                  v-for="(img, idx) in extractMessageImages(m)"
                  :key="`${img.mime}-${idx}`"
                  :src="`data:${img.mime};base64,${img.bytesBase64}`"
                  loading="lazy"
                  decoding="async"
                  class="rounded max-h-32 object-contain bg-base-100/40"
                />
              </div>
            </div>
          </div>
          <div class="modal-action"><button class="btn btn-sm" @click="closeHistory">关闭</button></div>
        </div>
      </dialog>

      <dialog ref="memoryDialog" class="modal">
        <div class="modal-box max-w-xl">
          <h3 class="font-semibold text-sm mb-2">记忆列表</h3>
          <input
            ref="memoryImportInput"
            type="file"
            accept=".json,application/json"
            class="hidden"
            @change="handleMemoryImportFile"
          />
          <div v-if="memoryList.length === 0" class="text-xs opacity-70">暂无记忆</div>
          <div v-else class="space-y-2">
            <div class="max-h-96 overflow-auto space-y-2">
              <div
                v-for="memory in pagedMemories"
                :key="memory.id"
                class="card card-compact bg-base-100 border border-base-300 shadow-md"
              >
                <div class="card-body text-xs p-3">
                  <div class="whitespace-pre-wrap break-words">{{ memory.content }}</div>
                  <div class="mt-2 flex flex-wrap items-center gap-1">
                    <span
                      v-for="(kw, kwIdx) in memory.keywords"
                      :key="`${memory.id}-kw-${kwIdx}`"
                      class="badge badge-sm badge-ghost"
                    >
                      {{ kw }}
                    </span>
                    <span v-if="memory.keywords.length === 0" class="text-[11px] opacity-60">-</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="flex items-center justify-between border-t border-base-300 pt-2">
              <span class="text-xs opacity-70">第 {{ memoryPage }} / {{ memoryPageCount }} 页</span>
              <div class="join">
                <button class="btn btn-xs join-item" :disabled="memoryPage <= 1" @click="memoryPage--">上一页</button>
                <button class="btn btn-xs join-item" :disabled="memoryPage >= memoryPageCount" @click="memoryPage++">下一页</button>
              </div>
            </div>
          </div>
          <div class="modal-action">
            <button class="btn btn-sm btn-ghost" @click="exportMemories">导出</button>
            <button class="btn btn-sm btn-ghost" @click="triggerMemoryImport">导入</button>
            <button class="btn btn-sm" @click="closeMemoryViewer">关闭</button>
          </div>
        </div>
      </dialog>

    </div>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { Channel, invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow, Window as WebviewWindow } from "@tauri-apps/api/window";
import { save } from "@tauri-apps/plugin-dialog";
import { Pin, X } from "lucide-vue-next";
import ConfigView from "./views/ConfigView.vue";
import ChatView from "./views/ChatView.vue";
import ArchivesView from "./views/ArchivesView.vue";
import type {
  PersonaProfile,
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatMessage,
  ChatSnapshot,
  ChatTurn,
  ImageTextCacheStats,
  ToolLoadStatus,
} from "./types/app";

let appWindow: WebviewWindow | null = null;
const viewMode = ref<"chat" | "archives" | "config">("config");

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  recordHotkey: "Alt",
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  toolMaxIterations: 10,
  selectedApiConfigId: "",
  chatApiConfigId: "",
  visionApiConfigId: undefined,
  apiConfigs: [],
});
const configTab = ref<"hotkey" | "api" | "tools" | "persona" | "chatSettings">("hotkey");
const currentTheme = ref<"light" | "forest">("light");
const personas = ref<PersonaProfile[]>([]);
const selectedPersonaId = ref("default-agent");
const userAlias = ref("用户");
const chatInput = ref("");
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const latestReasoningStandardText = ref("");
const latestReasoningInlineText = ref("");
const toolStatusText = ref("");
const toolStatusState = ref<"running" | "done" | "failed" | "">("");
const chatErrorText = ref("");
const currentHistory = ref<ChatMessage[]>([]);
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const recording = ref(false);
const recordingMs = ref(0);

const allMessages = shallowRef<ChatMessage[]>([]);
const visibleTurnCount = ref(1);

const archives = ref<ArchiveSummary[]>([]);
const archiveMessages = ref<ChatMessage[]>([]);

const windowReady = ref(false);
const status = ref("Ready.");
const loading = ref(false);
const saving = ref(false);
const chatting = ref(false);
const forcingArchive = ref(false);
const refreshingModels = ref(false);
const modelRefreshError = ref("");
const checkingToolsStatus = ref(false);
const toolStatuses = ref<ToolLoadStatus[]>([]);
const imageCacheStats = ref<ImageTextCacheStats>({ entries: 0, totalChars: 0 });
const imageCacheStatsLoading = ref(false);
const apiModelOptions = ref<Record<string, string[]>>({});
const historyDialog = ref<HTMLDialogElement | null>(null);
const memoryDialog = ref<HTMLDialogElement | null>(null);
const memoryImportInput = ref<HTMLInputElement | null>(null);
const alwaysOnTop = ref(false);
const configAutosaveReady = ref(false);
const personasAutosaveReady = ref(false);
const chatSettingsAutosaveReady = ref(false);
const suppressAutosave = ref(false);
let configAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let personasAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let chatSettingsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let mediaRecorder: MediaRecorder | null = null;
let mediaStream: MediaStream | null = null;
let recordingAudioContext: AudioContext | null = null;
let recordingAnalyser: AnalyserNode | null = null;
let recordingSourceNode: MediaStreamAudioSourceNode | null = null;
let recordingLevelTimer: ReturnType<typeof setInterval> | null = null;
let recordingPeakLevel = 0;
let recordingLevelSupported = false;
let recordingStartedAt = 0;
let recordingTickTimer: ReturnType<typeof setInterval> | null = null;
let recordingMaxTimer: ReturnType<typeof setTimeout> | null = null;
let recordingDiscardCurrent = false;
type SpeechRecognitionResultLike = { isFinal: boolean; 0: { transcript: string } };
type SpeechRecognitionEventLike = { resultIndex: number; results: ArrayLike<SpeechRecognitionResultLike> };
type SpeechRecognitionLike = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: { error?: string }) => void) | null;
  onend: (() => void) | null;
  start: () => void;
  stop: () => void;
};
let speechRecognizer: SpeechRecognitionLike | null = null;
let recordingRecognizedText = "";
const hotkeyTestRecording = ref(false);
const hotkeyTestRecordingMs = ref(0);
const hotkeyTestAudio = ref<{ mime: string; bytesBase64: string; durationMs: number } | null>(null);
let hotkeyTestRecorder: MediaRecorder | null = null;
let hotkeyTestStream: MediaStream | null = null;
let hotkeyTestStartedAt = 0;
let hotkeyTestTickTimer: ReturnType<typeof setInterval> | null = null;
let hotkeyTestPlayer: HTMLAudioElement | null = null;
let keydownHandler: ((event: KeyboardEvent) => void) | null = null;
let keyupHandler: ((event: KeyboardEvent) => void) | null = null;
const lastSavedConfigJson = ref("");
const memoryList = ref<MemoryEntry[]>([]);
const memoryPage = ref(1);
const MEMORY_PAGE_SIZE = 5;

const sortedMemories = computed(() =>
  [...memoryList.value].sort((a, b) => {
    const ta = Date.parse(a.updatedAt || a.createdAt || "");
    const tb = Date.parse(b.updatedAt || b.createdAt || "");
    if (Number.isFinite(ta) && Number.isFinite(tb)) return tb - ta;
    return (b.updatedAt || b.createdAt || "").localeCompare(a.updatedAt || a.createdAt || "");
  }),
);
const memoryPageCount = computed(() => Math.max(1, Math.ceil(sortedMemories.value.length / MEMORY_PAGE_SIZE)));
const pagedMemories = computed(() => {
  const page = Math.max(1, Math.min(memoryPage.value, memoryPageCount.value));
  const start = (page - 1) * MEMORY_PAGE_SIZE;
  return sortedMemories.value.slice(start, start + MEMORY_PAGE_SIZE);
});

const titleText = computed(() => {
  if (viewMode.value === "chat") {
    return `与 ${selectedPersona.value?.name || "助理"} 的对话`;
  }
  if (viewMode.value === "archives") {
    return "Easy Call AI - 归档窗口";
  }
  return "Easy Call AI - 配置窗口";
});
const selectedApiConfig = computed(() => config.apiConfigs.find((a) => a.id === config.selectedApiConfigId) ?? null);
const textCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableText));
const imageCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableImage));
const activeChatApiConfigId = computed(
  () => config.chatApiConfigId || textCapableApiConfigs.value[0]?.id || config.apiConfigs[0]?.id || "",
);
const activeChatApiConfig = computed(
  () => config.apiConfigs.find((a) => a.id === activeChatApiConfigId.value) ?? null,
);
const toolApiConfig = computed(() => activeChatApiConfig.value);
const hasVisionFallback = computed(() =>
  !!config.visionApiConfigId
  && config.apiConfigs.some((a) => a.id === config.visionApiConfigId && a.enableImage),
);
const speechRecognitionSupported = computed(() => {
  const w = window as typeof window & {
    SpeechRecognition?: new () => SpeechRecognitionLike;
    webkitSpeechRecognition?: new () => SpeechRecognitionLike;
  };
  return !!(w.SpeechRecognition || w.webkitSpeechRecognition);
});
const selectedPersona = computed(() => personas.value.find((p) => p.id === selectedPersonaId.value) ?? null);
const selectedModelOptions = computed(() => {
  const id = config.selectedApiConfigId;
  if (!id) return [];
  return apiModelOptions.value[id] ?? [];
});
const baseUrlReference = computed(() => {
  const format = selectedApiConfig.value?.requestFormat ?? "openai";
  if (format === "gemini") return "https://generativelanguage.googleapis.com/v1beta/openai";
  if (format === "deepseek/kimi") return "https://api.deepseek.com/v1";
  return "https://api.openai.com/v1";
});
const chatInputPlaceholder = computed(() => {
  const api = activeChatApiConfig.value;
  if (!api) return "输入问题";
  const hints: string[] = [];
  if (api.enableImage || hasVisionFallback.value) hints.push("Ctrl+V 粘贴图片");
  if (speechRecognitionSupported.value) hints.push("按住按钮语音转文字");
  if (hints.length === 0) return "输入问题";
  return `输入问题，${hints.join("，")}`;
});
const configDirty = computed(() => buildConfigSnapshotJson() !== lastSavedConfigJson.value);

function parseAssistantStoredText(rawText: string): {
  assistantText: string;
  reasoningStandard: string;
  reasoningInline: string;
} {
  const raw = rawText || "";
  const standardMarker = "[标准思考]";
  const standardIdx = raw.indexOf(standardMarker);

  if (standardIdx < 0) {
    return {
      assistantText: raw.trim(),
      reasoningStandard: "",
      reasoningInline: "",
    };
  }

  const reasoningStandard = raw.slice(standardIdx + standardMarker.length).trim();

  return {
    assistantText: raw.slice(0, standardIdx).trim(),
    reasoningStandard,
    reasoningInline: "",
  };
}

const allTurns = computed<ChatTurn[]>(() => {
  const msgs = allMessages.value;
  const turns: ChatTurn[] = [];
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    if (msg.role === "user") {
      const userText = removeBinaryPlaceholders(renderMessage(msg));
      const userImages = extractMessageImages(msg);
      const userAudios = extractMessageAudios(msg);
      let assistantText = "";
      let assistantReasoningStandard = "";
      let assistantReasoningInline = "";
      if (i + 1 < msgs.length && msgs[i + 1].role === "assistant") {
        const parsed = parseAssistantStoredText(renderMessage(msgs[i + 1]));
        assistantText = parsed.assistantText;
        assistantReasoningStandard = parsed.reasoningStandard;
        assistantReasoningInline = parsed.reasoningInline;
        i++;
      }
      if (userText || userImages.length > 0 || userAudios.length > 0 || assistantText.trim() || assistantReasoningStandard.trim() || assistantReasoningInline.trim()) {
        turns.push({
          id: msg.id,
          userText,
          userImages,
          userAudios,
          assistantText,
          assistantReasoningStandard,
          assistantReasoningInline,
        });
      }
    }
  }
  return turns;
});

const visibleTurns = computed(() =>
  allTurns.value.slice(Math.max(0, allTurns.value.length - visibleTurnCount.value))
);

const hasMoreTurns = computed(() => visibleTurnCount.value < allTurns.value.length);
const chatContextUsageRatio = computed(() => {
  const api = activeChatApiConfig.value;
  if (!api) return 0;
  const maxTokens = Math.max(16000, Math.min(200000, Number(api.contextWindowTokens ?? 128000)));
  const used = estimateConversationTokens(allMessages.value);
  return used / Math.max(1, maxTokens);
});
const chatUsagePercent = computed(() => Math.min(100, Math.max(0, Math.round(chatContextUsageRatio.value * 100))));

function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
  return {
    id: `api-config-${seed}`,
    name: `API Config ${config.apiConfigs.length + 1}`,
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: false,
    tools: defaultApiTools(),
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o-mini",
    temperature: 1,
    contextWindowTokens: 128000,
  };
}

function defaultApiTools() {
  return [
    {
      id: "fetch",
      command: "npx",
      args: ["-y", "@iflow-mcp/fetch"],
      values: {},
    },
    { id: "bing-search", command: "npx", args: ["-y", "bing-cn-mcp"], values: {} },
    { id: "memory-save", command: "builtin", args: ["memory-save"], values: {} },
  ];
}

function estimateTextTokens(text: string): number {
  let zh = 0;
  let other = 0;
  for (const ch of text || "") {
    if (/\s/.test(ch)) continue;
    if (/[\u3400-\u9fff\uf900-\ufaff]/.test(ch)) zh += 1;
    else other += 1;
  }
  return zh * 0.6 + other * 0.3;
}

function estimateConversationTokens(messages: ChatMessage[]): number {
  let total = 0;
  for (const m of messages) {
    total += 12;
    for (const p of m.parts || []) {
      if (p.type === "text") total += estimateTextTokens((p as { text?: string }).text || "");
      else if (p.type === "image") total += 280;
      else if (p.type === "audio") total += 320;
    }
  }
  return Math.ceil(total);
}

function buildConfigSnapshotJson(): string {
  return JSON.stringify({
    hotkey: config.hotkey,
    recordHotkey: config.recordHotkey,
    minRecordSeconds: config.minRecordSeconds,
    maxRecordSeconds: config.maxRecordSeconds,
    toolMaxIterations: config.toolMaxIterations,
    selectedApiConfigId: config.selectedApiConfigId,
    chatApiConfigId: config.chatApiConfigId,
    visionApiConfigId: config.visionApiConfigId,
    apiConfigs: config.apiConfigs.map((a) => ({
      id: a.id,
      name: a.name,
      requestFormat: a.requestFormat,
      enableText: a.enableText,
      enableImage: a.enableImage,
      enableAudio: a.enableAudio,
      enableTools: a.enableTools,
      tools: a.tools,
      baseUrl: a.baseUrl,
      apiKey: a.apiKey,
      model: a.model,
      temperature: a.temperature,
      contextWindowTokens: a.contextWindowTokens,
    })),
  });
}

function buildConfigPayload(): AppConfig {
  return {
    hotkey: config.hotkey,
    recordHotkey: config.recordHotkey,
    minRecordSeconds: config.minRecordSeconds,
    maxRecordSeconds: config.maxRecordSeconds,
    toolMaxIterations: config.toolMaxIterations,
    selectedApiConfigId: config.selectedApiConfigId,
    chatApiConfigId: config.chatApiConfigId,
    ...(config.visionApiConfigId ? { visionApiConfigId: config.visionApiConfigId } : {}),
    apiConfigs: config.apiConfigs.map((a) => ({
      id: a.id,
      name: a.name,
      requestFormat: a.requestFormat,
      enableText: !!a.enableText,
      enableImage: !!a.enableImage,
      enableAudio: !!a.enableAudio,
      enableTools: !!a.enableTools,
      tools: (a.tools || []).map((t) => ({
        id: t.id,
        command: t.command,
        args: Array.isArray(t.args) ? t.args : [],
        values: t.values ?? {},
      })),
      baseUrl: a.baseUrl,
      apiKey: a.apiKey,
      model: a.model,
      temperature: Number(a.temperature ?? 1),
      contextWindowTokens: Math.round(Number(a.contextWindowTokens ?? 128000)),
    })),
  };
}

function normalizeApiBindingsLocal() {
  if (!config.apiConfigs.length) return;
  for (const api of config.apiConfigs) {
    api.enableAudio = false;
    api.temperature = Math.max(0, Math.min(2, Number(api.temperature ?? 1)));
    api.contextWindowTokens = Math.max(16000, Math.min(200000, Math.round(Number(api.contextWindowTokens ?? 128000))));
  }
  if (!["Alt", "Ctrl", "Shift"].includes(config.recordHotkey)) {
    config.recordHotkey = "Alt";
  }
  config.minRecordSeconds = Math.max(1, Math.min(30, Math.round(Number(config.minRecordSeconds) || 1)));
  config.maxRecordSeconds = Math.max(config.minRecordSeconds, Math.round(Number(config.maxRecordSeconds) || 60));
  config.toolMaxIterations = Math.max(1, Math.min(100, Math.round(Number(config.toolMaxIterations) || 10)));
  if (!config.apiConfigs.some((a) => a.id === config.selectedApiConfigId)) {
    config.selectedApiConfigId = config.apiConfigs[0].id;
  }
  if (!config.apiConfigs.some((a) => a.id === config.chatApiConfigId && a.enableText)) {
    config.chatApiConfigId = textCapableApiConfigs.value[0]?.id ?? config.apiConfigs[0].id;
  }
  if (
    config.visionApiConfigId
    && !config.apiConfigs.some((a) => a.id === config.visionApiConfigId && a.enableImage)
  ) {
    config.visionApiConfigId = undefined;
  }
}

function renderMessage(msg: ChatMessage): string {
  return msg.parts.map((p) => {
    if (p.type === "text") return p.text;
    if (p.type === "image") return "[image]";
    return "[audio]";
  }).join("\n");
}

function messageText(msg: ChatMessage): string {
  return msg.parts
    .filter((p) => p.type === "text")
    .map((p) => p.text)
    .join("\n")
    .trim();
}

function removeBinaryPlaceholders(text: string): string {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "[image]" && line !== "[audio]")
    .join("\n")
    .trim();
}

function extractMessageImages(msg?: ChatMessage): Array<{ mime: string; bytesBase64: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "image")
    .map((p) => {
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      return {
        mime: anyPart.mime || "image/webp",
        bytesBase64: anyPart.bytesBase64 || anyPart.bytes_base64 || "",
      };
    })
    .filter((p) => !!p.bytesBase64);
}

function extractMessageAudios(msg?: ChatMessage): Array<{ mime: string; bytesBase64: string }> {
  if (!msg) return [];
  return msg.parts
    .filter((p) => p.type === "audio")
    .map((p) => {
      const anyPart = p as unknown as { mime?: string; bytesBase64?: string; bytes_base64?: string };
      return {
        mime: anyPart.mime || "audio/webm",
        bytesBase64: anyPart.bytesBase64 || anyPart.bytes_base64 || "",
      };
    })
    .filter((p) => !!p.bytesBase64);
}

async function loadConfig() {
  suppressAutosave.value = true;
  loading.value = true;
  status.value = "Loading config...";
  try {
    const cfg = await invoke<AppConfig>("load_config");
    config.hotkey = cfg.hotkey;
    config.recordHotkey = cfg.recordHotkey || "Alt";
    config.minRecordSeconds = Math.max(1, Math.min(30, Number(cfg.minRecordSeconds || 1)));
    config.maxRecordSeconds = Math.max(config.minRecordSeconds, Number(cfg.maxRecordSeconds || 60));
    config.toolMaxIterations = Math.max(1, Math.min(100, Number(cfg.toolMaxIterations || 10)));
    config.selectedApiConfigId = cfg.selectedApiConfigId;
    config.chatApiConfigId = cfg.chatApiConfigId;
    config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...(cfg.apiConfigs.length ? cfg.apiConfigs : [createApiConfig("default")]));
    normalizeApiBindingsLocal();
    lastSavedConfigJson.value = buildConfigSnapshotJson();
    status.value = "Config loaded.";
  } catch (e) {
    status.value = `Load failed: ${String(e)}`;
  } finally {
    suppressAutosave.value = false;
    loading.value = false;
  }
}

async function saveConfig() {
  suppressAutosave.value = true;
  saving.value = true;
  status.value = "Saving config...";
  try {
    console.info("[CONFIG] save_config invoked");
    const saved = await invoke<AppConfig>("save_config", { config: buildConfigPayload() });
    config.hotkey = saved.hotkey;
    config.recordHotkey = saved.recordHotkey || "Alt";
    config.minRecordSeconds = Math.max(1, Math.min(30, Number(saved.minRecordSeconds || 1)));
    config.maxRecordSeconds = Math.max(config.minRecordSeconds, Number(saved.maxRecordSeconds || 60));
    config.toolMaxIterations = Math.max(1, Math.min(100, Number(saved.toolMaxIterations || 10)));
    config.selectedApiConfigId = saved.selectedApiConfigId;
    config.chatApiConfigId = saved.chatApiConfigId;
    config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...saved.apiConfigs);
    normalizeApiBindingsLocal();
    lastSavedConfigJson.value = buildConfigSnapshotJson();
    console.info("[CONFIG] save_config success");
    status.value = "Config saved.";
  } catch (e) {
    const err = String(e);
    console.error("[CONFIG] save_config failed:", e);
    if (err.includes("404")) {
      status.value = "Save failed: 后端命令不可达（404）。请使用 `pnpm tauri dev` 启动桌面端，而不是仅 `pnpm dev`。";
    } else {
      status.value = `Save failed: ${err}`;
    }
  } finally {
    suppressAutosave.value = false;
    saving.value = false;
  }
}

async function loadPersonas() {
  suppressAutosave.value = true;
  try {
    const list = await invoke<PersonaProfile[]>("load_agents");
    personas.value = list;
    if (!personas.value.some((p) => p.id === selectedPersonaId.value)) selectedPersonaId.value = personas.value[0]?.id ?? "default-agent";
  } finally {
    suppressAutosave.value = false;
  }
}

async function loadChatSettings() {
  suppressAutosave.value = true;
  try {
    const settings = await invoke<{ selectedAgentId: string; userAlias: string }>("load_chat_settings");
    if (personas.value.some((p) => p.id === settings.selectedAgentId)) {
      selectedPersonaId.value = settings.selectedAgentId;
    }
    userAlias.value = settings.userAlias?.trim() || "用户";
  } finally {
    suppressAutosave.value = false;
  }
}

async function savePersonas() {
  suppressAutosave.value = true;
  try {
    personas.value = await invoke<PersonaProfile[]>("save_agents", { input: { agents: personas.value } });
    status.value = "人格已保存。";
  } catch (e) {
    status.value = `Save personas failed: ${String(e)}`;
  } finally {
    suppressAutosave.value = false;
  }
}

async function saveChatPreferences() {
  saving.value = true;
  status.value = "Saving chat settings...";
  try {
    await invoke("save_chat_settings", { input: { selectedAgentId: selectedPersonaId.value, userAlias: userAlias.value } });
    status.value = "Chat settings saved.";
  } catch (e) {
    status.value = `Save chat settings failed: ${String(e)}`;
  } finally {
    saving.value = false;
  }
}

async function saveConversationApiSettings() {
  if (suppressAutosave.value) return;
  try {
    console.info("[CONFIG] save_conversation_api_settings invoked");
    const saved = await invoke<{
      chatApiConfigId: string;
      visionApiConfigId?: string;
    }>("save_conversation_api_settings", {
      input: {
        chatApiConfigId: config.chatApiConfigId,
        visionApiConfigId: config.visionApiConfigId || null,
      },
    });
    config.chatApiConfigId = saved.chatApiConfigId;
    config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
    lastSavedConfigJson.value = buildConfigSnapshotJson();
    console.info("[CONFIG] save_conversation_api_settings success");
  } catch (e) {
    console.error("[CONFIG] save_conversation_api_settings failed:", e);
    status.value = `保存对话API设置失败: ${String(e)}`;
  }
}

function scheduleConfigAutosave() {
  // API 配置改为手动保存，保留函数占位避免大范围改动。
  return;
}

function schedulePersonasAutosave() {
  if (suppressAutosave.value) return;
  if (!personasAutosaveReady.value) return;
  if (personasAutosaveTimer) clearTimeout(personasAutosaveTimer);
  personasAutosaveTimer = setTimeout(() => {
    void savePersonas();
  }, 350);
}

function scheduleChatSettingsAutosave() {
  if (suppressAutosave.value) return;
  if (!chatSettingsAutosaveReady.value) return;
  if (chatSettingsAutosaveTimer) clearTimeout(chatSettingsAutosaveTimer);
  chatSettingsAutosaveTimer = setTimeout(() => {
    void saveChatPreferences();
  }, 350);
}

function addApiConfig() {
  const c = createApiConfig();
  config.apiConfigs.push(c);
  config.selectedApiConfigId = c.id;
  normalizeApiBindingsLocal();
}

function removeSelectedApiConfig() {
  if (config.apiConfigs.length <= 1) return;
  const idx = config.apiConfigs.findIndex((a) => a.id === config.selectedApiConfigId);
  if (idx >= 0) config.apiConfigs.splice(idx, 1);
  config.selectedApiConfigId = config.apiConfigs[0].id;
  normalizeApiBindingsLocal();
}

function addPersona() {
  const id = `persona-${Date.now()}`;
  const now = new Date().toISOString();
  personas.value.push({ id, name: `人格 ${personas.value.length + 1}`, systemPrompt: "", createdAt: now, updatedAt: now });
  selectedPersonaId.value = id;
}

function removeSelectedPersona() {
  if (personas.value.length <= 1) return;
  const idx = personas.value.findIndex((p) => p.id === selectedPersonaId.value);
  if (idx >= 0) personas.value.splice(idx, 1);
  selectedPersonaId.value = personas.value[0].id;
}

async function refreshModels() {
  if (!selectedApiConfig.value) return;
  refreshingModels.value = true;
  modelRefreshError.value = "";
  try {
    const models = await invoke<string[]>("refresh_models", { input: { baseUrl: selectedApiConfig.value.baseUrl, apiKey: selectedApiConfig.value.apiKey, requestFormat: selectedApiConfig.value.requestFormat } });
    apiModelOptions.value[selectedApiConfig.value.id] = models;
    if (models.length) selectedApiConfig.value.model = models[0];
    status.value = `Model list refreshed (${models.length}).`;
  } catch (e) {
    const err = String(e);
    modelRefreshError.value = err;
    status.value = `Refresh models failed: ${err}`;
  } finally {
    refreshingModels.value = false;
  }
}

async function refreshToolsStatus() {
  if (!toolApiConfig.value) return;
  checkingToolsStatus.value = true;
  try {
    toolStatuses.value = await invoke<ToolLoadStatus[]>("check_tools_status", {
      input: { apiConfigId: toolApiConfig.value.id },
    });
  } catch (e) {
    toolStatuses.value = [
      {
        id: "tools",
        status: "failed",
        detail: String(e),
      },
    ];
  } finally {
    checkingToolsStatus.value = false;
  }
}

async function refreshImageCacheStats() {
  imageCacheStatsLoading.value = true;
  try {
    imageCacheStats.value = await invoke<ImageTextCacheStats>("get_image_text_cache_stats");
  } catch (e) {
    status.value = `Load image cache stats failed: ${String(e)}`;
  } finally {
    imageCacheStatsLoading.value = false;
  }
}

async function clearImageCache() {
  imageCacheStatsLoading.value = true;
  try {
    imageCacheStats.value = await invoke<ImageTextCacheStats>("clear_image_text_cache");
    status.value = "图片转文缓存已清理。";
  } catch (e) {
    status.value = `Clear image cache failed: ${String(e)}`;
  } finally {
    imageCacheStatsLoading.value = false;
  }
}

async function refreshChatSnapshot() {
  if (!activeChatApiConfigId.value || !selectedPersonaId.value) return;
  try {
    const snap = await invoke<ChatSnapshot>("get_chat_snapshot", { input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedPersonaId.value } });
    latestUserText.value = snap.latestUser ? removeBinaryPlaceholders(renderMessage(snap.latestUser)) : "";
    latestUserImages.value = extractMessageImages(snap.latestUser);
    latestAssistantText.value = snap.latestAssistant ? renderMessage(snap.latestAssistant) : "";
  } catch (e) {
    status.value = `Load chat snapshot failed: ${String(e)}`;
  }
}

async function forceArchiveNow() {
  if (!activeChatApiConfigId.value || !selectedPersonaId.value) return;
  if (chatting.value || forcingArchive.value) return;
  forcingArchive.value = true;
  try {
    const result = await invoke<ForceArchiveResult>("force_archive_current", {
      input: {
        apiConfigId: activeChatApiConfigId.value,
        agentId: selectedPersonaId.value,
      },
    });
    status.value = result.archived
      ? `已强制归档，新增记忆 ${result.mergedMemories} 条。`
      : result.summary;
    await refreshChatSnapshot();
    await loadAllMessages();
    visibleTurnCount.value = 1;
  } catch (e) {
    status.value = `强制归档失败: ${String(e)}`;
  } finally {
    forcingArchive.value = false;
  }
}

async function loadAllMessages() {
  if (!activeChatApiConfigId.value || !selectedPersonaId.value) return;
  try {
    const msgs = await invoke<ChatMessage[]>("get_active_conversation_messages", {
      input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedPersonaId.value },
    });
    allMessages.value = msgs;
  } catch (e) {
    status.value = `Load messages failed: ${String(e)}`;
  }
}

function loadMoreTurns() {
  visibleTurnCount.value++;
}

let chatGeneration = 0;
type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
  toolName?: string;
  toolStatus?: "running" | "done" | "failed" | string;
  message?: string;
};
type MemoryEntry = {
  id: string;
  content: string;
  keywords: string[];
  createdAt: string;
  updatedAt: string;
};
type MemoryExportPayload = {
  version: number;
  exportedAt: string;
  memories: MemoryEntry[];
};
type ExportMemoriesFileResult = {
  path: string;
  count: number;
};
type ForceArchiveResult = {
  archived: boolean;
  archiveId?: string | null;
  summary: string;
  mergedMemories: number;
};
type ImportMemoriesResult = {
  importedCount: number;
  createdCount: number;
  mergedCount: number;
  totalCount: number;
};
const STREAM_FLUSH_INTERVAL_MS = 33;
const STREAM_DRAIN_TARGET_MS = 1000;
let streamPendingText = "";
let streamDrainDeadline = 0;
let streamFlushTimer: ReturnType<typeof setInterval> | null = null;
let reasoningStartedAtMs = 0;

function readDeltaMessage(message: unknown): string {
  if (typeof message === "string") return message;
  if (message && typeof message === "object" && "delta" in message) {
    const value = (message as { delta?: unknown }).delta;
    return typeof value === "string" ? value : "";
  }
  return "";
}

function readAssistantEvent(message: unknown): AssistantDeltaEvent {
  if (!message || typeof message !== "object") return {};
  const m = message as Record<string, unknown>;
  return {
    delta: typeof m.delta === "string" ? m.delta : undefined,
    kind: typeof m.kind === "string" ? m.kind : undefined,
    toolName: typeof m.toolName === "string" ? m.toolName : undefined,
    toolStatus: typeof m.toolStatus === "string" ? m.toolStatus : undefined,
    message: typeof m.message === "string" ? m.message : undefined,
  };
}

function clearStreamBuffer() {
  streamPendingText = "";
  streamDrainDeadline = 0;
  if (streamFlushTimer) {
    clearInterval(streamFlushTimer);
    streamFlushTimer = null;
  }
}

function flushStreamBuffer(gen: number) {
  if (gen !== chatGeneration) {
    clearStreamBuffer();
    return;
  }
  if (!streamPendingText) {
    if (!chatting.value) {
      clearStreamBuffer();
    }
    return;
  }
  const now = Date.now();
  const msLeft = Math.max(1, streamDrainDeadline - now);
  const ticksLeft = Math.max(1, Math.ceil(msLeft / STREAM_FLUSH_INTERVAL_MS));
  const step = Math.max(1, Math.ceil(streamPendingText.length / ticksLeft));
  latestAssistantText.value += streamPendingText.slice(0, step);
  streamPendingText = streamPendingText.slice(step);
}

function enqueueStreamDelta(gen: number, delta: string) {
  if (gen !== chatGeneration || !delta) return;
  streamPendingText += delta;
  streamDrainDeadline = Date.now() + STREAM_DRAIN_TARGET_MS;
  if (!streamFlushTimer) {
    streamFlushTimer = setInterval(() => flushStreamBuffer(gen), STREAM_FLUSH_INTERVAL_MS);
  }
}

function enqueueFinalAssistantText(gen: number, finalText: string) {
  if (gen !== chatGeneration) return;
  const text = finalText.trim();
  if (!text) return;
  const combined = `${latestAssistantText.value}${streamPendingText}`;
  if (!combined) {
    enqueueStreamDelta(gen, finalText);
    return;
  }
  if (text.startsWith(combined)) {
    const missing = text.slice(combined.length);
    if (missing) enqueueStreamDelta(gen, missing);
  }
}

async function sendChat() {
  if (chatting.value || forcingArchive.value) return;
  const text = chatInput.value.trim();
  if (!text && clipboardImages.value.length === 0) {
    return;
  }

  // 立刻刷新 UI：显示用户消息 + loading 气泡
  latestUserText.value = text;
  latestUserImages.value = [...clipboardImages.value];
  latestAssistantText.value = "";
  latestReasoningStandardText.value = "";
  latestReasoningInlineText.value = "";
  toolStatusText.value = "";
  toolStatusState.value = "";
  chatErrorText.value = "";

  const sentImages = [...clipboardImages.value];
  const sentModel = activeChatApiConfig.value?.model;
  chatInput.value = "";
  clipboardImages.value = [];

  const optimisticUserMessage: ChatMessage = {
    id: `optimistic-user-${Date.now()}`,
    role: "user",
    parts: [
      ...(text ? [{ type: "text" as const, text }] : []),
      ...sentImages.map((img) => ({
        type: "image" as const,
        mime: img.mime,
        bytesBase64: img.bytesBase64,
      })),
    ],
  };
  allMessages.value = [...allMessages.value, optimisticUserMessage];
  visibleTurnCount.value = 1;

  const gen = ++chatGeneration;
  clearStreamBuffer();
  const deltaChannel = new Channel<AssistantDeltaEvent>();
  deltaChannel.onmessage = (event) => {
    const parsed = readAssistantEvent(event);
    if (parsed.kind === "tool_status") {
      toolStatusText.value = parsed.message || "";
      toolStatusState.value = parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
        ? parsed.toolStatus
        : "";
      return;
    }
    if (parsed.kind === "reasoning_standard") {
      const text = readDeltaMessage(parsed);
      if (text && reasoningStartedAtMs === 0) reasoningStartedAtMs = Date.now();
      latestReasoningStandardText.value += text;
      return;
    }
    if (parsed.kind === "reasoning_inline") {
      const text = readDeltaMessage(parsed);
      if (text && reasoningStartedAtMs === 0) reasoningStartedAtMs = Date.now();
      latestReasoningInlineText.value += text;
      return;
    }
    const delta = readDeltaMessage(parsed);
    enqueueStreamDelta(gen, delta);
  };
  chatting.value = true;
  try {
    const result = await invoke<{ assistantText: string; latestUserText: string; archivedBeforeSend: boolean }>("send_chat_message", {
      input: {
        apiConfigId: activeChatApiConfigId.value,
        agentId: selectedPersonaId.value,
        payload: { text, images: sentImages, model: sentModel },
      },
      onDelta: deltaChannel,
    });
    if (gen !== chatGeneration) return;
    latestUserText.value = removeBinaryPlaceholders(result.latestUserText);
    latestUserImages.value = sentImages;
    enqueueFinalAssistantText(gen, result.assistantText);
    chatErrorText.value = "";
    if (toolStatusState.value === "running") {
      toolStatusState.value = "done";
      toolStatusText.value = "工具调用完成";
    }
    await loadAllMessages();
  } catch (e) {
    if (gen !== chatGeneration) return;
    clearStreamBuffer();
    latestAssistantText.value = "";
    latestReasoningStandardText.value = "";
    latestReasoningInlineText.value = "";
    chatErrorText.value = `请求失败：${String(e)}`;
    if (!toolStatusText.value) {
      toolStatusState.value = "failed";
      toolStatusText.value = "工具调用失败";
    }
    await loadAllMessages();
  } finally {
    if (gen === chatGeneration) {
      chatting.value = false;
      reasoningStartedAtMs = 0;
    }
  }
}

function stopChat() {
  chatGeneration++;
  clearStreamBuffer();
  chatting.value = false;
  latestAssistantText.value = "(已中断)";
  latestReasoningStandardText.value = "";
  latestReasoningInlineText.value = "";
  reasoningStartedAtMs = 0;
  toolStatusText.value = "";
  toolStatusState.value = "";
}

async function openCurrentHistory() {
  try {
    currentHistory.value = await invoke<ChatMessage[]>("get_active_conversation_messages", { input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedPersonaId.value } });
    historyDialog.value?.showModal();
  } catch (e) {
    status.value = `Load history failed: ${String(e)}`;
  }
}

function closeHistory() {
  historyDialog.value?.close();
}

async function openMemoryViewer() {
  try {
    memoryList.value = await invoke<MemoryEntry[]>("list_memories");
    memoryPage.value = 1;
    memoryDialog.value?.showModal();
  } catch (e) {
    status.value = `Load memories failed: ${String(e)}`;
  }
}

function closeMemoryViewer() {
  memoryDialog.value?.close();
}

async function exportMemories() {
  try {
    const path = await save({
      defaultPath: `easy-call-ai-memories-${new Date().toISOString().replace(/[:.]/g, "-")}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      status.value = "导出已取消。";
      return;
    }
    const result = await invoke<ExportMemoriesFileResult>("export_memories_to_path", {
      input: { path },
    });
    status.value = `记忆已导出（${result.count} 条）：${result.path}`;
  } catch (e) {
    status.value = `导出记忆失败: ${String(e)}`;
  }
}

function triggerMemoryImport() {
  if (memoryImportInput.value) {
    memoryImportInput.value.value = "";
    memoryImportInput.value.click();
  }
}

async function handleMemoryImportFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    const raw = await file.text();
    const parsed = JSON.parse(raw) as unknown;
    const memories = Array.isArray(parsed)
      ? parsed
      : (parsed && typeof parsed === "object" && Array.isArray((parsed as { memories?: unknown }).memories))
        ? (parsed as { memories: unknown[] }).memories
        : [];
    if (!Array.isArray(memories)) {
      throw new Error("invalid memories payload");
    }

    const result = await invoke<ImportMemoriesResult>("import_memories", {
      input: { memories },
    });
    memoryList.value = await invoke<MemoryEntry[]>("list_memories");
    memoryPage.value = 1;
    status.value = `导入完成：新增 ${result.createdCount}，合并 ${result.mergedCount}，总计 ${result.totalCount}。`;
  } catch (e) {
    status.value = `导入记忆失败: ${String(e)}`;
  } finally {
    input.value = "";
  }
}

async function loadArchives() {
  try {
    archives.value = await invoke<ArchiveSummary[]>("list_archives");
    if (archives.value.length > 0) await selectArchive(archives.value[0].archiveId);
  } catch (e) {
    status.value = `Load archives failed: ${String(e)}`;
  }
}

async function selectArchive(archiveId: string) {
  archiveMessages.value = await invoke<ChatMessage[]>("get_archive_messages", { archiveId });
}

function onPaste(event: ClipboardEvent) {
  if (viewMode.value !== "chat") return;
  if (chatting.value || forcingArchive.value) return;
  const items = event.clipboardData?.items;
  if (!items) return;
  const apiConfig = activeChatApiConfig.value;
  if (!apiConfig) return;

  const text = event.clipboardData?.getData("text/plain");
  if (text && !chatInput.value.trim() && apiConfig.enableText) chatInput.value = text;

  for (const item of Array.from(items)) {
    if (item.type.startsWith("image/")) {
      if (!apiConfig.enableImage && !hasVisionFallback.value) {
        event.preventDefault();
        return;
      }
      const file = item.getAsFile();
      if (!file) continue;
      const reader = new FileReader();
      reader.onload = () => {
        const result = String(reader.result || "");
        const base64 = result.includes(",") ? result.split(",")[1] : "";
        if (base64) clipboardImages.value.push({ mime: item.type, bytesBase64: base64 });
      };
      reader.readAsDataURL(file);
      event.preventDefault();
    }
  }
}

function removeClipboardImage(index: number) {
  if (index < 0 || index >= clipboardImages.value.length) return;
  clipboardImages.value.splice(index, 1);
}

async function readBlobAsDataUrl(blob: Blob): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function clearHotkeyTestTimers() {
  if (hotkeyTestTickTimer) {
    clearInterval(hotkeyTestTickTimer);
    hotkeyTestTickTimer = null;
  }
}

function stopHotkeyTestStream() {
  if (hotkeyTestStream) {
    for (const track of hotkeyTestStream.getTracks()) track.stop();
    hotkeyTestStream = null;
  }
}

async function startHotkeyRecordTest() {
  if (hotkeyTestRecording.value) return;
  if (recording.value) return;
  if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
    status.value = "当前环境不支持录音。";
    return;
  }
  try {
    hotkeyTestStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    hotkeyTestRecorder = new MediaRecorder(hotkeyTestStream);
    const chunks: BlobPart[] = [];
    hotkeyTestRecorder.ondataavailable = (event: BlobEvent) => {
      if (event.data && event.data.size > 0) chunks.push(event.data);
    };
    hotkeyTestRecorder.onstop = async () => {
      const durationMs = Math.max(0, Date.now() - hotkeyTestStartedAt);
      hotkeyTestRecording.value = false;
      clearHotkeyTestTimers();
      stopHotkeyTestStream();
      if (chunks.length === 0) return;
      const blob = new Blob(chunks, { type: hotkeyTestRecorder?.mimeType || "audio/webm" });
      const dataUrl = await readBlobAsDataUrl(blob);
      const base64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
      if (!base64) return;
      hotkeyTestAudio.value = {
        mime: blob.type || "audio/webm",
        bytesBase64: base64,
        durationMs,
      };
      status.value = `录音测试完成（${Math.max(1, Math.round(durationMs / 1000))}s）。`;
    };
    hotkeyTestRecorder.start();
    hotkeyTestStartedAt = Date.now();
    hotkeyTestRecording.value = true;
    hotkeyTestRecordingMs.value = 0;
    clearHotkeyTestTimers();
    hotkeyTestTickTimer = setInterval(() => {
      hotkeyTestRecordingMs.value = Math.max(0, Date.now() - hotkeyTestStartedAt);
    }, 100);
  } catch (e) {
    hotkeyTestRecording.value = false;
    clearHotkeyTestTimers();
    stopHotkeyTestStream();
    status.value = `录音测试失败: ${String(e)}`;
  }
}

async function stopHotkeyRecordTest() {
  if (!hotkeyTestRecording.value) return;
  if (hotkeyTestRecorder && hotkeyTestRecorder.state !== "inactive") {
    hotkeyTestRecorder.stop();
  } else {
    hotkeyTestRecording.value = false;
    clearHotkeyTestTimers();
    stopHotkeyTestStream();
  }
}

function playHotkeyRecordTest() {
  if (!hotkeyTestAudio.value) return;
  if (hotkeyTestPlayer) {
    hotkeyTestPlayer.pause();
    hotkeyTestPlayer.currentTime = 0;
    hotkeyTestPlayer = null;
  }
  const src = `data:${hotkeyTestAudio.value.mime};base64,${hotkeyTestAudio.value.bytesBase64}`;
  hotkeyTestPlayer = new Audio(src);
  void hotkeyTestPlayer.play().catch(() => {
    hotkeyTestPlayer = null;
  });
}

function clearRecordingTimers() {
  if (recordingTickTimer) {
    clearInterval(recordingTickTimer);
    recordingTickTimer = null;
  }
  if (recordingMaxTimer) {
    clearTimeout(recordingMaxTimer);
    recordingMaxTimer = null;
  }
  if (recordingLevelTimer) {
    clearInterval(recordingLevelTimer);
    recordingLevelTimer = null;
  }
}

function stopRecordingLevelMonitor() {
  recordingLevelSupported = false;
  if (recordingSourceNode) {
    try {
      recordingSourceNode.disconnect();
    } catch {
      // ignore
    }
    recordingSourceNode = null;
  }
  if (recordingAnalyser) {
    try {
      recordingAnalyser.disconnect();
    } catch {
      // ignore
    }
    recordingAnalyser = null;
  }
  if (recordingAudioContext) {
    void recordingAudioContext.close().catch(() => {});
    recordingAudioContext = null;
  }
}

function startRecordingLevelMonitor(stream: MediaStream) {
  stopRecordingLevelMonitor();
  recordingLevelSupported = false;
  recordingPeakLevel = 0;
  const Ctx = window.AudioContext || (window as typeof window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctx) return;
  recordingAudioContext = new Ctx();
  recordingSourceNode = recordingAudioContext.createMediaStreamSource(stream);
  recordingAnalyser = recordingAudioContext.createAnalyser();
  recordingLevelSupported = true;
  recordingAnalyser.fftSize = 1024;
  recordingSourceNode.connect(recordingAnalyser);
  const data = new Float32Array(recordingAnalyser.fftSize);
  recordingLevelTimer = setInterval(() => {
    if (!recordingAnalyser) return;
    recordingAnalyser.getFloatTimeDomainData(data);
    let sum = 0;
    for (let i = 0; i < data.length; i += 1) {
      const v = data[i];
      sum += v * v;
    }
    const rms = Math.sqrt(sum / data.length);
    recordingPeakLevel = Math.max(recordingPeakLevel, rms);
  }, 100);
}

function matchesRecordHotkey(event: KeyboardEvent): boolean {
  if (config.recordHotkey === "Alt") return event.key === "Alt";
  if (config.recordHotkey === "Ctrl") return event.key === "Control";
  if (config.recordHotkey === "Shift") return event.key === "Shift";
  return false;
}

async function startRecording() {
  if (recording.value || chatting.value || forcingArchive.value) return;
  const w = window as typeof window & {
    SpeechRecognition?: new () => SpeechRecognitionLike;
    webkitSpeechRecognition?: new () => SpeechRecognitionLike;
  };
  const SR = w.SpeechRecognition || w.webkitSpeechRecognition;
  if (!SR) {
    status.value = "当前环境不支持本地语音识别。";
    return;
  }
  try {
    recordingDiscardCurrent = false;
    recordingRecognizedText = "";
    const recognizer = new SR();
    speechRecognizer = recognizer;
    recognizer.lang = "zh-CN";
    recognizer.interimResults = true;
    recognizer.continuous = true;
    recognizer.onresult = (event) => {
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const item = event.results[i];
        const transcript = (item?.[0]?.transcript || "").trim();
        if (item?.isFinal && transcript) {
          recordingRecognizedText += `${transcript}\n`;
        }
      }
    };
    recognizer.onerror = (event) => {
      status.value = `本地语音识别失败: ${event?.error || "unknown"}`;
    };
    recognizer.onend = () => {
      recording.value = false;
      clearRecordingTimers();
      if (recordingDiscardCurrent) {
        speechRecognizer = null;
        return;
      }
      const text = recordingRecognizedText.trim();
      if (text) {
        chatInput.value = chatInput.value.trim() ? `${chatInput.value.trim()}\n${text}` : text;
        status.value = "录音已转文字。";
      } else {
        status.value = "未识别到文本。";
      }
      speechRecognizer = null;
      recordingRecognizedText = "";
    };
    recognizer.start();
    recordingStartedAt = Date.now();
    recording.value = true;
    recordingMs.value = 0;
    recordingTickTimer = setInterval(() => {
      recordingMs.value = Math.max(0, Date.now() - recordingStartedAt);
    }, 100);
    recordingMaxTimer = setTimeout(() => {
      void stopRecording(false);
      status.value = `录音已达到上限 ${config.maxRecordSeconds}s，自动停止。`;
    }, config.maxRecordSeconds * 1000);
  } catch (e) {
    recording.value = false;
    clearRecordingTimers();
    status.value = `开始录音失败: ${String(e)}`;
  }
}

async function stopRecording(discard: boolean) {
  if (!recording.value) return;
  recordingDiscardCurrent = discard;
  speechRecognizer?.stop();
}

async function importClipboardImageOnOpen() {
  if (viewMode.value !== "chat") return;
  const apiConfig = activeChatApiConfig.value;
  if (!apiConfig) return;

  if (apiConfig.enableText && !chatInput.value.trim() && navigator.clipboard?.readText) {
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (text) {
        chatInput.value = text;
      }
    } catch {
      // Ignore clipboard text read errors.
    }
  }

  if (!apiConfig.enableImage && !hasVisionFallback.value) return;
  if (!navigator.clipboard?.read) return;

  try {
    const items = await navigator.clipboard.read();
    for (const item of items) {
      const imageType = item.types.find((t) => t.startsWith("image/"));
      if (!imageType) continue;
      const blob = await item.getType(imageType);
      const dataUrl = await readBlobAsDataUrl(blob);
      const base64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
      if (!base64) continue;

      const exists = clipboardImages.value.some(
        (img) => img.mime === imageType && img.bytesBase64 === base64,
      );
      if (!exists) {
        clipboardImages.value.push({ mime: imageType, bytesBase64: base64 });
      }
      break;
    }
  } catch {
    // Clipboard read can fail depending on platform permissions; ignore silently.
  }
}

async function closeWindow() { 
  if (!appWindow) return;
  await appWindow.hide(); 
}
async function startDrag() { 
  if (!appWindow) return;
  await appWindow.startDragging(); 
}
async function toggleAlwaysOnTop() {
  if (!appWindow) return;
  alwaysOnTop.value = !alwaysOnTop.value;
  await appWindow.setAlwaysOnTop(alwaysOnTop.value);
}

function applyTheme(theme: "light" | "forest") {
  currentTheme.value = theme;
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("theme", theme);
}

function toggleTheme() {
  const next = currentTheme.value === "light" ? "forest" : "light";
  applyTheme(next);
  emit("easy-call:theme-changed", next);
}

onMounted(async () => {
  appWindow = getCurrentWindow();
  viewMode.value = appWindow.label === "chat" ? "chat" : appWindow.label === "archives" ? "archives" : "config";
  windowReady.value = true;

  // 从 localStorage 恢复主题
  const savedTheme = localStorage.getItem("theme") as "light" | "forest" | null;
  if (savedTheme) applyTheme(savedTheme);

  // 监听其他窗口的主题变更
  await listen<string>("easy-call:theme-changed", (event) => {
    applyTheme(event.payload as "light" | "forest");
  });

  window.addEventListener("paste", onPaste);
  keydownHandler = (event: KeyboardEvent) => {
    if (viewMode.value !== "chat") return;
    if (!matchesRecordHotkey(event)) return;
    if (event.repeat) return;
    event.preventDefault();
    void startRecording();
  };
  keyupHandler = (event: KeyboardEvent) => {
    if (viewMode.value !== "chat") return;
    if (!matchesRecordHotkey(event)) return;
    event.preventDefault();
    void stopRecording(false);
  };
  window.addEventListener("keydown", keydownHandler);
  window.addEventListener("keyup", keyupHandler);
  const refreshAll = async () => {
    await loadConfig();
    await loadPersonas();
    await loadChatSettings();
    if (viewMode.value === "config") {
      await refreshImageCacheStats();
    }
    if (viewMode.value === "chat") {
      await refreshChatSnapshot();
      await loadAllMessages();
      visibleTurnCount.value = 1;
      await importClipboardImageOnOpen();
    } else if (viewMode.value === "archives") {
      await loadArchives();
    }
  };

  await refreshAll();
  configAutosaveReady.value = true;
  personasAutosaveReady.value = true;
  chatSettingsAutosaveReady.value = true;
  if (viewMode.value === "chat") {
    try {
      alwaysOnTop.value = await appWindow.isAlwaysOnTop();
    } catch {
      alwaysOnTop.value = false;
    }
  }
  await listen("easy-call:refresh", async () => {
    configAutosaveReady.value = false;
    personasAutosaveReady.value = false;
    chatSettingsAutosaveReady.value = false;
    await refreshAll();
    configAutosaveReady.value = true;
    personasAutosaveReady.value = true;
    chatSettingsAutosaveReady.value = true;
  });

});

onBeforeUnmount(() => {
  clearStreamBuffer();
  void stopRecording(true);
  speechRecognizer?.stop();
  speechRecognizer = null;
  void stopHotkeyRecordTest();
  if (hotkeyTestPlayer) {
    hotkeyTestPlayer.pause();
    hotkeyTestPlayer.currentTime = 0;
    hotkeyTestPlayer = null;
  }
  stopRecordingLevelMonitor();
  if (keydownHandler) window.removeEventListener("keydown", keydownHandler);
  if (keyupHandler) window.removeEventListener("keyup", keyupHandler);
});

watch(
  () => ({
    hotkey: config.hotkey,
    recordHotkey: config.recordHotkey,
    minRecordSeconds: config.minRecordSeconds,
    maxRecordSeconds: config.maxRecordSeconds,
    selectedApiConfigId: config.selectedApiConfigId,
    chatApiConfigId: config.chatApiConfigId,
    visionApiConfigId: config.visionApiConfigId,
    apiConfigs: config.apiConfigs.map((a) => ({
      id: a.id,
      name: a.name,
      requestFormat: a.requestFormat,
      enableText: a.enableText,
      enableImage: a.enableImage,
      enableAudio: a.enableAudio,
      enableTools: a.enableTools,
      tools: a.tools,
      baseUrl: a.baseUrl,
      apiKey: a.apiKey,
      model: a.model,
      temperature: a.temperature,
      contextWindowTokens: a.contextWindowTokens,
    })),
  }),
  () => { /* 手动保存模式，不自动持久化 API 配置 */ },
  { deep: true },
);

watch(
  () => config.apiConfigs.map((a) => ({
    id: a.id,
    requestFormat: a.requestFormat,
    enableText: a.enableText,
    enableImage: a.enableImage,
    enableAudio: a.enableAudio,
    enableTools: a.enableTools,
    temperature: a.temperature,
    contextWindowTokens: a.contextWindowTokens,
  })),
  () => normalizeApiBindingsLocal(),
  { deep: true },
);

watch(
  () => personas.value.map((p) => ({
    id: p.id,
    name: p.name,
    systemPrompt: p.systemPrompt,
    createdAt: p.createdAt,
    updatedAt: p.updatedAt,
  })),
  () => schedulePersonasAutosave(),
  { deep: true },
);

watch(
  () => ({ selectedPersonaId: selectedPersonaId.value, userAlias: userAlias.value }),
  () => scheduleChatSettingsAutosave(),
);

watch(
  () => ({
    chatApiConfigId: config.chatApiConfigId,
    visionApiConfigId: config.visionApiConfigId,
  }),
  () => {
    void saveConversationApiSettings();
  },
);

watch(
  () => config.selectedApiConfigId,
  () => {
    modelRefreshError.value = "";
  },
);

watch(
  () => selectedApiConfig.value?.enableTools,
  (enabled) => {
    if (!enabled || !selectedApiConfig.value) return;
    if (selectedApiConfig.value.tools.length === 0) {
      selectedApiConfig.value.tools = defaultApiTools();
    }
  },
);

watch(
  () => [configTab.value, activeChatApiConfigId.value, toolApiConfig.value?.enableTools],
  async ([tab, id, enabled]) => {
    if (tab !== "tools") return;
    if (!id) return;
    if (!enabled) {
      toolStatuses.value = (toolApiConfig.value?.tools ?? []).map((t) => ({
        id: t.id,
        status: "disabled",
        detail: "当前对话AI未启用工具调用。",
      }));
      return;
    }
    await refreshToolsStatus();
  },
);

watch(
  () => configTab.value,
  async (tab) => {
    if (tab !== "chatSettings") return;
    await refreshImageCacheStats();
  },
);

watch(
  () => activeChatApiConfigId.value,
  async (_newId, oldId) => {
    if (_newId === oldId) return;
    if (viewMode.value !== "chat") return;
    await refreshChatSnapshot();
    await loadAllMessages();
    visibleTurnCount.value = 1;
  },
);
</script>

