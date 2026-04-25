<template>
  <div class="grid gap-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body gap-3 p-4">
        <div>
          <div class="card-title text-base mb-1">Codex 登录</div>
          <div class="text-xs opacity-60">本地读取只检查凭证文件本身；运行时如果 token 过期，再由后端按需要刷新。</div>
        </div>

        <div class="grid gap-3">
          <label class="flex flex-col gap-1">
            <span class="text-sm font-medium">认证方式</span>
            <select v-model="provider.codexAuthMode" class="select select-bordered select-sm" @change="void refreshCodexAuthStatus()">
              <option v-for="item in codexAuthModeOptions" :key="item.value" :value="item.value">
                {{ item.label }}
              </option>
            </select>
          </label>
        </div>

        <div v-if="provider.codexAuthMode === 'read_local'" class="grid gap-3">
          <label class="flex flex-col gap-1">
            <span class="text-sm font-medium">本地凭证路径</span>
            <input v-model="provider.codexLocalAuthPath" class="input input-bordered input-sm" :placeholder="DEFAULT_CODEX_LOCAL_AUTH_PATH" />
          </label>
          <div class="flex gap-2">
            <button class="btn btn-sm bg-base-200" type="button" :disabled="codexAuthBusy" @click="checkLocalCodexAuth">
              检查本地登录
            </button>
          </div>
        </div>

        <div v-else class="grid gap-3">
          <div class="text-sm opacity-70">应用会打开浏览器完成 OAuth 登录，凭证存到应用私有目录。</div>
          <div class="flex flex-wrap gap-2">
            <button class="btn btn-sm btn-primary" type="button" :disabled="codexAuthBusy" @click="startCodexOAuthLogin">
              <span v-if="codexAuthBusy" class="loading loading-spinner loading-xs"></span>
              <span>登录 Codex</span>
            </button>
            <button v-if="currentCodexAuthStatus?.authenticated" class="btn btn-sm btn-outline btn-error" type="button" :disabled="codexAuthBusy" @click="logoutCodex">
              退出登录
            </button>
          </div>
        </div>

        <div class="rounded-box border border-base-300 bg-base-200/50 p-3 text-sm">
          <div class="font-medium">状态：{{ currentCodexAuthStatus?.status || "unknown" }}</div>
          <div class="mt-1 opacity-80">{{ currentCodexAuthStatus?.message || "尚未检查登录状态。" }}</div>
          <div class="mt-2 text-xs opacity-70">解析路径：{{ currentCodexAuthStatus?.localAuthPath || provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH }}</div>
          <div v-if="currentCodexAuthStatus?.email" class="text-xs opacity-70">账号：{{ currentCodexAuthStatus.email }}</div>
          <div v-if="currentCodexAuthStatus?.accountId" class="text-xs opacity-70">Account ID：{{ currentCodexAuthStatus.accountId }}</div>
          <div v-if="currentCodexAuthStatus?.expiresAt" class="text-xs opacity-70">过期时间：{{ currentCodexAuthStatus.expiresAt }}</div>
          <div v-if="currentCodexAuthStatus?.managedAuthPath && provider.codexAuthMode === 'managed_oauth'" class="text-xs opacity-70">
            托管凭证：{{ currentCodexAuthStatus.managedAuthPath }}
          </div>
          <div class="mt-3 rounded-box border border-base-300 bg-base-100/70 p-3">
            <div class="flex items-center justify-between gap-2">
              <div>
                <div class="text-xs font-medium uppercase tracking-wide opacity-70">Rate Limits</div>
                <div class="text-[11px] opacity-60">同步官方 usage 快照，长窗口按 weekly 口径展示。</div>
              </div>
              <span v-if="currentCodexRateLimitBusy" class="loading loading-spinner loading-xs"></span>
            </div>

            <div v-if="currentCodexRateLimitError" class="mt-2 text-xs text-error">
              {{ currentCodexRateLimitError }}
            </div>

            <div v-else-if="currentCodexRateLimitSnapshots.length" class="mt-2 grid gap-2">
              <div v-if="currentCodexRateLimitPlanType" class="text-xs opacity-70">
                套餐：{{ formatCodexPlanType(currentCodexRateLimitPlanType) }}
              </div>
              <div class="text-xs opacity-70">
                快照数：{{ currentCodexRateLimitSnapshots.length }}
              </div>
              <div class="text-xs opacity-70 break-all">
                接口：{{ currentCodexRateLimitQuery?.usageUrl || "-" }}
              </div>

              <div
                v-for="snapshot in currentCodexRateLimitSnapshots"
                :key="`${snapshot.limitId || 'unknown'}-${snapshot.limitName || 'unnamed'}`"
                class="rounded-box border border-base-300 bg-base-100 p-3"
              >
                <div class="mb-2 font-medium">
                  {{ resolveCodexSnapshotTitle(snapshot) }}
                </div>

                <div v-if="snapshot.primary || snapshot.secondary" class="grid gap-2">
                  <div
                    v-if="snapshot.primary"
                    class="rounded-box border border-base-300 bg-base-200/60 px-3 py-2"
                  >
                    <div class="flex items-center justify-between gap-3">
                      <span class="font-medium">{{ resolveCodexWindowLabel(snapshot.primary, "5h") }}</span>
                      <span class="text-xs opacity-80">{{ formatCodexRemainingText(snapshot.primary) }}</span>
                    </div>
                    <div v-if="snapshot.primary.resetsAt" class="mt-1 text-[11px] opacity-70">
                      重置：{{ formatCodexResetAt(snapshot.primary.resetsAt) }}
                    </div>
                  </div>

                  <div
                    v-if="snapshot.secondary"
                    class="rounded-box border border-base-300 bg-base-200/60 px-3 py-2"
                  >
                    <div class="flex items-center justify-between gap-3">
                      <span class="font-medium">{{ resolveCodexWindowLabel(snapshot.secondary, "weekly") }}</span>
                      <span class="text-xs opacity-80">{{ formatCodexRemainingText(snapshot.secondary) }}</span>
                    </div>
                    <div v-if="snapshot.secondary.resetsAt" class="mt-1 text-[11px] opacity-70">
                      重置：{{ formatCodexResetAt(snapshot.secondary.resetsAt) }}
                    </div>
                  </div>
                </div>

                <div
                  v-else
                  class="rounded-box border border-dashed border-base-300 bg-base-100/60 px-3 py-2 text-xs opacity-70"
                >
                  当前 bucket 未返回窗口数据
                </div>
              </div>

              <div v-if="currentCodexRateLimitCredits" class="text-xs opacity-70">
                Credits：{{ formatCodexCredits(currentCodexRateLimitCredits) }}
              </div>
            </div>

            <div v-else class="mt-2 text-xs opacity-70">
              {{ codexRateLimitPlaceholder }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body gap-3 p-4">
        <div class="flex items-center justify-between gap-2">
          <div>
            <div class="card-title text-base mb-1">Codex 模型</div>
            <div class="text-xs opacity-60">Codex 只允许设置思维强度，其余参数保持默认。</div>
          </div>
          <div class="flex gap-2">
            <button class="btn btn-sm bg-base-200" type="button" :class="{ loading: refreshingModels }" :disabled="refreshingModels" @click="$emit('refreshModels')">
              <span>刷新模型</span>
            </button>
            <button class="btn btn-sm bg-base-200" type="button" @click="addModelCard">
              <span>新增模型</span>
            </button>
          </div>
        </div>

        <div class="text-[11px] text-error">{{ modelRefreshError || " " }}</div>

        <div class="grid gap-3">
          <div v-for="modelCard in provider.models" :key="modelCard.id" class="card border border-base-300 bg-base-200/50">
            <div class="card-body gap-3 p-4">
              <div class="flex items-start justify-between gap-2">
                <button class="min-w-0 flex-1 text-left" type="button" @click="$emit('selectModel', modelCard.id)">
                  <div class="card-title text-base mb-1">{{ `${provider.name || provider.id}/${modelCard.model || "未命名模型"}` }}</div>
                </button>
                <button class="btn btn-sm btn-square btn-ghost" type="button" :class="provider.models.length <= 1 ? 'text-base-content/30' : 'text-error'" :disabled="provider.models.length <= 1" @click="removeModelCard(modelCard.id)">
                  <Trash2 class="h-3.5 w-3.5" />
                </button>
              </div>

              <div class="grid gap-3 md:grid-cols-2">
                <label class="flex flex-col gap-1">
                  <span class="text-sm font-medium">模型</span>
                  <select v-model="modelCard.model" class="select select-bordered select-sm" @change="syncCachedModels">
                    <option v-for="option in providerModelOptions" :key="`${modelCard.id}-${option}`" :value="option">
                      {{ option }}
                    </option>
                  </select>
                </label>

                <label class="flex flex-col gap-1">
                  <span class="text-sm font-medium">思维强度</span>
                  <select v-model="modelCard.reasoningEffort" class="select select-bordered select-sm">
                    <option v-for="item in reasoningEffortOptions" :key="item.value" :value="item.value">
                      {{ item.label }}
                    </option>
                  </select>
                </label>
              </div>

              <label class="flex items-center justify-between rounded-box border border-base-300 bg-base-200 px-3 py-2">
                <span class="text-sm">启用工具</span>
                <input v-model="modelCard.enableTools" type="checkbox" class="toggle toggle-sm" />
              </label>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { Trash2 } from "lucide-vue-next";
import type {
  ApiModelConfigItem,
  ApiProviderConfigItem,
  CodexAuthMode,
  CodexAuthStatus,
  CodexCreditsSnapshot,
  CodexRateLimitQueryResult,
  CodexRateLimitSnapshot,
  CodexRateLimitWindow,
} from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";
import { formatIsoToLocalDateTime } from "../../../../utils/time";

const DEFAULT_CODEX_BASE_URL = "https://chatgpt.com/backend-api/codex";
const DEFAULT_CODEX_AUTH_MODE: CodexAuthMode = "read_local";
const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
const DEFAULT_REASONING_EFFORT = "medium";
const DEFAULT_CODEX_MODELS = ["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.3-codex-spark", "gpt-5.2"];

const props = defineProps<{
  provider: ApiProviderConfigItem;
  selectedApiConfigId: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshError: string;
}>();

const emit = defineEmits<{
  (e: "refreshModels"): void;
  (e: "selectModel", modelId: string): void;
}>();

const codexAuthBusy = ref(false);
const codexAuthStatusByProvider = ref<Record<string, CodexAuthStatus>>({});
const codexAuthPollTimer = ref<number | null>(null);
const codexRateLimitQueryByProvider = ref<Record<string, CodexRateLimitQueryResult | null>>({});
const codexRateLimitBusyByProvider = ref<Record<string, boolean>>({});
const codexRateLimitErrorByProvider = ref<Record<string, string>>({});
const reasoningEffortOptions = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "超高" },
];
const codexAuthModeOptions: Array<{ value: CodexAuthMode; label: string }> = [
  { value: "read_local", label: "读取本地" },
  { value: "managed_oauth", label: "自行登录" },
];

