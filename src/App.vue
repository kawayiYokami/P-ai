
<template>
  <div class="window-shell text-sm bg-base-200">
    <div class="navbar min-h-10 h-10 px-2 relative cursor-move select-none" :class="viewMode === 'chat' ? '' : 'bg-base-200 border-b border-base-300'" @mousedown.left.prevent="startDrag">
      <div class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex items-center px-2">
        <span class="font-semibold text-sm">{{ titleText }}</span>
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
          <button class="btn btn-ghost btn-xs" title="Minimize" @click.stop="minimizeWindow" :disabled="!windowReady"><Minus class="h-3.5 w-3.5" /></button>
          <button class="btn btn-ghost btn-xs" title="Maximize" @click.stop="toggleMaximize" :disabled="!windowReady"><Square class="h-3.5 w-3.5" /></button>
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
        :base-url-reference="baseUrlReference"
        :refreshing-models="refreshingModels"
        :checking-tools-status="checkingToolsStatus"
        :model-options="selectedModelOptions"
        :tool-statuses="toolStatuses"
        :agents="agents"
        :selected-agent-id="selectedAgentId"
        :selected-agent="selectedAgent"
        :user-alias="userAlias"
        :text-capable-api-configs="textCapableApiConfigs"
        :image-capable-api-configs="imageCapableApiConfigs"
        :audio-capable-api-configs="audioCapableApiConfigs"
        :cache-stats="imageCacheStats"
        :cache-stats-loading="imageCacheStatsLoading"
        @update:config-tab="configTab = $event"
        @update:selected-agent-id="selectedAgentId = $event"
        @update:user-alias="userAlias = $event"
        @toggle-theme="toggleTheme"
        @refresh-models="refreshModels"
        @refresh-tools-status="refreshToolsStatus"
        @add-api-config="addApiConfig"
        @remove-selected-api-config="removeSelectedApiConfig"
        @add-agent="addAgent"
        @remove-selected-agent="removeSelectedAgent"
        @open-current-history="openCurrentHistory"
        @refresh-image-cache-stats="refreshImageCacheStats"
        @clear-image-cache="clearImageCache"
      />

      <ChatView
        v-else-if="viewMode === 'chat'"
        :user-alias="userAlias"
        :agent-name="selectedAgent?.name || '助理'"
        :latest-user-text="latestUserText"
        :latest-user-images="latestUserImages"
        :latest-assistant-text="latestAssistantText"
        :clipboard-images="clipboardImages"
        :clipboard-audios="clipboardAudios"
        :chat-input="chatInput"
        :chat-input-placeholder="chatInputPlaceholder"
        :can-record="activeChatApiConfig?.enableAudio || hasSttFallback"
        :recording="recording"
        :recording-ms="recordingMs"
        :record-hotkey="config.recordHotkey"
        :chatting="chatting"
        :turns="visibleTurns"
        :has-more-turns="hasMoreTurns"
        @update:chat-input="chatInput = $event"
        @remove-clipboard-image="removeClipboardImage"
        @remove-clipboard-audio="removeClipboardAudio"
        @start-recording="startRecording"
        @stop-recording="stopRecording(false)"
        @send-chat="sendChat"
        @stop-chat="stopChat"
        @load-more-turns="loadMoreTurns"
      />

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
                  class="rounded max-h-32 object-contain bg-base-100/40"
                />
              </div>
            </div>
          </div>
          <div class="modal-action"><button class="btn btn-sm" @click="closeHistory">关闭</button></div>
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
import { Minus, Pin, Square, X } from "lucide-vue-next";
import ConfigView from "./views/ConfigView.vue";
import ChatView from "./views/ChatView.vue";
import ArchivesView from "./views/ArchivesView.vue";
import type {
  AgentProfile,
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatMessage,
  ChatSettings,
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
  selectedApiConfigId: "",
  chatApiConfigId: "",
  sttApiConfigId: undefined,
  visionApiConfigId: undefined,
  apiConfigs: [],
});
const configTab = ref<"hotkey" | "api" | "tools" | "agent" | "chatSettings">("hotkey");
const currentTheme = ref<"light" | "forest">("light");
const agents = ref<AgentProfile[]>([]);
const selectedAgentId = ref("default-agent");
const userAlias = ref("用户");
const chatInput = ref("");
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const currentHistory = ref<ChatMessage[]>([]);
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const clipboardAudios = ref<Array<{ mime: string; bytesBase64: string; durationMs: number }>>([]);
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
const refreshingModels = ref(false);
const checkingToolsStatus = ref(false);
const toolStatuses = ref<ToolLoadStatus[]>([]);
const imageCacheStats = ref<ImageTextCacheStats>({ entries: 0, totalChars: 0 });
const imageCacheStatsLoading = ref(false);
const apiModelOptions = ref<Record<string, string[]>>({});
const historyDialog = ref<HTMLDialogElement | null>(null);
const alwaysOnTop = ref(false);
const configAutosaveReady = ref(false);
const agentsAutosaveReady = ref(false);
const chatSettingsAutosaveReady = ref(false);
const suppressAutosave = ref(false);
let configAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let agentsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let chatSettingsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let mediaRecorder: MediaRecorder | null = null;
let mediaStream: MediaStream | null = null;
let recordingStartedAt = 0;
let recordingTickTimer: ReturnType<typeof setInterval> | null = null;
let recordingMaxTimer: ReturnType<typeof setTimeout> | null = null;
let recordingDiscardCurrent = false;
let keydownHandler: ((event: KeyboardEvent) => void) | null = null;
let keyupHandler: ((event: KeyboardEvent) => void) | null = null;

