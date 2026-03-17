<template>
  <div class="flex flex-col gap-4 min-h-0 h-full overflow-y-auto pr-1">
    <!-- 左侧：渠道列表 -->
    <div class="bg-base-100 rounded-box border border-base-300 w-full h-72 shrink-0 flex flex-col overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 shrink-0">
        <span class="font-semibold text-sm">{{ t("config.remoteIm.title") }}</span>
        <div class="flex items-center gap-1">
          <button class="btn btn-xs btn-ghost" :disabled="channelPage <= 1" @click="channelPage -= 1">‹</button>
          <span class="text-xs font-medium opacity-70">{{ channelPage }} / {{ channelPageCount }}</span>
          <button class="btn btn-xs btn-ghost" :disabled="channelPage >= channelPageCount" @click="channelPage += 1">›</button>
        </div>
        <div class="flex gap-1">
          <button class="btn  btn-square btn-ghost" :title="t('config.remoteIm.addChannel')" @click="addChannel">
            <Plus class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn  btn-square btn-ghost"
            :class="!selectedChannel ? 'cursor-not-allowed' : ''"
            :title="t('common.delete')"
            :disabled="!selectedChannel"
            @click="removeSelectedChannel"
          >
            <Trash2 class="h-3.5 w-3.5" :class="!selectedChannel ? '' : 'text-error'" />
          </button>
        </div>
      </div>
      <ul class="menu w-full flex-1 overflow-y-auto">
        <li v-if="channels.length === 0" class="menu-title">
          <span class="text-xs italic opacity-60">{{ t("config.remoteIm.empty") }}</span>
        </li>
        <li v-for="(ch, idx) in pagedChannels" :key="ch.id">
          <button class="flex items-center gap-2" :class="{ 'menu-active': selectedChannelId === ch.id }" @click="selectedChannelId = ch.id">
            <span class="badge badge-xs" :class="channelListStatusBadgeClass(ch)">{{ channelListStatusBadgeText(ch) }}</span>
            <span class="truncate">{{ ch.name || `#${(channelPage - 1) * CHANNELS_PAGE_SIZE + idx + 1}` }}</span>
          </button>
        </li>
      </ul>
    </div>

    <!-- 中间：渠道详情 -->
    <div class="w-full flex flex-col min-h-0">
      <div v-if="!selectedChannel" class="bg-base-100 rounded-box border border-base-300 flex-1 flex items-center justify-center">
        <div class="text-xs italic opacity-60">{{ t("config.remoteIm.empty") }}</div>
      </div>

      <div v-else class="bg-base-100 rounded-box border border-base-300 flex-1 min-h-0 overflow-hidden flex flex-col">
        <!-- 头部 -->
        <div class="flex items-center justify-between px-3 py-2 shrink-0">
          <span class="font-semibold text-sm">{{ selectedChannel.name || t('config.remoteIm.channelName') }}</span>
          <div class="flex gap-1">
            <button
              v-if="selectedChannel.platform === 'onebot_v11'"
              class="btn  btn-ghost"
              :title="t('common.reset')"
              @click="resetNapcatCredentials"
            >
              <RotateCcw class="h-3.5 w-3.5" />
              {{ t("common.reset") }}
            </button>
            <button
              class="btn "
              :class="channelDirty ? 'btn-primary' : 'btn-ghost'"
              :disabled="!channelDirty || saving"
              @click="saveChannels"
            >
              <Save v-if="!saving" class="h-3.5 w-3.5" />
              <span v-else class="loading loading-spinner loading-xs"></span>
              {{ t("common.save") }}
            </button>
          </div>
        </div>

        <!-- 状态栏 -->
        <div class="px-3 pb-2 shrink-0">
          <div class="rounded-box border border-base-300 bg-base-200/60 px-3 py-2 flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 min-w-0">
              <span
                class="size-2 rounded-full shrink-0"
                :class="selectedChannel.platform === 'onebot_v11'
                  ? (channelRuntimeStates[selectedChannel.id]?.connected ? 'bg-success' : (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300'))
                  : (selectedChannel.platform === 'dingtalk'
                    ? (channelRuntimeStates[selectedChannel.id]?.connected ? 'bg-success' : (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300'))
                    : ((selectedChannel.platform === 'feishu')
                      ? (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300')
                      : (selectedChannel.enabled ? 'bg-success' : 'bg-base-300')))"
              ></span>
              <span class="text-xs font-medium">{{ t("config.remoteIm.connectionStatus") }}</span>
              <span class="text-xs opacity-80 truncate">{{ channelStatusPreview(selectedChannel!) }}</span>
            </div>
            <div class="flex items-center gap-2">
              <button class="btn btn-xs btn-ghost" @click="openChannelLogsModal">
                {{ t("config.remoteIm.viewLogs") }}
              </button>
              <label class="flex items-center gap-2">
                <span class="text-xs opacity-70">{{ t("config.remoteIm.enabled") }}</span>
                <input
                  type="checkbox"
                  class="toggle toggle-primary bg-base-100"
                  :checked="selectedChannel.enabled"
                  :disabled="saving"
                  @change="(e) => toggleSelectedChannelEnabled((e.target as HTMLInputElement).checked)"
                />
              </label>
            </div>
          </div>
        </div>

        <!-- 内容滚动区 -->
        <div class="flex-1 overflow-y-auto px-3 text-xs">
            <!-- 渠道名称 -->
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <span>{{ t("config.remoteIm.channelName") }}</span>
              <input v-model="selectedChannel.name" class="input input-bordered input-sm w-48" :placeholder="t('config.remoteIm.channelName')" />
            </div>
            <!-- 平台 -->
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <span>{{ t("config.remoteIm.platform") }}</span>
              <select v-model="selectedChannel.platform" class="select select-bordered select-sm w-48">
                <option value="onebot_v11">{{ t("config.remoteIm.platformOptions.onebotV11") }}</option>
                <option value="feishu">{{ t("config.remoteIm.platformOptions.feishu") }}</option>
                <option value="dingtalk">{{ t("config.remoteIm.platformOptions.dingtalk") }}</option>
              </select>
            </div>
            <!-- 能力配置标题 -->
            <div class="border-b-base-content/5 flex items-center gap-2 border-b border-dashed py-2 mt-2">
              <span class="font-semibold">{{ t("config.remoteIm.capabilities") }}</span>
            </div>

            <!-- 能力配置列表 -->
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.activateAssistant" type="checkbox" class="checkbox" />
                <span>{{ t("config.remoteIm.activateAssistant") }}</span>
              </label>
            </div>
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.receiveFiles" type="checkbox" class="checkbox" />
                <span>{{ t("config.remoteIm.receiveFiles") }}</span>
              </label>
            </div>
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.allowSendFiles" type="checkbox" class="checkbox" />
                <span>{{ t("config.remoteIm.allowSendFiles") }}</span>
              </label>
            </div>
            <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.showToolCalls" type="checkbox" class="checkbox" />
                <span>{{ t("config.remoteIm.showToolCalls") }}</span>
              </label>
            </div>

            <!-- OneBot v11 凭证配置 -->
            <template v-if="selectedChannel.platform === 'onebot_v11'">
              <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                <span class="font-semibold">{{ t("config.remoteIm.napcatConfig") }}</span>
              </div>
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.wsHost") }}</span>
                <input v-model="napcatCredentials.wsHost" class="input input-bordered input-sm w-32" placeholder="0.0.0.0" />
              </div>
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.wsPort") }}</span>
                <input v-model.number="napcatCredentials.wsPort" type="number" class="input input-bordered input-sm w-32" placeholder="6199" />
              </div>
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.wsToken") }}</span>
                <input v-model="napcatCredentials.wsToken" class="input input-bordered input-sm w-32" :placeholder="t('config.remoteIm.wsTokenPlaceholder')" />
              </div>
            </template>

            <!-- 钉钉凭证 -->
            <template v-else-if="selectedChannel.platform === 'dingtalk'">
              <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                <span class="font-semibold">{{ t("config.remoteIm.dingtalkCredentials") }}</span>
              </div>
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.dingtalkClientId") }}</span>
                <input
                  v-model="dingtalkCredentials.clientId"
                  class="input input-bordered input-sm w-72"
                  placeholder="dingxxxxxxxxxxxxxxxx"
                />
              </div>
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.dingtalkClientSecret") }}</span>
                <div class="flex items-center gap-2">
                  <input
                    v-model="dingtalkCredentials.clientSecret"
                    :type="showDingtalkSecret ? 'text' : 'password'"
                    class="input input-bordered input-sm w-72"
                    placeholder="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                  />
                  <button
                    class="btn btn-xs btn-ghost"
                    type="button"
                    @click="showDingtalkSecret = !showDingtalkSecret"
                  >
                    {{ showDingtalkSecret ? "隐藏" : "显示" }}
                  </button>
                </div>
              </div>
            </template>

            <!-- 飞书凭证 JSON -->
            <template v-else>
              <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                <span class="font-semibold">{{ t("config.remoteIm.credentialsJson") }}</span>
                <textarea
                  v-model="credentialDrafts[selectedChannel.id]"
                  class="textarea textarea-bordered w-full min-h-20 font-mono"
                  spellcheck="false"
                  @blur="syncCredentialJson(selectedChannel)"
                />
              </div>
            </template>

          <!-- 连接状态区域 (仅 OneBot v11) -->
          <template v-if="selectedChannel.platform === 'onebot_v11'">
            <div class="border-t border-base-300 mt-2 pt-2">
              <div class="flex items-center justify-between">
                <span class="font-semibold">{{ t("config.remoteIm.connectionStatus") }}</span>
                <button class="btn btn-square btn-ghost" :title="t('common.refresh')" @click="refreshChannelStatus">
                  <RefreshCw class="h-3.5 w-3.5" />
                </button>
              </div>
              <div class="mt-2 flex items-center gap-2">
                <span class="size-2 rounded-full" :class="channelStatus?.connected ? 'bg-success' : 'bg-base-300'"></span>
                <span class="text-xs">
                  {{ channelStatus?.connected
                    ? `${t("config.remoteIm.connected")} (${channelStatus.peerAddr})`
                    : channelStatus?.listenAddr
                      ? t("config.remoteIm.waitingForConnection")
                      : t("config.remoteIm.serverNotStarted") }}
                </span>
              </div>
            </div>

            <!-- 日志区域 -->
            <div class="border-t border-base-300 mt-2 pt-2 min-h-0 flex flex-col">
              <div class="flex items-center justify-between shrink-0">
                <span class="font-semibold">{{ t("config.remoteIm.channelLogs") }}</span>
                <button class="btn btn-square btn-ghost" :title="t('common.refresh')" @click="refreshChannelLogs">
                  <RefreshCw class="h-3.5 w-3.5" />
                </button>
              </div>
              <div class="mt-2 mb-3 flex-1 min-h-0 overflow-y-auto">
                <div v-if="channelLogs.length === 0" class="opacity-60 italic text-xs">{{ t("config.remoteIm.noLogs") }}</div>
                <pre v-else class="bg-base-200 rounded-box p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all m-0"><template v-for="(log, idx) in channelLogs" :key="idx"><span :class="log.level === 'error' ? 'text-error' : log.level === 'warn' ? 'text-warning' : ''"><span class="opacity-50">{{ formatLogTime(log.timestamp) }}</span> {{ log.message }}</span>{{ '\n' }}</template></pre>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>

    <!-- 右侧：联系人列表 -->
    <div class="bg-base-100 rounded-box border border-base-300 w-full h-88 shrink-0 flex flex-col overflow-hidden">
      <div class="relative flex items-center justify-between px-3 py-2 shrink-0">
        <span class="flex items-center gap-2 font-semibold text-sm">
          {{ t("config.remoteIm.contactsTitle") }}
          <span class="badge badge-ghost badge-xs">{{ currentChannelContacts.length }}</span>
        </span>
        <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-1">
          <button class="btn btn-xs btn-ghost" :disabled="contactPage <= 1" @click="contactPage -= 1">‹</button>
          <span class="text-xs font-medium opacity-70">{{ contactPage }} / {{ contactPageCount }}</span>
          <button class="btn btn-xs btn-ghost" :disabled="contactPage >= contactPageCount" @click="contactPage += 1">›</button>
        </div>
        <button class="btn btn-square btn-ghost" :title="t('common.refresh')" @click="refreshContacts">
          <RefreshCw class="h-3.5 w-3.5" :class="contactsLoading ? 'animate-spin' : ''" />
        </button>
      </div>
      <ul class="list w-full flex-1 overflow-y-auto">
        <li v-if="selectedChannel && (selectedChannel.platform === 'feishu' || selectedChannel.platform === 'dingtalk')" class="menu-title">
          <span class="text-xs text-warning font-medium">{{ t("config.remoteIm.experimental") }}</span>
        </li>
        <li v-if="contactsError" class="menu-title">
          <span class="text-xs text-error">{{ contactsError }}</span>
        </li>
        <li v-if="currentChannelContacts.length === 0" class="menu-title">
          <span class="text-xs italic opacity-60">{{ t("config.remoteIm.contactsEmpty") }}</span>
        </li>
        <li v-for="(item, idx) in pagedCurrentChannelContacts" :key="item.id" class="flex flex-col border-b border-base-200">
          <!-- 主行（始终显示） -->
          <div class="flex items-center gap-3 px-3 py-2 cursor-pointer bg-base-300" @click="toggleContactExpand(item.id)">
            <span class="badge shrink-0" :class="item.remoteContactType === 'group' ? 'badge-secondary' : 'badge-primary'">{{ item.remoteContactType === "group" ? t("config.remoteIm.group") : t("config.remoteIm.private") }}</span>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="truncate font-semibold flex-1">{{ contactDisplayName(item) }}</span>
              </div>
              <div class="text-xs opacity-50">{{ item.remoteContactId }}</div>
            </div>
            <div class="text-base transition-transform duration-200" :class="expandedContactIds.has(item.id) ? 'rotate-90' : ''">›</div>
          </div>

          <!-- 展开的详情区域 -->
          <div v-if="expandedContactIds.has(item.id)" class="px-3 pb-3 bg-base-100/50 text-xs">
            <!-- 激活配置 -->
            <div class="flex flex-col gap-2 mt-2 pt-2 border-t border-base-200">
              <div class="flex items-center justify-between gap-2">
                <span>{{ t("config.remoteIm.activateMode") }}</span>
                <select
                  class="select select-bordered select-sm w-32"
                  :value="item.activationMode"
                  @change="(e) => onContactActivationModeChange(item, (e.target as HTMLSelectElement).value)"
                >
                  <option value="always">{{ t("config.remoteIm.activateModeAlways") }}</option>
                  <option value="never">{{ t("config.remoteIm.activateModeNever") }}</option>
                  <option value="keyword">{{ t("config.remoteIm.activateModeKeyword") }}</option>
                </select>
              </div>
              <div class="flex items-center justify-between gap-2">
                <span>{{ t("config.remoteIm.activateCooldown") }}</span>
                <div class="flex items-center gap-1">
                  <input
                    type="number"
                    class="input input-bordered input-sm w-16"
                    :value="item.activationCooldownSeconds"
                    min="0"
                    @change="(e) => onContactActivationCooldownChange(item, Number((e.target as HTMLInputElement).value || 0))"
                  />
                  <span class="opacity-60">{{ t("config.remoteIm.seconds") }}</span>
                </div>
              </div>
              <div v-if="item.activationMode === 'keyword'" class="flex items-center justify-between gap-2">
                <span>{{ t("config.remoteIm.activateKeywords") }}</span>
                <input
                  type="text"
                  class="input input-bordered input-sm flex-1"
                  :placeholder="t('config.remoteIm.activateKeywordsPlaceholder')"
                  :value="contactKeywordDrafts[item.id] ?? item.activationKeywords.join(', ')"
                  @input="(e) => { contactKeywordDrafts[item.id] = (e.target as HTMLInputElement).value; }"
                  @blur="() => onContactActivationKeywordsBlur(item)"
                />
              </div>
              <div class="flex items-center justify-between gap-2 pt-1">
                <span>{{ t("config.remoteIm.allowReceive") }}</span>
                <input
                  type="checkbox"
                  class="toggle toggle-primary"
                  :checked="item.allowReceive"
                  @change="(e) => toggleContactAllowReceive(item, (e.target as HTMLInputElement).checked)"
                />
              </div>
              <div class="flex items-center justify-between gap-2">
                <span>{{ t("config.remoteIm.allowSend") }}</span>
                <input
                  type="checkbox"
                  class="toggle toggle-primary"
                  :checked="item.allowSend"
                  @change="(e) => toggleContactAllowSend(item, (e.target as HTMLInputElement).checked)"
                />
              </div>
            </div>
          </div>
        </li>
      </ul>
    </div>

    <div class="modal" :class="{ 'modal-open': channelLogsModalOpen }" @click.self="closeChannelLogsModal">
      <div class="modal-box max-w-4xl">
        <div class="flex items-center justify-between">
          <div class="font-semibold">
            {{ t("config.remoteIm.channelLogs") }} · {{ selectedChannel?.name || "-" }}
          </div>
          <div class="flex items-center gap-2">
            <button class="btn btn-sm btn-ghost" :title="t('common.refresh')" @click="refreshChannelLogs">
              <RefreshCw class="h-4 w-4" :class="channelLogsLoading ? 'animate-spin' : ''" />
            </button>
            <button class="btn btn-sm" @click="closeChannelLogsModal">{{ t("common.close") }}</button>
          </div>
        </div>
        <div class="mt-3 max-h-[60vh] overflow-y-auto">
          <div v-if="channelLogs.length === 0" class="opacity-60 italic text-xs">{{ t("config.remoteIm.noLogs") }}</div>
          <pre v-else class="bg-base-200 rounded-box p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all m-0"><template v-for="(log, idx) in channelLogs" :key="idx"><span :class="log.level === 'error' ? 'text-error' : log.level === 'warn' ? 'text-warning' : ''"><span class="opacity-50">{{ formatLogTime(log.timestamp) }}</span> {{ log.message }}</span>{{ '\n' }}</template></pre>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, RefreshCw, RotateCcw, Save, Trash2 } from "lucide-vue-next";
import { invokeTauri } from "../../../../services/tauri-api";
import type { AppConfig, RemoteImChannelConfig, RemoteImContact } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  saveConfigAction: () => Promise<boolean> | boolean;
  setStatusAction: (text: string) => void;
}>();

const { t } = useI18n();
const saving = ref(false);
const contactsLoading = ref(false);
const contactsError = ref("");
const contacts = ref<RemoteImContact[]>([]);
const credentialDrafts = ref<Record<string, string>>({});
const napcatCredentials = ref({ wsHost: "0.0.0.0", wsPort: 6199, wsToken: "" });
const dingtalkCredentials = ref({ clientId: "", clientSecret: "" });
const showDingtalkSecret = ref(false);
const suppressCredentialSync = ref(false);
const selectedChannelId = ref<string>("");
const channels = computed(() => props.config.remoteImChannels || []);
const CHANNELS_PAGE_SIZE = 8;
const CONTACTS_PAGE_SIZE = 6;
const channelPage = ref(1);
const contactPage = ref(1);

// 连接状态和日志
type ChannelConnectionStatus = {
  channelId: string;
  connected: boolean;
  peerAddr?: string;
  connectedAt?: string;
  listenAddr: string;
};
type ChannelLogEntry = {
  timestamp: string;
  level: string;
  message: string;
};
const channelStatus = ref<ChannelConnectionStatus | null>(null);
const channelLogs = ref<ChannelLogEntry[]>([]);
const channelLogsModalOpen = ref(false);
const channelLogsLoading = ref(false);
const channelRuntimeStates = ref<Record<string, ChannelConnectionStatus | null>>({});
let channelStatusTimer: ReturnType<typeof setInterval> | null = null;

const selectedChannel = computed(() =>
  channels.value.find((ch) => ch.id === selectedChannelId.value) ?? null,
);

const channelSnapshot = computed(() => {
  const ch = selectedChannel.value;
  if (!ch) return "";
  const credStr = JSON.stringify(ch.credentials || {}, Object.keys(ch.credentials || {}).sort());
  return JSON.stringify({
    name: ch.name,
    platform: ch.platform,
    enabled: ch.enabled,
    activateAssistant: ch.activateAssistant,
    receiveFiles: ch.receiveFiles,
    allowSendFiles: ch.allowSendFiles,
    streamingSend: ch.streamingSend,
    showToolCalls: ch.showToolCalls,
    credentials: credStr,
  });
});
const lastSavedChannelSnapshot = ref(channelSnapshot.value);
const channelDirty = computed(() => channelSnapshot.value !== lastSavedChannelSnapshot.value);

const channelMap = computed(() => {
  const map = new Map<string, RemoteImChannelConfig>();
  for (const item of channels.value) map.set(item.id, item);
  return map;
});

const groupedContacts = computed(() => {
  const groups = new Map<string, { channelId: string; channelName: string; platformLabel: string; channelEnabled: boolean; contacts: RemoteImContact[] }>();
  for (const item of contacts.value) {
    const channel = channelMap.value.get(item.channelId);
    const channelName = channel?.name || item.channelId;
    const platformLabel = platformLabelOf(String(channel?.platform || item.platform));
    const channelEnabled = channel?.enabled !== false;
    if (!groups.has(item.channelId)) {
      groups.set(item.channelId, {
        channelId: item.channelId,
        channelName,
        platformLabel,
        channelEnabled,
        contacts: [],
      });
    }
    groups.get(item.channelId)!.contacts.push(item);
  }
  return [...groups.values()];
});

const currentChannelContacts = computed(() => {
  if (!selectedChannelId.value) return [];
  return contacts.value.filter((c) => c.channelId === selectedChannelId.value);
});

const channelPageCount = computed(() =>
  Math.max(1, Math.ceil(channels.value.length / CHANNELS_PAGE_SIZE)),
);

const pagedChannels = computed(() => {
  const start = (channelPage.value - 1) * CHANNELS_PAGE_SIZE;
  return channels.value.slice(start, start + CHANNELS_PAGE_SIZE);
});

const contactPageCount = computed(() =>
  Math.max(1, Math.ceil(currentChannelContacts.value.length / CONTACTS_PAGE_SIZE)),
);

const pagedCurrentChannelContacts = computed(() => {
  const start = (contactPage.value - 1) * CONTACTS_PAGE_SIZE;
  return currentChannelContacts.value.slice(start, start + CONTACTS_PAGE_SIZE);
});

// 展开的联系人ID集合
const expandedContactIds = ref<Set<string>>(new Set());
const contactKeywordDrafts = ref<Record<string, string>>({});

function toggleContactExpand(contactId: string) {
  const newSet = new Set(expandedContactIds.value);
  if (newSet.has(contactId)) {
    newSet.delete(contactId);
  } else {
    newSet.add(contactId);
  }
  expandedContactIds.value = newSet;
}

function platformLabelOf(platform: string): string {
  const value = String(platform || "").trim().toLowerCase();
  if (value === "feishu") return "Feishu";
  if (value === "dingtalk") return "DingTalk";
  return "OneBot v11";
}

function asNonEmptyString(value: unknown): string {
  return String(value || "").trim();
}

function validateChannelBeforeEnable(channel: RemoteImChannelConfig): string {
  const creds = channel.credentials || {};
  if (channel.platform === "dingtalk") {
    const clientId = asNonEmptyString(creds.clientId);
    const clientSecret = asNonEmptyString(creds.clientSecret);
    if (!clientId || !clientSecret) {
      return t("config.remoteIm.enableNeedDingtalkCredentials");
    }
  }
  if (channel.platform === "feishu") {
    const appId = asNonEmptyString(creds.appId);
    const appSecret = asNonEmptyString(creds.appSecret);
    if (!appId || !appSecret) {
      return t("config.remoteIm.enableNeedFeishuCredentials");
    }
  }
  return "";
}

function newChannel(): RemoteImChannelConfig {
  return {
    id: `remote-im-${Date.now()}`,
    name: "Remote IM",
    platform: "onebot_v11",
    enabled: false,
    credentials: {},
    activateAssistant: true,
    receiveFiles: true,
    streamingSend: false,
    showToolCalls: false,
    allowSendFiles: false,
  };
}

function addChannel() {
  const ch = newChannel();
  props.config.remoteImChannels.push(ch);
  selectedChannelId.value = ch.id;
}

function removeSelectedChannel() {
  const idx = channels.value.findIndex((ch) => ch.id === selectedChannelId.value);
  if (idx >= 0) {
    props.config.remoteImChannels.splice(idx, 1);
    const nextIdx = Math.min(idx, channels.value.length - 1);
    selectedChannelId.value = nextIdx >= 0 ? channels.value[nextIdx].id : "";
  }
}

function syncCredentialJson(channel: RemoteImChannelConfig) {
  const raw = String(credentialDrafts.value[channel.id] || "").trim();
  if (!raw) {
    channel.credentials = {};
    return;
  }
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("credentials json must be object");
    }
    channel.credentials = parsed as Record<string, unknown>;
  } catch (error) {
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function loadNapcatCredentials(channel: RemoteImChannelConfig) {
  suppressCredentialSync.value = true;
  const creds = channel.credentials || {};
  napcatCredentials.value = {
    wsHost: String(creds.wsHost || "0.0.0.0"),
    wsPort: Number(creds.wsPort) || 6199,
    wsToken: String(creds.wsToken || ""),
  };
  nextTick(() => {
    suppressCredentialSync.value = false;
  });
}

function loadDingtalkCredentials(channel: RemoteImChannelConfig) {
  suppressCredentialSync.value = true;
  const creds = channel.credentials || {};
  dingtalkCredentials.value = {
    clientId: String(creds.clientId || creds.clientID || ""),
    clientSecret: String(creds.clientSecret || creds.appSecret || ""),
  };
  nextTick(() => {
    suppressCredentialSync.value = false;
  });
}

function resetNapcatCredentials() {
  if (!selectedChannel.value) return;
  loadNapcatCredentials(selectedChannel.value);
  // 同时更新 channelSnapshot 以清除 dirty 状态
  lastSavedChannelSnapshot.value = channelSnapshot.value;
}

async function saveChannels() {
  if (saving.value || !selectedChannel.value) return;
  if (selectedChannel.value.platform === "feishu") {
    syncCredentialJson(selectedChannel.value);
  }
  const savedId = selectedChannelId.value;
  saving.value = true;
  try {
    const result = await Promise.resolve(props.saveConfigAction());
    if (result) {
      if (channels.value.some((ch) => ch.id === savedId)) {
        selectedChannelId.value = savedId;
      }
      if (selectedChannel.value && selectedChannel.value.platform === "onebot_v11") {
        loadNapcatCredentials(selectedChannel.value);
        try {
          const status = await invokeTauri<ChannelConnectionStatus>(
            "remote_im_restart_channel",
            { channelId: selectedChannel.value.id },
          );
          channelStatus.value = status;
          channelRuntimeStates.value = {
            ...channelRuntimeStates.value,
            [selectedChannel.value.id]: status,
          };
        } catch (err) {
          console.warn("[RemoteImTab] restart channel failed:", err);
          void refreshChannelStatus();
        }
      }
      await nextTick();
      lastSavedChannelSnapshot.value = channelSnapshot.value;
    }
  } finally {
    saving.value = false;
  }
}

async function toggleChannelEnabled(channel: RemoteImChannelConfig, enabled: boolean) {
  const previousEnabled = channel.enabled;
  if (enabled) {
    const validationError = validateChannelBeforeEnable(channel);
    if (validationError) {
      props.setStatusAction(validationError);
      return;
    }
  }
  channel.enabled = enabled;
  saving.value = true;
  try {
    const result = await Promise.resolve(props.saveConfigAction());
    if (result) {
      if (channel.platform === "onebot_v11" || channel.platform === "dingtalk") {
        try {
          const status = await invokeTauri<ChannelConnectionStatus>(
            "remote_im_restart_channel",
            { channelId: channel.id },
          );
          channelStatus.value = status;
          channelRuntimeStates.value = {
            ...channelRuntimeStates.value,
            [channel.id]: status,
          };
        } catch (err) {
          console.warn("[RemoteImTab] restart channel failed:", err);
          void refreshChannelStatus();
        }
      }
      await nextTick();
      lastSavedChannelSnapshot.value = channelSnapshot.value;
    } else {
      channel.enabled = previousEnabled;
    }
  } catch (error) {
    channel.enabled = previousEnabled;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  } finally {
    saving.value = false;
  }
}

async function toggleSelectedChannelEnabled(enabled: boolean) {
  if (!selectedChannel.value) return;
  await toggleChannelEnabled(selectedChannel.value, enabled);
}

async function toggleContactAllowSend(item: RemoteImContact, enabled: boolean) {
  const oldValue = item.allowSend;
  item.allowSend = enabled;
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_allow_send", {
      input: { contactId: item.id, allowSend: enabled },
    });
    await refreshContacts();
  } catch (error) {
    item.allowSend = oldValue;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

async function toggleContactAllowReceive(item: RemoteImContact, enabled: boolean) {
  const oldValue = item.allowReceive;
  item.allowReceive = enabled;
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_allow_receive", {
      input: { contactId: item.id, allowReceive: enabled },
    });
    await refreshContacts();
  } catch (error) {
    item.allowReceive = oldValue;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function normalizeActivationMode(value: string): RemoteImContact["activationMode"] {
  const mode = String(value || "").trim().toLowerCase();
  if (mode === "always" || mode === "keyword") return mode;
  if (mode === "never") return "never";
  return "never";
}

function parseActivationKeywords(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of String(raw || "").split(/[,\n，]/)) {
    const keyword = item.trim();
    if (!keyword || seen.has(keyword)) continue;
    seen.add(keyword);
    out.push(keyword);
  }
  return out;
}

