<template>
  <ConfigTemplate :model-value="templateValues" :groups="templateGroups">
    <template #group-actions-network-access>
      <button class="btn btn-sm bg-base-100 shrink-0" :disabled="loading" :title="t('common.refresh')" @click="refreshInfo">
        <AppIcon name="refresh-cw" class="h-4 w-4" :class="{ 'animate-spin': loading }" />
        <span>{{ t("common.refresh") }}</span>
      </button>
    </template>

    <template #row-network-enabled>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("config.networkAccess.enabled") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("config.networkAccess.enabledHint") }}</div>
        </div>
        <input
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          :checked="networkAccessEnabled"
          @change="updateEnabled"
        />
      </label>
    </template>

    <template #row-network-port>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <label class="grid min-w-0 gap-1">
          <span class="text-sm">{{ t("config.networkAccess.port") }}</span>
          <input
            class="input input-bordered input-sm w-full max-w-48 font-mono"
            type="number"
            min="1024"
            max="65535"
            :disabled="props.savingConfig"
            :value="portInput"
            @input="updatePort"
          />
        </label>
        <button
          class="btn btn-sm btn-primary shrink-0"
          :disabled="!settingsDirty || props.savingConfig"
          @click="saveSettings"
        >
          <AppIcon name="save" class="h-4 w-4" />
          {{ props.savingConfig ? t("common.saving") : t("common.save") }}
        </button>
      </div>
    </template>

    <template #row-network-local-link>
      <div class="flex min-w-0 items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("config.networkAccess.localLink") }}</div>
          <div class="mt-1 break-all font-mono text-xs text-base-content/70">{{ localUrlText }}</div>
        </div>
        <button class="btn btn-sm btn-primary shrink-0" :disabled="!webInfo?.localUrl || !webInfo?.enabled" @click="openLocalUrl">
          <AppIcon name="external-link" class="h-4 w-4" />
          {{ t("config.networkAccess.open") }}
        </button>
      </div>
    </template>

    <template #row-network-password>
      <div class="flex min-w-0 flex-wrap items-end justify-between gap-3">
        <label class="grid min-w-0 gap-2">
          <span class="text-sm">{{ t("config.networkAccess.remotePassword") }}</span>
          <input
            class="input input-bordered input-sm w-full max-w-64 font-mono"
            :disabled="!networkAccessEnabled"
            :value="passwordInput"
            :placeholder="webInfo?.remotePassword || ''"
            @input="updatePassword"
          />
        </label>
        <div class="flex shrink-0 items-center gap-1">
          <button class="btn btn-sm btn-ghost" :disabled="!networkAccessEnabled" :title="t('config.networkAccess.regeneratePassword')" @click="regeneratePassword">
            <AppIcon name="key-round" class="h-4 w-4" />
          </button>
          <button class="btn btn-sm btn-ghost" :disabled="!passwordInput || !webInfo?.enabled" @click="copyText(passwordInput)">
            <AppIcon name="copy" class="h-4 w-4" />
          </button>
          <button class="btn btn-sm btn-primary" :disabled="!settingsDirty || props.savingConfig" :title="t('common.save')" @click="saveSettings">
            <AppIcon name="save" class="h-4 w-4" />
          </button>
        </div>
      </div>
    </template>

    <template #row-network-links>
      <div class="grid min-w-0 gap-2">
        <div class="flex items-center justify-between gap-4">
          <div class="text-sm">{{ t("config.networkAccess.remoteLinks") }}</div>
          <button class="btn btn-sm btn-ghost shrink-0" :disabled="remoteUrls.length === 0 || !webInfo?.enabled" @click="copyText(remoteUrls.join('\n'))">
            <AppIcon name="copy" class="h-4 w-4" />
          </button>
        </div>
        <div v-if="remoteUrls.length > 0" class="grid gap-1">
          <div v-for="url in remoteUrls" :key="url" class="break-all font-mono text-xs text-base-content/70">
            {{ url }}
          </div>
        </div>
        <div v-else class="text-xs text-base-content/60">
          {{ webInfo?.enabled === false ? t("config.networkAccess.disabled") : t("config.networkAccess.noRemoteLink") }}
        </div>
      </div>
    </template>

    <template #row-network-connections>
      <div class="grid min-w-0 gap-3">
        <div>
          <div class="text-sm">{{ t("config.networkAccess.activeConnections") }}</div>
          <div class="mt-1 text-xs text-base-content/60">
            {{ t("config.networkAccess.activeConnectionsCount", { count: activeConnections.length }) }}
          </div>
        </div>
        <div v-if="activeConnections.length > 0" class="grid gap-2">
          <div v-for="item in activeConnections" :key="item.id" class="rounded-box border border-base-300 bg-base-100/70 p-3 text-xs">
            <div class="flex flex-wrap items-center gap-2">
              <span class="badge badge-outline">{{ connectionKindLabel(item.path) }}</span>
              <span class="badge badge-ghost">{{ item.local ? t("config.networkAccess.connectionLocal") : t("config.networkAccess.connectionRemote") }}</span>
              <span class="badge" :class="item.authenticated ? 'badge-success badge-outline' : 'badge-warning badge-outline'">
                {{ item.authenticated ? t("config.networkAccess.connectionAuthenticated") : t("config.networkAccess.connectionPending") }}
              </span>
            </div>
            <div class="mt-2 break-all font-mono text-base-content/70">{{ item.peerAddr }}</div>
            <div class="mt-1 text-base-content/60">{{ item.connectedAt }}</div>
          </div>
        </div>
        <div v-else class="text-xs text-base-content/60">
          {{ t("config.networkAccess.noActiveConnections") }}
        </div>
      </div>
    </template>

    <template #row-network-status>
      <div class="grid min-w-0 gap-2 text-xs">
        <div v-if="statusText" :class="statusError ? 'text-error' : 'text-success'">
          {{ statusText }}
        </div>
        <div class="font-medium">{{ t("config.networkAccess.linkStatus") }}</div>
        <div class="text-base-content/70">{{ linkStatusText }}</div>
        <div v-if="webInfo?.listenAddr" class="break-all font-mono text-base-content/60">{{ webInfo.listenAddr }}</div>
        <div v-if="webInfo?.lastError" class="break-all text-error">{{ webInfo.lastError }}</div>
      </div>
    </template>
  </ConfigTemplate>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import { invokeTauri, openTransportExternalUrl } from "../../../../services/tauri-api";