const titleText = computed(() => {
  if (viewMode.value === "chat") {
    return `与 ${selectedAgent.value?.name || "助理"} 的对话`;
  }
  if (viewMode.value === "archives") {
    return "Easy Call AI - 归档窗口";
  }
  return "Easy Call AI - 配置窗口";
});
const selectedApiConfig = computed(() => config.apiConfigs.find((a) => a.id === config.selectedApiConfigId) ?? null);
const textCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableText));
const imageCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableImage));
const audioCapableApiConfigs = computed(() =>
  config.apiConfigs.filter((a) => a.enableAudio && a.requestFormat === "openai_tts"),
);
const activeChatApiConfigId = computed(
  () => config.chatApiConfigId || textCapableApiConfigs.value[0]?.id || config.apiConfigs[0]?.id || "",
);
const activeChatApiConfig = computed(
  () => config.apiConfigs.find((a) => a.id === activeChatApiConfigId.value) ?? null,
);
const hasVisionFallback = computed(() =>
  !!config.visionApiConfigId
  && config.apiConfigs.some((a) => a.id === config.visionApiConfigId && a.enableImage),
);
const hasSttFallback = computed(() =>
  !!config.sttApiConfigId
  && config.apiConfigs.some(
    (a) => a.id === config.sttApiConfigId && a.enableAudio && a.requestFormat === "openai_tts",
  ),
);
const selectedAgent = computed(() => agents.value.find((a) => a.id === selectedAgentId.value) ?? null);
const selectedModelOptions = computed(() => {
  const id = config.selectedApiConfigId;
  if (!id) return [];
  return apiModelOptions.value[id] ?? [];
});
const baseUrlReference = computed(() => {
  const format = selectedApiConfig.value?.requestFormat ?? "openai";
  if (format === "gemini") return "https://generativelanguage.googleapis.com/v1beta/openai";
  if (format === "deepseek/kimi") return "https://api.deepseek.com/v1";
  if (format === "openai_tts") return "https://api.siliconflow.cn/v1/audio/transcriptions";
  return "https://api.openai.com/v1";
});
const chatInputPlaceholder = computed(() => {
  const api = activeChatApiConfig.value;
  if (!api) return "输入问题";
  const hints: string[] = [];
  if (api.enableImage || hasVisionFallback.value) hints.push("Ctrl+V 粘贴图片");
  if (api.enableAudio || hasSttFallback.value) hints.push("可发送语音");
  if (hints.length === 0) return "输入问题";
  return `输入问题，${hints.join("，")}`;
});

