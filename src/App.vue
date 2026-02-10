
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
        :loading="loading"
        :agents="agents"
        :selected-agent-id="selectedAgentId"
        :selected-agent="selectedAgent"
        :user-alias="userAlias"
        @update:config-tab="configTab = $event"
        @update:selected-agent-id="selectedAgentId = $event"
        @update:user-alias="userAlias = $event"
        @toggle-theme="toggleTheme"
        @load-config="loadConfig"
        @refresh-models="refreshModels"
        @add-api-config="addApiConfig"
        @remove-selected-api-config="removeSelectedApiConfig"
        @add-agent="addAgent"
        @remove-selected-agent="removeSelectedAgent"
        @open-current-history="openCurrentHistory"
      />

      <ChatView
        v-else-if="viewMode === 'chat'"
        :user-alias="userAlias"
        :agent-name="selectedAgent?.name || '助理'"
        :latest-user-text="latestUserText"
        :latest-assistant-text="latestAssistantText"
        :clipboard-images="clipboardImages"
        :chat-input="chatInput"
        :chat-input-placeholder="chatInputPlaceholder"
        :chatting="chatting"
        @update:chat-input="chatInput = $event"
        @remove-clipboard-image="removeClipboardImage"
        @send-chat="sendChat"
        @stop-chat="stopChat"
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
              <div class="whitespace-pre-wrap">{{ renderMessage(m) }}</div>
            </div>
          </div>
          <div class="modal-action"><button class="btn btn-sm" @click="closeHistory">关闭</button></div>
        </div>
      </dialog>

    </div>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow, WebviewWindow } from "@tauri-apps/api/window";
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
} from "./types/app";

let appWindow: WebviewWindow | null = null;
const viewMode = ref<"chat" | "archives" | "config">("config");

const config = reactive<AppConfig>({ hotkey: "Alt+C", selectedApiConfigId: "", apiConfigs: [] });
const configTab = ref<"hotkey" | "api" | "agent" | "chatSettings">("hotkey");
const currentTheme = ref<"light" | "forest">("light");
const agents = ref<AgentProfile[]>([]);
const selectedAgentId = ref("default-agent");
const userAlias = ref("用户");
const chatInput = ref("");
const latestUserText = ref("");
const latestAssistantText = ref("");
const currentHistory = ref<ChatMessage[]>([]);
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);

const archives = ref<ArchiveSummary[]>([]);
const archiveMessages = ref<ChatMessage[]>([]);

const windowReady = ref(false);
const status = ref("Ready.");
const loading = ref(false);
const saving = ref(false);
const chatting = ref(false);
const refreshingModels = ref(false);
const historyDialog = ref<HTMLDialogElement | null>(null);
const alwaysOnTop = ref(false);
const configAutosaveReady = ref(false);
const agentsAutosaveReady = ref(false);
const chatSettingsAutosaveReady = ref(false);
const suppressAutosave = ref(false);
let configAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let agentsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
let chatSettingsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;

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
const selectedAgent = computed(() => agents.value.find((a) => a.id === selectedAgentId.value) ?? null);
const baseUrlReference = computed(() => {
  const format = selectedApiConfig.value?.requestFormat ?? "openai";
  if (format === "gemini") return "https://generativelanguage.googleapis.com/v1beta/openai";
  if (format === "deepseek/kimi") return "https://api.deepseek.com/v1";
  return "https://api.openai.com/v1";
});
const chatInputPlaceholder = computed(() => {
  const api = selectedApiConfig.value;
  if (!api) return "输入问题";
  const hints: string[] = [];
  if (api.enableImage) hints.push("Ctrl+V 粘贴图片");
  if (api.enableAudio) hints.push("可发送语音");
  if (hints.length === 0) return "输入问题";
  return `输入问题，${hints.join("，")}`;
});
function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
  return {
    id: `api-config-${seed}`,
    name: `API Config ${config.apiConfigs.length + 1}`,
    requestFormat: "openai",
    enableText: true,
    enableImage: true,
    enableAudio: true,
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o-mini",
  };
}

function renderMessage(msg: ChatMessage): string {
  return msg.parts.map((p) => {
    if (p.type === "text") return p.text;
    if (p.type === "image") return "[image]";
    return "[audio]";
  }).join("\n");
}

async function loadConfig() {
  suppressAutosave.value = true;
  loading.value = true;
  status.value = "Loading config...";
  try {
    const cfg = await invoke<AppConfig>("load_config");
    config.hotkey = cfg.hotkey;
    config.selectedApiConfigId = cfg.selectedApiConfigId;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...(cfg.apiConfigs.length ? cfg.apiConfigs : [createApiConfig("default")]));
    if (!config.apiConfigs.some((a) => a.id === config.selectedApiConfigId)) config.selectedApiConfigId = config.apiConfigs[0].id;
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
    config.selectedApiConfigId = saved.selectedApiConfigId;
    config.apiConfigs.splice(0, config.apiConfigs.length, ...saved.apiConfigs);
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
}