const currentCodexAuthStatus = computed(() => codexAuthStatusByProvider.value[props.provider.id] ?? null);
const currentCodexRateLimitQuery = computed(() => codexRateLimitQueryByProvider.value[props.provider.id] ?? null);
const currentCodexRateLimitSnapshots = computed(() => currentCodexRateLimitQuery.value?.snapshots || []);
const currentCodexRateLimitPlanType = computed(() => {
  return (
    currentCodexRateLimitQuery.value?.preferredSnapshot?.planType
    || currentCodexRateLimitSnapshots.value[0]?.planType
    || ""
  );
});
const currentCodexRateLimitCredits = computed(() => {
  return (
    currentCodexRateLimitQuery.value?.preferredSnapshot?.credits
    || currentCodexRateLimitSnapshots.value.find((item) => item.credits)?.credits
    || null
  );
});
const currentCodexRateLimitError = computed(() => codexRateLimitErrorByProvider.value[props.provider.id] ?? "");
const currentCodexRateLimitBusy = computed(() => Boolean(codexRateLimitBusyByProvider.value[props.provider.id]));
const codexRateLimitPlaceholder = computed(() => {
  if (currentCodexRateLimitBusy.value) {
    return "正在同步 Codex 周用量。";
  }
  if (currentCodexAuthStatus.value?.status === "pending") {
    return "登录完成后会自动同步周用量。";
  }
  if (shouldSyncCodexRateLimits(currentCodexAuthStatus.value)) {
    return "尚未查询到 Codex 周用量。";
  }
  return "登录后会自动同步 Codex 周用量。";
});
const providerModelOptions = computed(() => {
  const current = (props.provider.models || []).map((item) => String(item.model || "").trim()).filter(Boolean);
  const cached = Array.isArray(props.provider.cachedModelOptions) ? props.provider.cachedModelOptions : [];
  return Array.from(new Set([...DEFAULT_CODEX_MODELS, ...props.modelOptions, ...cached, ...current].map((item) => String(item || "").trim()).filter(Boolean)));
});

