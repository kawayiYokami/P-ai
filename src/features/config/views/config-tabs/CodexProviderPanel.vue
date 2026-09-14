<template>
  <div class="grid gap-3">
    <section>
      <ConfigTemplate v-model="codexTemplateValues" :groups="codexTemplateGroups" />
      <div class="grid gap-3">

        <div v-if="provider.codexAuthMode === 'custom_url'" class="grid gap-3">
          <div class="text-sm opacity-70">{{ t("config.api.codexCustomConfigHint") }}</div>
          <label class="flex flex-col gap-1">
            <span class="text-sm font-medium">{{ t("config.api.codexCustomApiKey") }}</span>
            <div class="flex items-center gap-2">
              <input
                v-model="provider.codexCustomApiKey"
                class="input input-bordered input-sm flex-1"
                :type="showCodexCustomApiKey ? 'text' : 'password'"
                placeholder="sk-..."
              />
              <button class="btn btn-sm btn-square bg-base-200" type="button" @click="showCodexCustomApiKey = !showCodexCustomApiKey">
                <EyeOff v-if="showCodexCustomApiKey" class="h-3.5 w-3.5" />
                <Eye v-else class="h-3.5 w-3.5" />
              </button>
            </div>
            <span class="text-xs opacity-60">{{ t("config.api.codexCustomApiKeyHint") }}</span>
          </label>
        </div>

        <div v-if="provider.codexAuthMode === 'read_local'" class="flex gap-2">
          <button class="btn btn-sm bg-base-200" type="button" :disabled="codexAuthBusy" @click="checkLocalCodexAuth">
            {{ t("config.api.codexCheckLocalLogin") }}
          </button>
        </div>

        <div v-else-if="provider.codexAuthMode === 'managed_oauth'" class="grid gap-3">
          <div class="text-sm opacity-70">{{ t("config.api.codexOAuthHint") }}</div>
          <div class="flex flex-wrap gap-2">
            <button class="btn btn-sm btn-primary" type="button" :disabled="codexAuthBusy" @click="startCodexOAuthLogin">
              <span v-if="codexAuthBusy" class="loading loading-spinner loading-xs"></span>
              <span>{{ t("config.api.codexLoginAction") }}</span>
            </button>
            <button v-if="currentCodexAuthStatus?.authenticated" class="btn btn-sm btn-outline btn-error" type="button" :disabled="codexAuthBusy" @click="logoutCodex">
              {{ t("config.api.codexLogout") }}
            </button>
          </div>
        </div>

        <div v-if="provider.codexAuthMode !== 'custom_url'" class="rounded-box border border-base-300 bg-base-200/50 p-3 text-sm">
          <div class="font-medium">{{ t("config.api.codexStatus", { status: currentCodexAuthStatus?.status || "unknown" }) }}</div>
          <div class="mt-1 opacity-80">{{ currentCodexAuthStatus?.message || t("config.api.codexStatusUnchecked") }}</div>
          <div class="mt-2 text-xs opacity-70">{{ t("config.api.codexResolvedPath", { path: currentCodexAuthStatus?.localAuthPath || provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH }) }}</div>
          <div v-if="currentCodexAuthStatus?.email" class="text-xs opacity-70">{{ t("config.api.codexAccount", { email: currentCodexAuthStatus.email }) }}</div>
          <div v-if="currentCodexAuthStatus?.accountId" class="text-xs opacity-70">Account ID：{{ currentCodexAuthStatus.accountId }}</div>
          <div v-if="currentCodexAuthStatus?.expiresAt" class="text-xs opacity-70">{{ t("config.api.codexExpiresAt", { time: currentCodexAuthStatus.expiresAt }) }}</div>
          <div v-if="currentCodexAuthStatus?.managedAuthPath && provider.codexAuthMode === 'managed_oauth'" class="text-xs opacity-70">
            {{ t("config.api.codexManagedAuthPath", { path: currentCodexAuthStatus.managedAuthPath }) }}
          </div>
          <div v-if="showCodexRateLimits" class="mt-3 rounded-box border border-base-300 bg-base-100/70 p-3">
            <div class="flex items-center justify-between gap-2">
              <div>
                <div class="text-xs font-medium uppercase tracking-wide opacity-70">Rate Limits</div>
                <div class="text-xs opacity-60">{{ t("config.api.codexRateLimitHint") }}</div>
              </div>
              <span v-if="currentCodexRateLimitBusy" class="loading loading-spinner loading-xs"></span>
            </div>

            <div v-if="currentCodexRateLimitError" class="mt-2 text-xs text-error">
              {{ currentCodexRateLimitError }}
            </div>

            <div v-else-if="currentCodexRateLimitSnapshots.length" class="mt-2 grid gap-2">
              <div v-if="currentCodexRateLimitPlanType" class="text-xs opacity-70">
                {{ t("config.api.codexPlan", { plan: formatCodexPlanType(currentCodexRateLimitPlanType) }) }}
              </div>
              <div class="text-xs opacity-70">
                {{ t("config.api.codexSnapshotCount", { count: currentCodexRateLimitSnapshots.length }) }}
              </div>
              <div class="text-xs opacity-70 break-all">
                {{ t("config.api.codexEndpoint", { url: currentCodexRateLimitQuery?.usageUrl || "-" }) }}
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
                    <div v-if="snapshot.primary.resetsAt" class="mt-1 text-xs opacity-70">
                      {{ t("config.api.codexResetAt", { time: formatCodexResetAt(snapshot.primary.resetsAt) }) }}
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
                    <div v-if="snapshot.secondary.resetsAt" class="mt-1 text-xs opacity-70">
                      {{ t("config.api.codexResetAt", { time: formatCodexResetAt(snapshot.secondary.resetsAt) }) }}
                    </div>
                  </div>
                </div>

                <div
                  v-else
                  class="rounded-box border border-dashed border-base-300 bg-base-100/60 px-3 py-2 text-xs opacity-70"
                >
                  {{ t("config.api.codexNoWindowData") }}
                </div>
              </div>

              <div v-if="currentCodexRateLimitCredits" class="text-xs opacity-70">
                Credits：{{ formatCodexCredits(currentCodexRateLimitCredits) }}
              </div>
              <div class="flex items-center justify-between gap-3 rounded-box border border-warning/40 bg-warning/10 p-3">
                <div class="text-xs">
                  <div class="font-medium">可用重置券：{{ currentCodexResetCreditCount }}</div>
                  <div class="mt-1 opacity-70">重置会消耗一张券，并立即重置符合条件的 Codex 限额窗口。</div>
                </div>
                <button
                  class="btn btn-sm btn-warning"
                  type="button"
                  :disabled="currentCodexResetCreditCount <= 0 || codexResetBusy"
                  @click="consumeCodexResetCredit"
                >
                  <span v-if="codexResetBusy" class="loading loading-spinner loading-xs"></span>
                  重置额度
                </button>
              </div>
            </div>

            <div v-else class="mt-2 text-xs opacity-70">
              {{ codexRateLimitPlaceholder }}
            </div>
          </div>
        </div>
      </div>
    </section>

    <ConfigCard :title="t('config.api.codexModels')">
      <template #actions>
        <select
          class="select select-bordered select-sm w-36 shrink-0"
          :disabled="!builtinCodexModelsLoaded || allCodexModelsEnabled"
          :value="''"
          @change="onAddModelSelect"
        >
          <option value="" disabled>{{ t("config.api.addModel") }}</option>
          <option v-for="name in candidateModels" :key="name" :value="name">{{ name }}</option>
        </select>
      </template>

      <div class="divide-y divide-base-200/60">
        <div v-for="group in props.draftGroups" :key="group.primary.id" class="py-3">
          <div class="flex items-start justify-between gap-2">
            <label class="min-w-0 flex-1">
              <span class="text-sm font-medium">{{ t("config.api.model") }}</span>
              <input
                v-model="group.primary.model"
                type="text"
                class="input input-bordered input-sm mt-1 w-full"
                :placeholder="t('config.api.unnamedModel')"
              />
            </label>
            <button class="btn btn-sm btn-square btn-ghost" type="button" :class="props.draftGroups.length <= 1 ? 'text-base-content/30' : 'text-error'" :disabled="props.draftGroups.length <= 1" @click="removeModelCard(group.primary.id)">
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>
    </ConfigCard>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, EyeOff, Trash2 } from "@lucide/vue";
