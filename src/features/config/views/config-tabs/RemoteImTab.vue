<template>
  <div class="grid gap-2">
    <!-- 渠道选择器 -->
    <label class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.remoteIm.title") }}</span></div>
      <div class="flex gap-1">
        <select v-model="selectedChannelId" class="select select-bordered select-sm flex-1">
          <option v-for="(ch, idx) in channels" :key="ch.id" :value="ch.id">
            #{{ idx + 1 }} {{ ch.name || "-" }} ({{ platformLabel(ch.platform) }})
          </option>
        </select>
        <button class="btn btn-sm btn-square bg-base-100" :title="t('config.remoteIm.addChannel')" @click="addChannel">
          <Plus class="h-3.5 w-3.5" />
        </button>
        <button
          class="btn btn-sm btn-square bg-base-100"
          :class="!selectedChannel ? 'text-base-content/30 cursor-not-allowed' : ''"
          :title="t('common.delete')"
          :disabled="!selectedChannel"
          @click="removeSelectedChannel"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </button>
        <button
          class="btn btn-sm btn-square"
          :class="channelDirty ? 'btn-primary' : 'bg-base-100'"
          :disabled="!selectedChannel || !channelDirty || saving"
          :title="saving ? t('config.api.saving') : t('common.save')"
          @click="saveChannels"
        >
          <Save v-if="!saving" class="h-3.5 w-3.5" />
          <span v-else class="loading loading-spinner loading-sm"></span>
        </button>
      </div>
    </label>
    <div class="text-sm opacity-60">{{ t("config.remoteIm.hint") }}</div>

    <!-- 渠道详情卡片 -->
    <div v-if="selectedChannel" class="card bg-base-100 card-border border-base-300 card-sm">
      <div class="card-body gap-3">
        <div class="flex items-center gap-2">
          <span class="font-medium">{{ selectedChannel.name || "-" }}</span>
          <span class="badge badge-ghost badge-sm">{{ platformLabel(selectedChannel.platform) }}</span>
          <span v-if="!selectedChannel.enabled" class="badge badge-warning badge-sm">{{ t("config.remoteIm.disabledState") }}</span>
        </div>

        <div class="flex flex-col gap-3">
          <!-- 渠道名称 -->
          <div class="border-b border-base-content/5 border-dashed pb-3">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.remoteIm.channelName") }}</div>
            <input v-model="selectedChannel.name" class="input input-bordered input-sm w-full" />
          </div>

          <!-- 平台 -->
          <div class="border-b border-base-content/5 border-dashed pb-3">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.remoteIm.platform") }}</div>
            <select v-model="selectedChannel.platform" class="select select-bordered select-sm w-full">
              <option value="napcat">NapCat</option>
              <option value="feishu">Feishu</option>
              <option value="dingtalk">DingTalk</option>
            </select>
          </div>

          <!-- 启用 -->
          <div class="border-b border-base-content/5 border-dashed pb-3">
            <label class="flex cursor-pointer items-center gap-2 select-none">
              <input v-model="selectedChannel.enabled" type="checkbox" class="checkbox checkbox-sm" />
              <span class="text-sm">{{ t("config.remoteIm.enabled") }}</span>
            </label>
          </div>

          <!-- 能力配置 -->
          <div class="border-b border-base-content/5 border-dashed pb-3">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.remoteIm.capabilities") }}</div>
            <div class="flex flex-col gap-2">
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.activateAssistant" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.activateAssistant") }}</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.receiveFiles" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.receiveFiles") }}</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.allowSendFiles" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.allowSendFiles") }}</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.allowProactiveSend" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.allowProactiveSend") }}</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.streamingSend" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.streamingSend") }}</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2 select-none">
                <input v-model="selectedChannel.showToolCalls" type="checkbox" class="checkbox checkbox-sm" />
                <span class="text-sm">{{ t("config.remoteIm.showToolCalls") }}</span>
              </label>
            </div>
          </div>

          <!-- 默认回复策略 -->
          <div class="border-b border-base-content/5 border-dashed pb-3">
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.remoteIm.defaultReplyMode") }}</div>
            <select v-model="selectedChannel.defaultReplyMode" class="select select-bordered select-sm w-full">
              <option value="none">{{ t("config.remoteIm.replyMode.none") }}</option>
              <option value="reply_once">{{ t("config.remoteIm.replyMode.replyOnce") }}</option>
              <option value="always">{{ t("config.remoteIm.replyMode.always") }}</option>
            </select>
          </div>

          <!-- 凭证 -->
          <div>
            <div class="text-[11px] opacity-40 uppercase tracking-wide mb-2">{{ t("config.remoteIm.credentialsJson") }}</div>
            <textarea
              v-model="credentialDrafts[selectedChannel.id]"
              class="textarea textarea-bordered textarea-sm w-full min-h-24 font-mono text-xs"
              spellcheck="false"
              @blur="syncCredentialJson(selectedChannel)"
            />
          </div>
        </div>
      </div>
    </div>

    <div v-else class="card bg-base-100 card-border border-base-300 card-sm">
      <div class="card-body text-center py-12">
        <div class="text-sm opacity-40">{{ t("config.remoteIm.empty") }}</div>
      </div>
    </div>

    <!-- 联系人列表 -->
    <div class="card bg-base-100 card-border border-base-300 card-sm">
      <div class="card-body gap-3">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">{{ t("config.remoteIm.contactsTitle") }} ({{ contacts.length }})</span>
          <button class="btn btn-sm btn-ghost" :class="{ loading: contactsLoading }" @click="refreshContacts">{{ t("common.refresh") }}</button>
        </div>

        <div v-if="contactsError" class="text-xs text-error">{{ contactsError }}</div>
        <div v-if="contacts.length === 0" class="text-sm opacity-60 text-center py-4">
          {{ t("config.remoteIm.contactsEmpty") }}
        </div>
        <div v-else class="flex flex-col gap-4">
          <div v-for="group in groupedContacts" :key="group.channelId">
            <div class="text-xs opacity-60 mb-2">{{ group.platformLabel }} · {{ group.channelName }}
              <span v-if="!group.channelEnabled" class="badge badge-xs badge-warning ml-1">{{ t("config.remoteIm.disabledState") }}</span>
            </div>
            <div class="flex flex-col">
              <div v-for="item in group.contacts" :key="item.id" class="flex items-center justify-between gap-2 border-b border-base-content/5 border-dashed py-2">
                <div class="flex items-center gap-2 flex-1 min-w-0">
                  <div v-if="item.hasNewMessage" class="w-2 h-2 rounded-full bg-error shrink-0" />
                  <div v-else class="w-2 h-2 shrink-0" />
                  <span class="truncate">{{ contactDisplayName(item) }}</span>
                  <span class="badge badge-ghost badge-xs">{{ item.remoteContactType === "group" ? t("config.remoteIm.group") : t("config.remoteIm.private") }}</span>
                </div>
                <div class="flex items-center gap-2">
                  <button :class="replyModeButtonClass(item.replyMode)" @click="cycleReplyMode(item)">
                    {{ replyModeLabel(item.replyMode) }}
                  </button>
                  <span class="text-xs opacity-50">{{ formatRelativeTime(item.lastMessageAt) }}</span>
                  <input
                    :value="item.remarkName || ''"
                    class="input input-xs input-bordered w-24"
                    :placeholder="t('config.remoteIm.remarkPlaceholder')"
                    @change="onRemarkChange(item.id, $event)"
                  />
                  <button class="btn btn-xs btn-ghost text-error" @click="deleteContact(item.id)">{{ t("common.delete") }}</button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Save, Trash2 } from "lucide-vue-next";