function applyCodexDefaults() {
  props.provider.baseUrl = DEFAULT_CODEX_BASE_URL;
  props.provider.codexAuthMode = (String(props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE).trim() === "managed_oauth" ? "managed_oauth" : "read_local");
  props.provider.codexLocalAuthPath = String(props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH;
  props.provider.apiKeys = [];
  props.provider.models = (props.provider.models || []).map((model) => ({
    ...model,
    reasoningEffort: String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 128000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
  }));
}

function stopCodexAuthPolling() {
  if (codexAuthPollTimer.value !== null) {
    window.clearInterval(codexAuthPollTimer.value);
    codexAuthPollTimer.value = null;
  }
}

function storeCodexAuthStatus(status: CodexAuthStatus) {
  codexAuthStatusByProvider.value = {
    ...codexAuthStatusByProvider.value,
    [status.providerId]: status,
  };
  if (status.authenticated || status.status === "error" || status.status === "expired") {
    stopCodexAuthPolling();
  }
}

function setCodexRateLimitBusy(providerId: string, busy: boolean) {
  codexRateLimitBusyByProvider.value = {
    ...codexRateLimitBusyByProvider.value,
    [providerId]: busy,
  };
}

function storeCodexRateLimitSnapshot(providerId: string, result: CodexRateLimitQueryResult | null) {
  codexRateLimitQueryByProvider.value = {
    ...codexRateLimitQueryByProvider.value,
    [providerId]: result,
  };
  codexRateLimitErrorByProvider.value = {
    ...codexRateLimitErrorByProvider.value,
    [providerId]: "",
  };
}

function storeCodexRateLimitError(providerId: string, error: unknown) {
  codexRateLimitQueryByProvider.value = {
    ...codexRateLimitQueryByProvider.value,
    [providerId]: null,
  };
  codexRateLimitErrorByProvider.value = {
    ...codexRateLimitErrorByProvider.value,
    [providerId]: String(error || "Codex 周用量查询失败。"),
  };
}

function clearCodexRateLimits(providerId: string) {
  codexRateLimitQueryByProvider.value = {
    ...codexRateLimitQueryByProvider.value,
    [providerId]: null,
  };
  codexRateLimitErrorByProvider.value = {
    ...codexRateLimitErrorByProvider.value,
    [providerId]: "",
  };
  setCodexRateLimitBusy(providerId, false);
}

function codexAuthFailureStatus(error: unknown): CodexAuthStatus {
  const message = String(error || "Codex 登录状态检查失败。");
  const normalized = message.toLowerCase();
  const status = normalized.includes("auth.json") || normalized.includes("读取本地 codex 凭证失败") ? "unauthenticated" : "error";
  return {
    providerId: props.provider.id,
    authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
    authenticated: false,
    status,
    message,
    email: "",
    accountId: "",
    accessTokenPreview: "",
    localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
    managedAuthPath: "",
    expiresAt: "",
  };
}

function shouldSyncCodexRateLimits(status?: CodexAuthStatus | null): boolean {
  return Boolean(status?.authenticated || status?.status === "expired");
}

async function refreshCodexRateLimits(status?: CodexAuthStatus | null) {
  const providerId = String(props.provider.id || "").trim();
  if (!providerId) return null;
  if (!shouldSyncCodexRateLimits(status)) {
    clearCodexRateLimits(providerId);
    return null;
  }

  setCodexRateLimitBusy(providerId, true);
  try {
    const result = await invokeTauri<CodexRateLimitQueryResult>("codex_get_rate_limits", {
      input: {
        providerId,
        authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
        localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        baseUrl: props.provider.baseUrl || DEFAULT_CODEX_BASE_URL,
      },
    });
    storeCodexRateLimitSnapshot(providerId, result);
    return result;
  } catch (error) {
    storeCodexRateLimitError(providerId, error);
    return null;
  } finally {
    setCodexRateLimitBusy(providerId, false);
  }
}

async function refreshCodexAuthStatus() {
  applyCodexDefaults();
  try {
    const status = await invokeTauri<CodexAuthStatus>("codex_get_auth_status", {
      input: {
        providerId: props.provider.id,
        authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
        localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
      },
    });
    storeCodexAuthStatus(status);
    await refreshCodexRateLimits(status);
    return status;
  } catch (error) {
    const status = codexAuthFailureStatus(error);
    storeCodexAuthStatus(status);
    clearCodexRateLimits(props.provider.id);
    return status;
  }
}

function startCodexAuthPolling() {
  stopCodexAuthPolling();
  codexAuthPollTimer.value = window.setInterval(() => {
    void refreshCodexAuthStatus();
  }, 2500);
}

async function checkLocalCodexAuth() {
  if (codexAuthBusy.value) return;
  codexAuthBusy.value = true;
  try {
    await refreshCodexAuthStatus();
  } finally {
    codexAuthBusy.value = false;
  }
}

async function startCodexOAuthLogin() {
  applyCodexDefaults();
  codexAuthBusy.value = true;
  try {
    const status = await invokeTauri<CodexAuthStatus>("codex_start_oauth_login", {
      input: {
        providerId: props.provider.id,
      },
    });
    storeCodexAuthStatus(status);
    startCodexAuthPolling();
  } catch (error) {
    storeCodexAuthStatus(codexAuthFailureStatus(error));
  } finally {
    codexAuthBusy.value = false;
  }
}

async function logoutCodex() {
  codexAuthBusy.value = true;
  try {
    await invokeTauri("codex_logout", {
      input: {
        providerId: props.provider.id,
      },
    });
    stopCodexAuthPolling();
    clearCodexRateLimits(props.provider.id);
    storeCodexAuthStatus({
      providerId: props.provider.id,
      authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
      authenticated: false,
      status: "unauthenticated",
      message: "已退出 Codex 登录。",
      email: "",
      accountId: "",
      accessTokenPreview: "",
      localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
      managedAuthPath: "",
      expiresAt: "",
    });
  } catch (error) {
    storeCodexAuthStatus(codexAuthFailureStatus(error));
  } finally {
    codexAuthBusy.value = false;
  }
}

function syncCachedModels() {
  props.provider.cachedModelOptions = Array.from(new Set(providerModelOptions.value));
}

function resolveCodexWindowLabel(window?: CodexRateLimitWindow | null, fallback = "weekly"): string {
  const minutes = Number(window?.windowDurationMins ?? 0);
  if (!Number.isFinite(minutes) || minutes <= 0) {
    return fallback;
  }

  const minutesPerHour = 60;
  const minutesPerDay = 24 * minutesPerHour;
  const minutesPerWeek = 7 * minutesPerDay;
  const minutesPerMonth = 30 * minutesPerDay;
  const roundingBiasMinutes = 3;
  const normalized = Math.max(0, minutes);

  if (normalized <= minutesPerDay + roundingBiasMinutes) {
    const hours = Math.max(1, Math.floor((normalized + roundingBiasMinutes) / minutesPerHour));
    return `${hours}h`;
  }
  if (normalized <= minutesPerWeek + roundingBiasMinutes) {
    return "weekly";
  }
  if (normalized <= minutesPerMonth + roundingBiasMinutes) {
    return "monthly";
  }
  return "annual";
}

function resolveCodexSnapshotTitle(snapshot?: CodexRateLimitSnapshot | null): string {
  const limitName = String(snapshot?.limitName || "").trim();
  if (limitName) return limitName;
  const limitId = String(snapshot?.limitId || "").trim();
  if (!limitId || limitId === "codex") return "Codex";
  return limitId;
}

function formatCodexRemainingText(window?: CodexRateLimitWindow | null): string {
  const usedPercent = Number(window?.usedPercent ?? 0);
  const remaining = Math.max(0, Math.min(100, 100 - usedPercent));
  return `${Math.round(remaining)}% left`;
}

function formatCodexResetAt(unixSeconds?: number | null): string {
  if (!unixSeconds || unixSeconds <= 0) return "";
  return formatIsoToLocalDateTime(new Date(unixSeconds * 1000).toISOString(), "");
}

function formatCodexPlanType(planType?: string | null): string {
  const value = String(planType || "").trim();
  if (!value) return "-";
  return value.replace(/_/g, " ");
}

function formatCodexCredits(credits?: CodexCreditsSnapshot | null): string {
  if (!credits?.hasCredits) {
    const balance = String(credits?.balance || "").trim();
    return balance ? `未启用（balance ${balance}）` : "未启用";
  }
  if (credits.unlimited) return "Unlimited";
  const balance = String(credits.balance || "").trim();
  return balance ? `${balance} credits` : "已启用";
}

function createModel(seed: string, modelName: string): ApiModelConfigItem {
  return {
    id: `api-model-${seed}`,
    model: modelName,
    enableImage: false,
    enableTools: true,
    reasoningEffort: DEFAULT_REASONING_EFFORT,
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 128000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
  };
}

function addModelCard() {
  applyCodexDefaults();
  const existing = new Set((props.provider.models || []).map((item) => String(item.model || "").trim()).filter(Boolean));
  const nextModel = providerModelOptions.value.find((item) => !existing.has(item)) || providerModelOptions.value[0] || DEFAULT_CODEX_MODELS[0];
  const seed = Date.now().toString();
  const model = createModel(seed, nextModel);
  props.provider.models.push(model);
  syncCachedModels();
  emit("selectModel", model.id);
}

function removeModelCard(modelId: string) {
  if ((props.provider.models || []).length <= 1) return;
  const idx = props.provider.models.findIndex((item) => item.id === modelId);
  if (idx < 0) return;
  props.provider.models.splice(idx, 1);
  const fallback = props.provider.models[Math.max(0, idx - 1)] ?? props.provider.models[0];
  if (fallback) {
    emit("selectModel", fallback.id);
  }
}

watch(
  () => props.provider.id,
  () => {
    syncCachedModels();
    void refreshCodexAuthStatus();
  },
  { immediate: true },
);

onUnmounted(() => {
  stopCodexAuthPolling();
});
</script>