import ConfigCard from "../../components/ConfigCard.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import type {
  ApiModelConfigItem,
  ApiProviderConfigItem,
  CodexAuthMode,
  CodexAuthStatus,
  CodexCreditsSnapshot,
  CodexConsumeRateLimitResetCreditResult,
  CodexRateLimitQueryResult,
  CodexRateLimitSnapshot,
  CodexRateLimitWindow,
} from "../../../../types/app";
import type { DraftModelGroup } from "../../utils/draft-model-groups";
import { modelGroupKey } from "../../utils/draft-model-groups";
import { CODEX_REASONING_EFFORTS } from "../../utils/api-config-display";
import { invokeTauri } from "../../../../services/tauri-api";
import { formatIsoToLocalDateTime } from "../../../../utils/time";

const DEFAULT_CODEX_BASE_URL = "https://chatgpt.com/backend-api/codex";
const DEFAULT_CODEX_AUTH_MODE: CodexAuthMode = "read_local";
const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
const DEFAULT_CODEX_ORIGINATOR = "codex-tui";
const DEFAULT_REASONING_EFFORT = "medium";
const DEFAULT_CODEX_CONTEXT_WINDOW_TOKENS = 262144;
const CODEX_SPARK_CONTEXT_WINDOW_TOKENS = 131072;