function removeSelectedApiConfig() {
  if (config.apiConfigs.length <= 1) return;
  const idx = config.apiConfigs.findIndex((a) => a.id === config.selectedApiConfigId);
  if (idx >= 0) config.apiConfigs.splice(idx, 1);
  config.selectedApiConfigId = config.apiConfigs[0].id;
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
    if (models.length) selectedApiConfig.value.model = models[0];
    status.value = `Model list refreshed (${models.length}).`;
  } catch (e) {
    status.value = `Refresh models failed: ${String(e)}`;
  } finally {
    refreshingModels.value = false;
  }
}

async function refreshChatSnapshot() {
  if (!config.selectedApiConfigId || !selectedAgentId.value) return;
  try {
    const snap = await invoke<ChatSnapshot>("get_chat_snapshot", { input: { apiConfigId: config.selectedApiConfigId, agentId: selectedAgentId.value } });
    latestUserText.value = snap.latestUser ? renderMessage(snap.latestUser) : "";
    latestAssistantText.value = snap.latestAssistant ? renderMessage(snap.latestAssistant) : "";
  } catch (e) {
    status.value = `Load chat snapshot failed: ${String(e)}`;
  }
}
let chatGeneration = 0;

async function sendChat() {
  const text = chatInput.value.trim();
  if (!text && clipboardImages.value.length === 0) {
    return;
  }

  // 立刻刷新 UI：显示用户消息 + loading 气泡
  const imageCount = clipboardImages.value.length;
  const userPreview = [text, imageCount > 0 ? `[图片 x${imageCount}]` : ""].filter(Boolean).join("\n");
  latestUserText.value = userPreview;
  latestAssistantText.value = "";

  const sentImages = [...clipboardImages.value];
  const sentModel = config.apiConfigs.find((a) => a.id === config.selectedApiConfigId)?.model;
  chatInput.value = "";
  clipboardImages.value = [];

  const gen = ++chatGeneration;
  chatting.value = true;
  try {
    const result = await invoke<{ assistantText: string; latestUserText: string; archivedBeforeSend: boolean }>("send_chat_message", {
      input: {
        apiConfigId: config.selectedApiConfigId,
        agentId: selectedAgentId.value,
        payload: { text, images: sentImages, model: sentModel },
      },
    });
    if (gen !== chatGeneration) return;
    latestUserText.value = result.latestUserText;
    latestAssistantText.value = result.assistantText;
  } catch (e) {
    if (gen !== chatGeneration) return;
    latestAssistantText.value = `Error: ${String(e)}`;
  } finally {
    if (gen === chatGeneration) chatting.value = false;
  }
}

function stopChat() {
  chatGeneration++;
  chatting.value = false;
  latestAssistantText.value = "(已中断)";
}

async function openCurrentHistory() {
  try {
    currentHistory.value = await invoke<ChatMessage[]>("get_active_conversation_messages", { input: { apiConfigId: config.selectedApiConfigId, agentId: selectedAgentId.value } });
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
  const apiConfig = selectedApiConfig.value;
  if (!apiConfig) return;

  const text = event.clipboardData?.getData("text/plain");
  if (text && !chatInput.value.trim() && apiConfig.enableText) chatInput.value = text;

  for (const item of Array.from(items)) {
    if (item.type.startsWith("image/")) {
      if (!apiConfig.enableImage) {
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

async function importClipboardImageOnOpen() {
  if (viewMode.value !== "chat") return;
  const apiConfig = selectedApiConfig.value;
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

  if (!apiConfig.enableImage) return;
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
  const refreshAll = async () => {
    await loadConfig();
    await loadAgents();
    await loadChatSettings();
    if (viewMode.value === "chat") {
      await refreshChatSnapshot();
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

watch(
  () => ({
    hotkey: config.hotkey,
    selectedApiConfigId: config.selectedApiConfigId,
    apiConfigs: config.apiConfigs.map((a) => ({
      id: a.id,
      name: a.name,
      requestFormat: a.requestFormat,
      enableText: a.enableText,
      enableImage: a.enableImage,
      enableAudio: a.enableAudio,
      baseUrl: a.baseUrl,
      apiKey: a.apiKey,
      model: a.model,
    })),
  }),
  () => scheduleConfigAutosave(),
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
</script>