const allTurns = computed<ChatTurn[]>(() => {
  const msgs = allMessages.value;
  const turns: ChatTurn[] = [];
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    if (msg.role === "user") {
      const userText = removeBinaryPlaceholders(renderMessage(msg));
      const userImages = extractMessageImages(msg);
      let assistantText = "";
      if (i + 1 < msgs.length && msgs[i + 1].role === "assistant") {
        assistantText = renderMessage(msgs[i + 1]);
        i++;
      }
      if (userText || userImages.length > 0 || assistantText.trim()) {
        turns.push({ id: msg.id, userText, userImages, assistantText });
      }
    }
  }
  return turns;
});

const visibleTurns = computed(() =>
  allTurns.value.slice(Math.max(0, allTurns.value.length - visibleTurnCount.value))
);

const hasMoreTurns = computed(() => visibleTurnCount.value < allTurns.value.length);

function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
  return {
    id: `api-config-${seed}`,
    name: `API Config ${config.apiConfigs.length + 1}`,
    requestFormat: "openai",
    enableText: true,
    enableImage: true,
    enableAudio: true,
    enableTools: false,
    tools: defaultApiTools(),
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o-mini",
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
  ];
}

function normalizeApiBindingsLocal() {
  if (!config.apiConfigs.length) return;
  if (!["Alt", "Ctrl", "Shift"].includes(config.recordHotkey)) {
    config.recordHotkey = "Alt";
  }
  config.minRecordSeconds = Math.max(1, Math.min(30, Math.round(Number(config.minRecordSeconds) || 1)));
  config.maxRecordSeconds = Math.max(config.minRecordSeconds, Math.round(Number(config.maxRecordSeconds) || 60));
  if (!config.apiConfigs.some((a) => a.id === config.selectedApiConfigId)) {
    config.selectedApiConfigId = config.apiConfigs[0].id;
  }
  if (!config.apiConfigs.some((a) => a.id === config.chatApiConfigId && a.enableText)) {
    config.chatApiConfigId = textCapableApiConfigs.value[0]?.id ?? config.selectedApiConfigId;
  }
  if (
    config.sttApiConfigId
    && !config.apiConfigs.some(
      (a) => a.id === config.sttApiConfigId && a.enableAudio && a.requestFormat === "openai_tts",
    )
  ) {
    config.sttApiConfigId = undefined;
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
    .map((p) => ({ mime: p.mime, bytesBase64: p.bytesBase64 }));
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
    config.selectedApiConfigId = cfg.selectedApiConfigId;
    config.chatApiConfigId = cfg.chatApiConfigId;
    config.sttApiConfigId = cfg.sttApiConfigId ?? undefined;
    config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...(cfg.apiConfigs.length ? cfg.apiConfigs : [createApiConfig("default")]));
    normalizeApiBindingsLocal();
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
    const saved = await invoke<AppConfig>("save_config", { config: { ...config } });
    config.hotkey = saved.hotkey;
    config.recordHotkey = saved.recordHotkey || "Alt";
    config.minRecordSeconds = Math.max(1, Math.min(30, Number(saved.minRecordSeconds || 1)));
    config.maxRecordSeconds = Math.max(config.minRecordSeconds, Number(saved.maxRecordSeconds || 60));
    config.selectedApiConfigId = saved.selectedApiConfigId;
    config.chatApiConfigId = saved.chatApiConfigId;
    config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
    config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...saved.apiConfigs);
    normalizeApiBindingsLocal();
    status.value = "Config saved.";
  } catch (e) {
    status.value = `Save failed: ${String(e)}`;
  } finally {
    suppressAutosave.value = false;
    saving.value = false;
  }
}

async function loadAgents() {
  suppressAutosave.value = true;
  try {
    const list = await invoke<AgentProfile[]>("load_agents");
    agents.value = list;
    if (!agents.value.some((a) => a.id === selectedAgentId.value)) selectedAgentId.value = agents.value[0]?.id ?? "default-agent";
  } finally {
    suppressAutosave.value = false;
  }
}