async function saveContactActivation(
  item: RemoteImContact,
  patch?: Partial<Pick<RemoteImContact, "activationMode" | "activationKeywords" | "activationCooldownSeconds">>,
) {
  const oldMode = item.activationMode;
  const oldKeywords = [...item.activationKeywords];
  const oldCooldown = item.activationCooldownSeconds;
  if (patch?.activationMode) item.activationMode = patch.activationMode;
  if (patch?.activationKeywords) item.activationKeywords = [...patch.activationKeywords];
  if (typeof patch?.activationCooldownSeconds === "number") {
    item.activationCooldownSeconds = Math.max(0, Math.floor(patch.activationCooldownSeconds));
  }
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_activation", {
      input: {
        contactId: item.id,
        activationMode: item.activationMode,
        activationKeywords: item.activationKeywords,
        activationCooldownSeconds: item.activationCooldownSeconds,
      },
    });
    await refreshContacts();
  } catch (error) {
    item.activationMode = oldMode;
    item.activationKeywords = oldKeywords;
    item.activationCooldownSeconds = oldCooldown;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function onContactActivationModeChange(item: RemoteImContact, modeRaw: string) {
  const mode = normalizeActivationMode(modeRaw);
  void saveContactActivation(item, { activationMode: mode });
}

function onContactActivationCooldownChange(item: RemoteImContact, cooldownSeconds: number) {
  void saveContactActivation(item, {
    activationCooldownSeconds: Math.max(0, Math.floor(Number(cooldownSeconds) || 0)),
  });
}

function onContactActivationKeywordsBlur(item: RemoteImContact) {
  const raw = contactKeywordDrafts.value[item.id] ?? item.activationKeywords.join(", ");
  const keywords = parseActivationKeywords(raw);
  contactKeywordDrafts.value[item.id] = keywords.join(", ");
  void saveContactActivation(item, { activationKeywords: keywords });
}

async function refreshContacts() {
  contactsLoading.value = true;
  contactsError.value = "";
  try {
    contacts.value = await invokeTauri<RemoteImContact[]>("remote_im_list_contacts");
    for (const item of contacts.value) {
      item.activationMode = normalizeActivationMode(item.activationMode || "never");
      item.activationKeywords = Array.isArray(item.activationKeywords) ? item.activationKeywords : [];
      item.activationCooldownSeconds = Math.max(0, Number(item.activationCooldownSeconds || 0));
      contactKeywordDrafts.value[item.id] = item.activationKeywords.join(", ");
    }
  } catch (error) {
    contactsError.value = String(error);
  } finally {
    contactsLoading.value = false;
  }
}

function contactDisplayName(item: RemoteImContact): string {
  const remark = String(item.remarkName || "").trim();
  if (remark) return remark;
  const remoteName = String(item.remoteContactName || "").trim();
  if (remoteName) return remoteName;
  return item.remoteContactId;
}

async function deleteContact(contactId: string) {
  try {
    await invokeTauri<boolean>("remote_im_delete_contact", { input: { contactId } });
    await refreshContacts();
  } catch (error) {
    contactsError.value = String(error);
  }
}

async function saveRemark(contactId: string, remarkName: string) {
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_remark", {
      input: {
        contactId,
        remarkName,
      },
    });
    await refreshContacts();
  } catch (error) {
    contactsError.value = String(error);
  }
}