import type { AppConfig } from "../../../../types/app";

type WebAccessInfo = {
  running: boolean;
  enabled: boolean;
  configuredPort: number;
  port: number;
  listenAddr?: string;
  statusText?: string;
  lastError?: string;
  localUrl: string;
  remoteUrls: string[];
  remotePassword: string;
  activeConnections: Array<{
    id: string;
    path: string;
    peerAddr: string;
    local: boolean;
    authenticated: boolean;
    connectedAt: string;
    clientId: string;
  }>;
};

const props = defineProps<{
  config: AppConfig;
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  lastSavedConfigJson: string;
}>();

const { t } = useI18n();
const templateValues = {};
const templateGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "network-access",
    title: t("config.networkAccess.title"),
    rows: [
      { key: "network-enabled", items: [] },
      { key: "network-port", items: [] },
      { key: "network-local-link", items: [] },
      { key: "network-password", items: [] },
      { key: "network-links", items: [] },
      { key: "network-connections", items: [] },
      { key: "network-status", items: [] },
    ],
  },
]);
const webInfo = ref<WebAccessInfo | null>(null);
const loading = ref(false);
const statusText = ref("");
const statusError = ref(false);
const portInput = ref("");
const passwordInput = ref("");
const networkAccessEnabled = computed(() => props.config.webAccessEnabled !== false);