async function loadChatSettings() {
  suppressAutosave.value = true;
  try {
    const settings = await invoke<ChatSettings>("load_chat_settings");
    if (agents.value.some((a) => a.id === settings.selectedAgentId)) {
      selectedAgentId.value = settings.selectedAgentId;
    }
    userAlias.value = settings.userAlias?.trim() || "用户";
  } finally {
    suppressAutosave.value = false;
  }
}

async function saveAgents() {
  suppressAutosave.value = true;
  try {
    agents.value = await invoke<AgentProfile[]>("save_agents", { input: { agents: agents.value } });
    status.value = "Agents saved.";
  } catch (e) {
    status.value = `Save agents failed: ${String(e)}`;
  } finally {
    suppressAutosave.value = false;
  }
}

async function saveChatPreferences() {
  saving.value = true;
  status.value = "Saving chat settings...";
  try {
    await saveConfig();
    await invoke("save_chat_settings", { input: { selectedAgentId: selectedAgentId.value, userAlias: userAlias.value } });
    status.value = "Chat settings saved.";
  } catch (e) {
    status.value = `Save chat settings failed: ${String(e)}`;
  } finally {
    saving.value = false;
  }
}

function scheduleConfigAutosave() {
  if (suppressAutosave.value) return;
  if (!configAutosaveReady.value) return;
  if (configAutosaveTimer) clearTimeout(configAutosaveTimer);
  configAutosaveTimer = setTimeout(() => {
    void saveConfig();
  }, 350);
}