function onRemarkChange(contactId: string, event: Event) {
  const value = (event.target as HTMLInputElement).value || "";
  void saveRemark(contactId, value.trim());
}

function formatRelativeTime(raw?: string): string {
  const now = Date.now();
  const ts = Date.parse(String(raw || ""));
  if (!Number.isFinite(ts)) return "-";
  const diff = Math.max(0, now - ts);
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  if (diff < minute) return t("config.remoteIm.justNow");
  if (diff < hour) return t("config.remoteIm.minutesAgo", { count: Math.floor(diff / minute) });
  if (diff < day) return t("config.remoteIm.hoursAgo", { count: Math.floor(diff / hour) });
  if (diff < 7 * day) return t("config.remoteIm.daysAgo", { count: Math.floor(diff / day) });
  const d = new Date(ts);
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

async function refreshChannelStatus() {
  if (!selectedChannel.value) return;
  const channelId = selectedChannel.value.id;
  try {
    const status = await invokeTauri<ChannelConnectionStatus>("remote_im_get_channel_status", {
      channelId,
    });
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = status;
    }
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: status,
    };
  } catch (error) {
    console.error("[RemoteImTab] refreshChannelStatus failed:", error);
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = null;
    }
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: null,
    };
  }
}

async function refreshChannelStatusById(channelId: string) {
  try {
    const status = await invokeTauri<ChannelConnectionStatus>("remote_im_get_channel_status", { channelId });
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: status,
    };
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = status;
    }
  } catch {
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: null,
    };
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = null;
    }
  }
}