function codexContextWindowTokens(modelName: string): number {
  return modelName.trim().toLowerCase().includes("gpt-5.3-codex-spark")
    ? CODEX_SPARK_CONTEXT_WINDOW_TOKENS
    : DEFAULT_CODEX_CONTEXT_WINDOW_TOKENS;
}

const props = defineProps<{
  provider: ApiProviderConfigItem;
  draftGroups: DraftModelGroup[];
}>();

const { t } = useI18n();
const emit = defineEmits<{
  (e: "selectModel", modelId: string): void;
}>();

const codexAuthBusy = ref(false);
const builtinCodexModels = ref<string[]>([]);
const builtinCodexModelsLoaded = ref(false);
const showCodexCustomApiKey = ref(false);
const codexAuthStatusByProvider = ref<Record<string, CodexAuthStatus>>({});
const codexAuthPollTimer = ref<number | null>(null);
const codexRateLimitQueryByProvider = ref<Record<string, CodexRateLimitQueryResult | null>>({});
const codexRateLimitBusyByProvider = ref<Record<string, boolean>>({});
const codexRateLimitErrorByProvider = ref<Record<string, string>>({});
const codexResetBusy = ref(false);
const codexAuthModeOptions: Array<{ value: CodexAuthMode; label: string }> = [
  { value: "read_local", label: t("config.api.codexAuthModeReadLocal") },
  { value: "managed_oauth", label: t("config.api.codexAuthModeManagedOauth") },
  { value: "custom_url", label: t("config.api.codexAuthModeCustomUrl") },
];
const codexTemplateValues = computed<Record<string, unknown>>({
  get: () => ({
    authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
    localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
    customUrl: props.provider.codexCustomUrl || DEFAULT_CODEX_BASE_URL,
  }),
  set: (values) => {
    if (typeof values.authMode === "string" && values.authMode !== props.provider.codexAuthMode) {
      props.provider.codexAuthMode = values.authMode as CodexAuthMode;
      void refreshCodexAuthStatus();
    }
    if (typeof values.localAuthPath === "string") props.provider.codexLocalAuthPath = values.localAuthPath;
    if (typeof values.customUrl === "string") {
      props.provider.codexCustomUrl = values.customUrl;
      if (props.provider.codexAuthMode === "custom_url") props.provider.baseUrl = values.customUrl;
    }
  },
});
const codexTemplateGroups = computed<ConfigTemplateGroup[]>(() => {
  const mode = props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE;
  const fields: ConfigTemplateGroup["rows"] = [{
    items: [{
      key: "authMode",
      label: t("config.api.codexAuthMode"),
      type: "select",
      options: codexAuthModeOptions.map((item) => ({ value: item.value, label: item.label })),
    }],
  }];
  if (mode === "read_local") {
    fields.push({
      items: [{
        key: "localAuthPath",
        label: t("config.api.codexLocalAuthPath"),
        type: "text",
        placeholder: DEFAULT_CODEX_LOCAL_AUTH_PATH,
      }],
    });
  } else if (mode === "custom_url") {
    fields.push({
      items: [{
        key: "customUrl",
        label: t("config.api.codexCustomUrl"),
        description: t("config.api.codexCustomUrlHint"),
        type: "text",
        placeholder: DEFAULT_CODEX_BASE_URL,
      }],
    });
  }
  return [{ title: t("config.api.codexLogin"), rows: fields }];
});

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
const currentCodexResetCreditCount = computed(() => {
  return Math.max(0, Number(currentCodexRateLimitQuery.value?.rateLimitResetCreditCount || 0));
});
const currentCodexRateLimitError = computed(() => codexRateLimitErrorByProvider.value[props.provider.id] ?? "");
const currentCodexRateLimitBusy = computed(() => Boolean(codexRateLimitBusyByProvider.value[props.provider.id]));
const showCodexRateLimits = computed(() => props.provider.codexAuthMode !== "custom_url");
const codexRateLimitPlaceholder = computed(() => {
  if (!showCodexRateLimits.value) {
    return "自定义 URL 模式不提供官方 Codex 用量查询。";
  }
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

function applyCodexDefaults() {
  const normalizedMode = String(props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE).trim();
  if (normalizedMode === "managed_oauth") {
    props.provider.codexAuthMode = "managed_oauth";
    props.provider.baseUrl = DEFAULT_CODEX_BASE_URL;
    props.provider.apiKeys = [];
  } else if (normalizedMode === "custom_url") {
    props.provider.codexAuthMode = "custom_url";
    props.provider.baseUrl = props.provider.codexCustomUrl || DEFAULT_CODEX_BASE_URL;
    props.provider.apiKeys = props.provider.codexCustomApiKey ? [props.provider.codexCustomApiKey] : [];
  } else {
    props.provider.codexAuthMode = "read_local";
    props.provider.baseUrl = DEFAULT_CODEX_BASE_URL;
    props.provider.apiKeys = [];
  }
  props.provider.codexLocalAuthPath = String(props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH;
  props.provider.codexOriginator = String(props.provider.codexOriginator || DEFAULT_CODEX_ORIGINATOR).trim() || DEFAULT_CODEX_ORIGINATOR;
  props.provider.codexResidencyRequirement = props.provider.codexResidencyRequirement || "";
  props.provider.codexCustomUrl = props.provider.codexCustomUrl || "";
  props.provider.codexCustomApiKey = props.provider.codexCustomApiKey || "";
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
  if (!showCodexRateLimits.value) return false;
  return Boolean(status?.authenticated || status?.status === "expired");
}

async function refreshCodexRateLimits(status?: CodexAuthStatus | null) {
  const providerId = String(props.provider.id || "").trim();
  if (!providerId) return null;
  if (!showCodexRateLimits.value) {
    clearCodexRateLimits(providerId);
    return null;
  }
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
        baseUrl: props.provider.codexCustomUrl || props.provider.baseUrl || DEFAULT_CODEX_BASE_URL,
        customApiKey: props.provider.codexCustomApiKey || "",
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

async function consumeCodexResetCredit() {
  const providerId = String(props.provider.id || "").trim();
  if (!providerId || currentCodexResetCreditCount.value <= 0 || codexResetBusy.value) return;
  if (!window.confirm(`确定消耗 1 张重置券吗？当前有 ${currentCodexResetCreditCount.value} 张可用券。此操作不可撤销。`)) return;
  codexResetBusy.value = true;
  try {
    const result = await invokeTauri<CodexConsumeRateLimitResetCreditResult>("codex_consume_rate_limit_reset_credit", {
      input: {
        providerId,
        authMode: props.provider.codexAuthMode || DEFAULT_CODEX_AUTH_MODE,
        localAuthPath: props.provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        baseUrl: props.provider.codexCustomUrl || props.provider.baseUrl || DEFAULT_CODEX_BASE_URL,
      },
    });
    if (result.outcome !== "reset" && result.outcome !== "alreadyRedeemed") {
      throw new Error(`额度未重置：${result.outcome || "unknown"}`);
    }
    await refreshCodexRateLimits(currentCodexAuthStatus.value);
  } catch (error) {
    storeCodexRateLimitError(providerId, error);
  } finally {
    codexResetBusy.value = false;
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
        customApiKey: props.provider.codexCustomApiKey || "",
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
    enableImage: true,
    enableTools: true,
    reasoningEffort: DEFAULT_REASONING_EFFORT,
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: codexContextWindowTokens(modelName),
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
  };
}

const candidateModels = computed(() =>
  builtinCodexModels.value.filter(
    (name) => !props.draftGroups.some((group) => String(group.primary.model || "").trim() === name),
  ),
);
const allCodexModelsEnabled = computed(() => candidateModels.value.length === 0);

async function loadBuiltinCodexModels() {
  try {
    builtinCodexModels.value = await invokeTauri<string[]>("codex_get_builtin_models");
  } catch (error) {
    console.warn("[codex] 获取内置模型清单失败", error);
  } finally {
    builtinCodexModelsLoaded.value = true;
  }
}

function onAddModelSelect(event: Event) {
  const target = event.target as HTMLSelectElement;
  const name = String(target.value || "").trim();
  target.value = "";
  if (!name) return;
  addModelCard(name);
}

function addModelCard(modelName: string) {
  const seed = Date.now().toString();
  const model = createModel(seed, modelName);
  const variantIdByEffort = new Map<string, string>();
  const primaryEffort = String(model.reasoningEffort || "default").trim().toLowerCase() || "default";
  for (const effort of CODEX_REASONING_EFFORTS) {
    // 主等级复用卡 id（保持选中有效），其余等级用带等级后缀的唯一 id，避免同毫秒生成重复 id 互相覆盖
    variantIdByEffort.set(effort, effort === primaryEffort ? model.id : `api-model-${seed}-${effort}`);
  }
  props.draftGroups.unshift({
    key: modelGroupKey(model),
    primary: model,
    reasoningEfforts: [...CODEX_REASONING_EFFORTS],
    variantIdByEffort,
  });
  emit("selectModel", model.id);
}

function removeModelCard(modelId: string) {
  if (props.draftGroups.length <= 1) return;
  const idx = props.draftGroups.findIndex((group) => group.primary.id === modelId);
  if (idx < 0) return;
  props.draftGroups.splice(idx, 1);
  const fallback = props.draftGroups[Math.max(0, idx - 1)] ?? props.draftGroups[0];
  if (fallback) {
    emit("selectModel", fallback.primary.id);
  }
}

// 上下文窗口按模型名固定：模型名变化后重算
watch(
  () => props.draftGroups.map((group) => String(group.primary.model || "")),
  () => {
    for (const group of props.draftGroups) {
      group.primary.contextWindowTokens = codexContextWindowTokens(group.primary.model);
    }
  },
);

watch(
  () => props.provider.id,
  () => {
    void refreshCodexAuthStatus();
  },
  { immediate: true },
);

onMounted(() => {
  void loadBuiltinCodexModels();
});

onUnmounted(() => {
  stopCodexAuthPolling();
});
</script>