import { invokeTauri } from "../../../../services/tauri-api";
import type { AppConfig, RemoteImChannelConfig, RemoteImContact, RemoteImReplyMode } from "../../../../types/app";

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
const selectedChannelId = ref<string>("");
const channels = computed(() => props.config.remoteImChannels || []);

const selectedChannel = computed(() =>
  channels.value.find((ch) => ch.id === selectedChannelId.value) ?? null,
);

const channelSnapshot = computed(() => {
  const ch = selectedChannel.value;
  if (!ch) return "";
  return JSON.stringify({
    name: ch.name,
    platform: ch.platform,
    enabled: ch.enabled,
    activateAssistant: ch.activateAssistant,
    receiveFiles: ch.receiveFiles,
    allowSendFiles: ch.allowSendFiles,
    allowProactiveSend: ch.allowProactiveSend,
    streamingSend: ch.streamingSend,
    showToolCalls: ch.showToolCalls,
    defaultReplyMode: ch.defaultReplyMode,
    credentials: ch.credentials,
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

function platformLabelOf(platform: string): string {
  const value = String(platform || "").trim().toLowerCase();
  if (value === "feishu") return "Feishu";
  if (value === "dingtalk") return "DingTalk";
  return "NapCat";
}

function platformLabel(platform: RemoteImChannelConfig["platform"]): string {
  return platformLabelOf(platform);
}

function newChannel(): RemoteImChannelConfig {
  return {
    id: `remote-im-${Date.now()}`,
    name: "Remote IM",
    platform: "napcat",
    enabled: true,
    credentials: {},
    activateAssistant: true,
    defaultReplyMode: "none",
    receiveFiles: true,
    streamingSend: false,
    showToolCalls: false,
    allowProactiveSend: false,
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
    selectedChannelId.value = channels.value[0]?.id || "";
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

async function saveChannels() {
  if (saving.value || !selectedChannel.value) return;
  syncCredentialJson(selectedChannel.value);
  saving.value = true;
  try {
    await Promise.resolve(props.saveConfigAction());
    lastSavedChannelSnapshot.value = channelSnapshot.value;
  } finally {
    saving.value = false;
  }
}

async function refreshContacts() {
  contactsLoading.value = true;
  contactsError.value = "";
  try {
    contacts.value = await invokeTauri<RemoteImContact[]>("remote_im_list_contacts");
  } catch (error) {
    contactsError.value = String(error);
  } finally {
    contactsLoading.value = false;
  }
}

async function setReplyMode(contactId: string, replyMode: RemoteImReplyMode) {
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_reply_mode", {
      input: { contactId, replyMode },
    });
    await refreshContacts();
  } catch (error) {
    contactsError.value = String(error);
  }
}

function replyModeLabel(mode: RemoteImReplyMode): string {
  if (mode === "none") return t("config.remoteIm.replyMode.none");
  if (mode === "always") return t("config.remoteIm.replyMode.always");
  return t("config.remoteIm.replyMode.replyOnce");
}

function replyModeButtonClass(mode: RemoteImReplyMode): string {
  if (mode === "none") return "btn btn-xs btn-ghost opacity-50";
  if (mode === "always") return "btn btn-xs btn-info";
  return "btn btn-xs btn-outline btn-info";
}

function contactDisplayName(item: RemoteImContact): string {
  const remark = String(item.remarkName || "").trim();
  if (remark) return remark;
  const remoteName = String(item.remoteContactName || "").trim();
  if (remoteName) return remoteName;
  return item.remoteContactId;
}

function nextReplyMode(mode: RemoteImReplyMode): RemoteImReplyMode {
  if (mode === "none") return "reply_once";
  if (mode === "reply_once") return "always";
  return "none";
}

async function cycleReplyMode(item: RemoteImContact) {
  await setReplyMode(item.id, nextReplyMode(item.replyMode));
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
  if (diff < hour) return t("config.remoteIm.minutesAgo", { n: Math.floor(diff / minute) });
  if (diff < day) return t("config.remoteIm.hoursAgo", { n: Math.floor(diff / hour) });
  if (diff < 7 * day) return t("config.remoteIm.daysAgo", { n: Math.floor(diff / day) });
  const d = new Date(ts);
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

watch(
  channels,
  (list) => {
    const next: Record<string, string> = {};
    for (const item of list) {
      next[item.id] = JSON.stringify(item.credentials || {}, null, 2);
    }
    credentialDrafts.value = next;
    // 确保选中的渠道存在
    if (list.length > 0 && !list.some((ch) => ch.id === selectedChannelId.value)) {
      selectedChannelId.value = list[0].id;
    }
  },
  { immediate: true, deep: true },
);

watch(selectedChannelId, () => {
  lastSavedChannelSnapshot.value = channelSnapshot.value;
});

onMounted(() => {
  if (channels.value.length > 0 && !selectedChannelId.value) {
    selectedChannelId.value = channels.value[0].id;
  }
  lastSavedChannelSnapshot.value = channelSnapshot.value;
  void refreshContacts();
});
</script>