async function refreshAllChannelStatuses() {
  const jobs = channels.value
    .filter((item) => item.platform === "onebot_v11" || item.platform === "dingtalk")
    .map((item) => refreshChannelStatusById(item.id));
  await Promise.all(jobs);
}

function channelStatusPreview(channel: RemoteImChannelConfig): string {
  if (channel.platform === "dingtalk") {
    const status = channelRuntimeStates.value[channel.id];
    if (!channel.enabled) return t("config.remoteIm.disabledState");
    if (!status) return t("config.remoteIm.dingtalkConnectingState");
    if (status.connected) return t("config.remoteIm.connected");
    return t("config.remoteIm.dingtalkConnectingState");
  }
  if (channel.platform === "feishu") {
    return channel.enabled
      ? t("config.remoteIm.feishuSendOnlyState")
      : t("config.remoteIm.disabledState");
  }
  if (channel.platform !== "onebot_v11") {
    return channel.enabled ? t("config.remoteIm.enabledState") : t("config.remoteIm.disabledState");
  }
  const status = channelRuntimeStates.value[channel.id];
  if (!status) {
    return channel.enabled ? t("config.remoteIm.serverNotStarted") : t("config.remoteIm.disabledState");
  }
  if (status.connected) {
    return t("config.remoteIm.connected");
  }
  return status.listenAddr ? t("config.remoteIm.waitingForConnection") : t("config.remoteIm.serverNotStarted");
}