function scheduleAgentsAutosave() {
  if (suppressAutosave.value) return;
  if (!agentsAutosaveReady.value) return;
  if (agentsAutosaveTimer) clearTimeout(agentsAutosaveTimer);
  agentsAutosaveTimer = setTimeout(() => {
    void saveAgents();
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

function addAgent() {
  const id = `agent-${Date.now()}`;
  const now = new Date().toISOString();
  agents.value.push({ id, name: `Agent ${agents.value.length + 1}`, systemPrompt: "", createdAt: now, updatedAt: now });
  selectedAgentId.value = id;
}

function removeSelectedAgent() {
  if (agents.value.length <= 1) return;
  const idx = agents.value.findIndex((a) => a.id === selectedAgentId.value);
  if (idx >= 0) agents.value.splice(idx, 1);
  selectedAgentId.value = agents.value[0].id;
}

async function refreshModels() {
  if (!selectedApiConfig.value) return;
  refreshingModels.value = true;
  try {
    const models = await invoke<string[]>("refresh_models", { input: { baseUrl: selectedApiConfig.value.baseUrl, apiKey: selectedApiConfig.value.apiKey, requestFormat: selectedApiConfig.value.requestFormat } });
    apiModelOptions.value[selectedApiConfig.value.id] = models;
    if (models.length) selectedApiConfig.value.model = models[0];
    status.value = `Model list refreshed (${models.length}).`;
  } catch (e) {
    status.value = `Refresh models failed: ${String(e)}`;
  } finally {
    refreshingModels.value = false;
  }
}

async function refreshToolsStatus() {
  if (!selectedApiConfig.value) return;
  checkingToolsStatus.value = true;
  try {
    toolStatuses.value = await invoke<ToolLoadStatus[]>("check_tools_status", {
      input: { apiConfigId: selectedApiConfig.value.id },
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
  if (!activeChatApiConfigId.value || !selectedAgentId.value) return;
  try {
    const snap = await invoke<ChatSnapshot>("get_chat_snapshot", { input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedAgentId.value } });
    latestUserText.value = snap.latestUser ? removeBinaryPlaceholders(renderMessage(snap.latestUser)) : "";
    latestUserImages.value = extractMessageImages(snap.latestUser);
    latestAssistantText.value = snap.latestAssistant ? renderMessage(snap.latestAssistant) : "";
  } catch (e) {
    status.value = `Load chat snapshot failed: ${String(e)}`;
  }
}
async function loadAllMessages() {
  if (!activeChatApiConfigId.value || !selectedAgentId.value) return;
  try {
    const msgs = await invoke<ChatMessage[]>("get_active_conversation_messages", {
      input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedAgentId.value },
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
type AssistantDeltaEvent = { delta: string };
const STREAM_FLUSH_INTERVAL_MS = 33;
const STREAM_DRAIN_TARGET_MS = 1000;
let streamPendingText = "";
let streamDrainDeadline = 0;
let streamFlushTimer: ReturnType<typeof setInterval> | null = null;

function readDeltaMessage(message: unknown): string {
  if (typeof message === "string") return message;
  if (message && typeof message === "object" && "delta" in message) {
    const value = (message as { delta?: unknown }).delta;
    return typeof value === "string" ? value : "";
  }
  return "";
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
  const text = chatInput.value.trim();
  if (!text && clipboardImages.value.length === 0 && clipboardAudios.value.length === 0) {
    return;
  }

  // 立刻刷新 UI：显示用户消息 + loading 气泡
  latestUserText.value = text;
  latestUserImages.value = [...clipboardImages.value];
  latestAssistantText.value = "";

  const sentImages = [...clipboardImages.value];
  const sentAudios = [...clipboardAudios.value];
  const sentModel = activeChatApiConfig.value?.model;
  chatInput.value = "";
  clipboardImages.value = [];
  clipboardAudios.value = [];

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
      ...sentAudios.map((aud) => ({
        type: "audio" as const,
        mime: aud.mime,
        bytesBase64: aud.bytesBase64,
      })),
    ],
  };
  allMessages.value = [...allMessages.value, optimisticUserMessage];
  visibleTurnCount.value = 1;

  const gen = ++chatGeneration;
  clearStreamBuffer();
  const deltaChannel = new Channel<AssistantDeltaEvent>();
  deltaChannel.onmessage = (event) => {
    const delta = readDeltaMessage(event);
    enqueueStreamDelta(gen, delta);
  };
  chatting.value = true;
  try {
    const result = await invoke<{ assistantText: string; latestUserText: string; archivedBeforeSend: boolean }>("send_chat_message", {
      input: {
        apiConfigId: activeChatApiConfigId.value,
        agentId: selectedAgentId.value,
        payload: { text, images: sentImages, audios: sentAudios, model: sentModel },
      },
      onDelta: deltaChannel,
    });
    if (gen !== chatGeneration) return;
    latestUserText.value = removeBinaryPlaceholders(result.latestUserText);
    latestUserImages.value = sentImages;
    enqueueFinalAssistantText(gen, result.assistantText);
    await loadAllMessages();
  } catch (e) {
    if (gen !== chatGeneration) return;
    clearStreamBuffer();
    latestAssistantText.value = `Error: ${String(e)}`;
    await loadAllMessages();
  } finally {
    if (gen === chatGeneration) {
      chatting.value = false;
    }
  }
}

function stopChat() {
  chatGeneration++;
  clearStreamBuffer();
  chatting.value = false;
  latestAssistantText.value = "(已中断)";
}

async function openCurrentHistory() {
  try {
    currentHistory.value = await invoke<ChatMessage[]>("get_active_conversation_messages", { input: { apiConfigId: activeChatApiConfigId.value, agentId: selectedAgentId.value } });
    historyDialog.value?.showModal();
  } catch (e) {
    status.value = `Load history failed: ${String(e)}`;
  }
}

function closeHistory() {
  historyDialog.value?.close();
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

function removeClipboardAudio(index: number) {
  if (index < 0 || index >= clipboardAudios.value.length) return;
  clipboardAudios.value.splice(index, 1);
}

async function readBlobAsDataUrl(blob: Blob): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
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
}

function matchesRecordHotkey(event: KeyboardEvent): boolean {
  if (config.recordHotkey === "Alt") return event.key === "Alt";
  if (config.recordHotkey === "Ctrl") return event.key === "Control";
  if (config.recordHotkey === "Shift") return event.key === "Shift";
  return false;
}

async function startRecording() {
  if (recording.value || chatting.value) return;
  if (!(activeChatApiConfig.value?.enableAudio || hasSttFallback.value)) return;
  if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
    status.value = "当前环境不支持录音。";
    return;
  }

  try {
    mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    mediaRecorder = new MediaRecorder(mediaStream);
    const chunks: BlobPart[] = [];
    recordingDiscardCurrent = false;

    mediaRecorder.ondataavailable = (event: BlobEvent) => {
      if (event.data && event.data.size > 0) chunks.push(event.data);
    };
    mediaRecorder.onstop = async () => {
      const durationMs = Math.max(0, Date.now() - recordingStartedAt);
      recording.value = false;
      clearRecordingTimers();
      if (mediaStream) {
        for (const track of mediaStream.getTracks()) track.stop();
        mediaStream = null;
      }
      const minMs = config.minRecordSeconds * 1000;
      if (recordingDiscardCurrent || durationMs < minMs || chunks.length === 0) {
        if (!recordingDiscardCurrent && durationMs < minMs) {
          status.value = `录音过短（< ${config.minRecordSeconds}s），已丢弃。`;
        }
        return;
      }
      const blob = new Blob(chunks, { type: mediaRecorder?.mimeType || "audio/webm" });
      const dataUrl = await readBlobAsDataUrl(blob);
      const base64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
      if (!base64) return;
      clipboardAudios.value.push({
        mime: blob.type || "audio/webm",
        bytesBase64: base64,
        durationMs,
      });
    };

    mediaRecorder.start();
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
    if (mediaStream) {
      for (const track of mediaStream.getTracks()) track.stop();
      mediaStream = null;
    }
  }
}

async function stopRecording(discard: boolean) {
  if (!recording.value) return;
  recordingDiscardCurrent = discard;
  if (mediaRecorder && mediaRecorder.state !== "inactive") {
    mediaRecorder.stop();
  } else {
    recording.value = false;
    clearRecordingTimers();
  }
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

async function minimizeWindow() { 
  if (!appWindow) return;
  await appWindow.minimize(); 
}
async function toggleMaximize() { 
  if (!appWindow) return;
  await appWindow.toggleMaximize(); 
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
    await loadAgents();
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
  agentsAutosaveReady.value = true;
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
    agentsAutosaveReady.value = false;
    chatSettingsAutosaveReady.value = false;
    await refreshAll();
    configAutosaveReady.value = true;
    agentsAutosaveReady.value = true;
    chatSettingsAutosaveReady.value = true;
  });

});

onBeforeUnmount(() => {
  clearStreamBuffer();
  void stopRecording(true);
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
    sttApiConfigId: config.sttApiConfigId,
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
    })),
  }),
  () => scheduleConfigAutosave(),
  { deep: true },
);

watch(
  () => config.apiConfigs.map((a) => ({
    id: a.id,
    enableText: a.enableText,
    enableImage: a.enableImage,
    enableAudio: a.enableAudio,
  })),
  () => normalizeApiBindingsLocal(),
  { deep: true },
);

watch(
  () => agents.value.map((a) => ({
    id: a.id,
    name: a.name,
    systemPrompt: a.systemPrompt,
    createdAt: a.createdAt,
    updatedAt: a.updatedAt,
  })),
  () => scheduleAgentsAutosave(),
  { deep: true },
);

watch(
  () => ({ selectedAgentId: selectedAgentId.value, userAlias: userAlias.value }),
  () => scheduleChatSettingsAutosave(),
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
  () => [configTab.value, config.selectedApiConfigId, selectedApiConfig.value?.enableTools],
  async ([tab, id, enabled]) => {
    if (tab !== "tools") return;
    if (!id) return;
    if (!enabled) {
      toolStatuses.value = (selectedApiConfig.value?.tools ?? []).map((t) => ({
        id: t.id,
        status: "disabled",
        detail: "此 API 配置未启用工具调用。",
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
</script>