const savedPort = computed(() => {
  try {
    const parsed = JSON.parse(String(props.lastSavedConfigJson || "{}")) as Partial<AppConfig>;
    return normalizePort(parsed.webAccessPort);
  } catch {
    return 8429;
  }
});
const savedEnabled = computed(() => {
  try {
    const parsed = JSON.parse(String(props.lastSavedConfigJson || "{}")) as Partial<AppConfig>;
    return parsed.webAccessEnabled !== false;
  } catch {
    return true;
  }
});
const savedPassword = computed(() => {
  try {
    const parsed = JSON.parse(String(props.lastSavedConfigJson || "{}")) as Partial<AppConfig>;
    return String(parsed.webAccessPassword || "").trim();
  } catch {
    return "";
  }
});
const portDirty = computed(() => normalizePort(portInput.value) !== savedPort.value);
const enabledDirty = computed(() => networkAccessEnabled.value !== savedEnabled.value);
const passwordDirty = computed(() => passwordInput.value.trim() !== savedPassword.value);
const settingsDirty = computed(() => portDirty.value || enabledDirty.value || passwordDirty.value);
const remoteUrls = computed(() => webInfo.value?.remoteUrls || []);
const activeConnections = computed(() => webInfo.value?.activeConnections || []);
const localUrlText = computed(() => webInfo.value?.enabled === false ? t("config.networkAccess.disabled") : (webInfo.value?.localUrl || t("config.networkAccess.waiting")));
const linkStatusText = computed(() => {
  const info = webInfo.value;
  if (!info) return t("config.networkAccess.waiting");
  if (info.enabled === false) return t("config.networkAccess.disabled");
  if (info.running && info.localUrl) return t("config.networkAccess.statusListening", { port: info.port });
  if (info.statusText === "binding") return t("config.networkAccess.statusBinding", { port: info.configuredPort });
  if (info.statusText === "bind_failed") return t("config.networkAccess.statusBindFailed", { port: info.configuredPort });
  if (info.statusText === "stopped") return t("config.networkAccess.statusStopped");
  if (info.statusText === "error") return t("config.networkAccess.statusError");
  return t("config.networkAccess.statusUnavailable");
});

function normalizePort(value: unknown): number {
  const parsed = Math.round(Number(value));
  if (Number.isFinite(parsed) && parsed >= 1024 && parsed <= 65535) {
    return parsed;
  }
  return 8429;
}

function connectionKindLabel(path: string): string {
  return path === "/chat"
    ? t("config.networkAccess.connectionKindChat")
    : t("config.networkAccess.connectionKindContext");
}

function updateEnabled(event: Event) {
  props.config.webAccessEnabled = (event.target as HTMLInputElement).checked;
}

function updatePassword(event: Event) {
  const value = (event.target as HTMLInputElement).value.trim();
  passwordInput.value = value;
  props.config.webAccessPassword = value;
}

function updatePort(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  portInput.value = value;
  props.config.webAccessPort = normalizePort(value);
}

function regeneratePassword() {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let raw = "";
  for (let index = 0; index < 8; index += 1) {
    raw += chars[Math.floor(Math.random() * chars.length)] || "A";
  }
  passwordInput.value = `${raw.slice(0, 4)}-${raw.slice(4)}`;
  props.config.webAccessPassword = passwordInput.value;
}

async function refreshInfo() {
  await refreshInfoInternal(true);
}

async function refreshInfoInternal(forceRefresh = false) {
  loading.value = true;
  statusText.value = "";
  statusError.value = false;
  try {
    webInfo.value = await invokeTauri<WebAccessInfo>("get_web_access_info", {
      input: { forceRefresh },
    });
    if (!passwordInput.value.trim() && webInfo.value?.remotePassword) {
      passwordInput.value = webInfo.value.remotePassword;
      props.config.webAccessPassword = webInfo.value.remotePassword;
    }
  } catch (error) {
    statusError.value = true;
    statusText.value = String(error || t("config.networkAccess.refreshFailed"));
  } finally {
    loading.value = false;
  }
}

async function saveSettings() {
  if (!settingsDirty.value) return;
  props.config.webAccessPort = normalizePort(portInput.value);
  portInput.value = String(props.config.webAccessPort);
  props.config.webAccessPassword = passwordInput.value.trim();
  const saved = await Promise.resolve(props.saveConfigAction());
  if (saved !== false) {
    window.setTimeout(() => void refreshInfoInternal(true), 500);
  }
}

async function openLocalUrl() {
  const url = webInfo.value?.localUrl || "";
  if (!url) return;
  try {
    await openTransportExternalUrl(url);
  } catch (error) {
    statusError.value = true;
    statusText.value = String(error || t("config.networkAccess.openFailed"));
  }
}

async function copyText(text: string) {
  const value = text.trim();
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    statusError.value = false;
    statusText.value = t("config.networkAccess.copied");
  } catch (error) {
    statusError.value = true;
    statusText.value = String(error || t("config.networkAccess.copyFailed"));
  }
}

onMounted(() => {
  portInput.value = String(normalizePort(props.config.webAccessPort));
  passwordInput.value = String(props.config.webAccessPassword || "").trim();
  void refreshInfoInternal(false);
});
</script>