function channelListStatusBadgeText(channel: RemoteImChannelConfig): string {
  if (!channel.enabled) return t("config.remoteIm.disabledState");
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk") {
    const status = channelRuntimeStates.value[channel.id];
    if (status?.connected) return t("config.remoteIm.connected");
    return t("config.remoteIm.enabledState");
  }
  return t("config.remoteIm.enabledState");
}

function channelListStatusBadgeClass(channel: RemoteImChannelConfig): string {
  if (!channel.enabled) return "badge-ghost";
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk") {
    const status = channelRuntimeStates.value[channel.id];
    return status?.connected ? "badge-success" : "badge-warning";
  }
  return "badge-success";
}

async function refreshChannelLogs() {
  if (!selectedChannel.value) return;
  channelLogsLoading.value = true;
  try {
    channelLogs.value = await invokeTauri<ChannelLogEntry[]>("remote_im_get_channel_logs", {
      channelId: selectedChannel.value.id,
    });
  } catch {
    channelLogs.value = [];
  } finally {
    channelLogsLoading.value = false;
  }
}

function openChannelLogsModal() {
  if (!selectedChannel.value) return;
  channelLogsModalOpen.value = true;
  void refreshChannelLogs();
}

function closeChannelLogsModal() {
  channelLogsModalOpen.value = false;
}

function formatLogTime(timestamp: string): string {
  const d = new Date(timestamp);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;
}

watch(
  channels,
  (list) => {
    if (channelPage.value > channelPageCount.value) {
      channelPage.value = channelPageCount.value;
    }
    if (list.length > 0 && !list.some((ch) => ch.id === selectedChannelId.value)) {
      selectedChannelId.value = list[0].id;
    }
    for (const item of list) {
      if (!(item.id in credentialDrafts.value)) {
        credentialDrafts.value[item.id] = JSON.stringify(item.credentials || {}, null, 2);
      }
    }
  },
  { immediate: true },
);

watch(selectedChannelId, () => {
  contactPage.value = 1;
  const selectedIndex = channels.value.findIndex((item) => item.id === selectedChannelId.value);
  if (selectedIndex >= 0) {
    channelPage.value = Math.floor(selectedIndex / CHANNELS_PAGE_SIZE) + 1;
  }
  if (selectedChannel.value) {
    credentialDrafts.value[selectedChannel.value.id] = JSON.stringify(
      selectedChannel.value.credentials || {}, null, 2,
    );
    if (selectedChannel.value.platform === "onebot_v11") {
      loadNapcatCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "dingtalk") {
      loadDingtalkCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else {
      channelStatus.value = null;
    }
    if (channelLogsModalOpen.value) {
      void refreshChannelLogs();
    } else {
      channelLogs.value = [];
    }
  }
  lastSavedChannelSnapshot.value = channelSnapshot.value;
});

watch(currentChannelContacts, () => {
  if (contactPage.value > contactPageCount.value) {
    contactPage.value = contactPageCount.value;
  }
});

watch(napcatCredentials, () => {
  if (suppressCredentialSync.value) return;
  if (selectedChannel.value && selectedChannel.value.platform === "onebot_v11") {
    selectedChannel.value.credentials = {
      wsHost: napcatCredentials.value.wsHost || "0.0.0.0",
      wsPort: napcatCredentials.value.wsPort || 6199,
      wsToken: napcatCredentials.value.wsToken || "",
    };
  }
}, { deep: true });

watch(dingtalkCredentials, () => {
  if (suppressCredentialSync.value) return;
  if (selectedChannel.value && selectedChannel.value.platform === "dingtalk") {
    const current = selectedChannel.value.credentials || {};
    selectedChannel.value.credentials = {
      ...current,
      clientId: dingtalkCredentials.value.clientId || "",
      clientSecret: dingtalkCredentials.value.clientSecret || "",
    };
  }
}, { deep: true });

onMounted(() => {
  if (channels.value.length > 0 && !selectedChannelId.value) {
    selectedChannelId.value = channels.value[0].id;
  }
  // channels watcher (immediate: true) 已在 selectedChannelId watcher 注册前
  // 就同步设置了 selectedChannelId，导致 selectedChannelId watcher 不会触发。
  // 这里需要手动执行初始化操作：加载 napcatCredentials 和刷新连接状态/日志。
  if (selectedChannel.value) {
    credentialDrafts.value[selectedChannel.value.id] = JSON.stringify(
      selectedChannel.value.credentials || {}, null, 2,
    );
    if (selectedChannel.value.platform === "onebot_v11") {
      loadNapcatCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "dingtalk") {
      loadDingtalkCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    }
  }
  void refreshAllChannelStatuses();
  channelStatusTimer = setInterval(() => {
    void refreshAllChannelStatuses();
    void refreshContacts();
    if (channelLogsModalOpen.value) {
      void refreshChannelLogs();
    }
  }, 3000);
  lastSavedChannelSnapshot.value = channelSnapshot.value;
  void refreshContacts();
});

onUnmounted(() => {
  if (channelStatusTimer) {
    clearInterval(channelStatusTimer);
    channelStatusTimer = null;
  }
});
</script>
