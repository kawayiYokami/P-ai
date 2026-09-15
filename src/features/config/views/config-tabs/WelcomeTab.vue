<template>
  <div class="flex flex-col gap-4 pb-20 [&_div]:[transition:background-color_200ms,border-color_200ms,box-shadow_200ms,border-radius_200ms_ease-out]">
    <!-- 仪表盘：品牌 + 缺失项提示 + 开始对话，只占一行 -->
    <div class="card bg-base-100 card-border border-base-300 from-base-content/5 bg-linear-to-bl to-50% card-sm overflow-hidden">
      <div class="card-body flex-row flex-wrap items-center gap-2 px-4 py-2.5">
        <!-- 品牌区 -->
        <div class="flex items-center gap-2 pr-1">
          <img :src="appIconUrl" alt="P-ai" class="size-6 rounded" />
          <span class="text-sm font-bold">P-ai</span>
          <span class="text-xs opacity-60" v-if="appVersion">v{{ appVersion }}</span>
        </div>

        <!-- 未设置的模型分工（点击跳对话设置页） -->
      <button
        v-if="!quickModel"
        class="btn btn-xs btn-outline btn-warning gap-1"
        type="button"
        @click="emit('jump', 'chatSettings')"
      >
        <span>{{ t("config.welcome.cards.quickModel.title") }}</span>
        <span class="opacity-80">{{ t("config.welcome.notSet") }}</span>
      </button>
      <button
        v-if="!expertModel"
        class="btn btn-xs btn-outline btn-warning gap-1"
        type="button"
        @click="emit('jump', 'chatSettings')"
      >
        <span>{{ t("config.welcome.cards.expertModel.title") }}</span>
        <span class="opacity-80">{{ t("config.welcome.notSet") }}</span>
      </button>

      <div class="flex-1" />

      <button class="btn btn-sm btn-primary" type="button" @click="emit('start-chat')">
        <MessageSquare class="h-3.5 w-3.5" />
        {{ t("window.startChat") }}
      </button>
      </div>
    </div>

    <!-- 运行时依赖：ripgrep 独立设置项 -->
    <div
      v-if="showRuntimeDeps"
      class="card bg-base-100 card-border border-base-300 from-base-content/5 bg-linear-to-bl to-50% card-sm overflow-hidden"
    >
      <div class="card-body gap-3 px-4 py-3">
        <div class="flex items-center gap-2">
          <span class="text-sm font-bold">{{ t("config.welcome.runtimeDeps.title") }}</span>
        </div>
        <div class="flex flex-col gap-2">
          <div v-for="dep in runtimeDeps" :key="dep.kind" class="flex flex-wrap items-center gap-2">
            <div class="flex flex-col w-44 shrink-0">
              <span class="text-sm font-medium">{{ dep.label }}</span>
              <span class="text-xs opacity-60">{{ dep.hint }}</span>
            </div>
            <span class="badge badge-error gap-1 font-medium">
              {{ t("config.welcome.notInstalled") }}
            </span>
            <div class="flex-1" />
            <span v-if="runtimeInstallStatusError[dep.kind]" class="text-xs text-error max-w-56 text-right">
              {{ runtimeInstallStatus[dep.kind] }}
            </span>
            <button
              class="btn btn-xs btn-primary"
              type="button"
              :disabled="installingPrerequisite !== null"
              @click="installPrerequisite(dep.kind)"
            >
              {{ installingPrerequisite === dep.kind ? t("config.welcome.installing") : t("config.welcome.autoInstall") }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 足迹墙 -->
    <UsageTrailWall />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { MessageSquare } from "@lucide/vue";
import type { ApiConfigItem, AppConfig } from "../../../../types/app";
import UsageTrailWall from "./UsageTrailWall.vue";
import {
  canUseTransportHostRuntimeCheck,
  getTransportHostRuntimePrerequisites,
  installTransportHostRuntimePrerequisite,
  invokeTauri,
  openTransportExternalUrl,
} from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import appIconUrl from "../../../../../src-tauri/icons/128x128.png";

type ConfigTab = "welcome" | "hotkey" | "api" | "mcp" | "skill" | "persona" | "chatSettings" | "usage" | "memory" | "task" | "logs" | "appearance" | "migration" | "about";
type HostRuntimePrerequisiteKind = "git" | "node" | "rg";
type HostRuntimePrerequisites = {
  gitInstalled?: boolean;
  nodeInstalled?: boolean;
  rgInstalled?: boolean;
};
type HostRuntimePrerequisiteInstallResult = {
  kind: HostRuntimePrerequisiteKind;
  installed: boolean;
  message: string;
};
type MissingDep = {
  kind: HostRuntimePrerequisiteKind;
  label: string;
  hint: string;
};

const props = defineProps<{
  config: AppConfig;
}>();

const emit = defineEmits<{
  (e: "jump", value: ConfigTab): void;
  (e: "start-chat"): void;
}>();

const { t } = useI18n();
// 备用下载兜底：当前前端只有 rg 安装入口，git/node 保留供后续 UI 复用
const GIT_DOWNLOAD_URL = "https://git-scm.com/downloads";
const NODE_DOWNLOAD_URL = "https://nodejs.org/en/download";
const RG_DOWNLOAD_URL = "https://github.com/BurntSushi/ripgrep/releases";

const hostRuntimePrerequisites = ref<HostRuntimePrerequisites>({});
const installingPrerequisite = ref<HostRuntimePrerequisiteKind | null>(null);
const runtimeInstallStatus = ref<Record<string, string>>({});
const runtimeInstallStatusError = ref<Record<string, boolean>>({});
const appVersion = ref("");

function findModel(apiConfigs: ApiConfigItem[], apiConfigId: string | undefined | null) {
  const id = String(apiConfigId || "").trim();
  return id ? apiConfigs.find((api) => api.id === id && api.enableText) ?? null : null;
}

async function loadHostRuntimeState() {
  try {
    hostRuntimePrerequisites.value = await getTransportHostRuntimePrerequisites<HostRuntimePrerequisites>();
  } catch {
    hostRuntimePrerequisites.value = {};
  }
}

onMounted(() => {
  void loadHostRuntimeState();
  void loadAppVersion();
});

async function loadAppVersion() {
  try {
    appVersion.value = await invokeTauri<string>("get_app_version");
  } catch {
    appVersion.value = "";
  }
}

// 运行时依赖卡片：ripgrep 独立设置项，只在明确未安装时显示；
// 检测无结果（invoke 异常/未返回字段/Web 宿主无本机检测）或已安装时整卡隐藏。
const showRuntimeDeps = computed(() => {
  if (!canUseTransportHostRuntimeCheck()) return false;
  return hostRuntimePrerequisites.value.rgInstalled === false;
});
const runtimeDeps = computed<MissingDep[]>(() => {
  return [
    {
      kind: "rg",
      label: t("config.welcome.cards.ripgrep.title"),
      hint: t("config.welcome.cards.ripgrep.hint"),
    },
  ];
});

const quickModel = computed(() => findModel(props.config.apiConfigs || [], props.config.toolReviewApiConfigId));
const expertModel = computed(() => findModel(props.config.apiConfigs || [], props.config.expertApiConfigId));

async function installPrerequisite(kind: HostRuntimePrerequisiteKind) {
  if (installingPrerequisite.value) return;
  installingPrerequisite.value = kind;
  runtimeInstallStatus.value[kind] = t("config.welcome.installing");
  runtimeInstallStatusError.value[kind] = false;
  try {
    const result = await installTransportHostRuntimePrerequisite<HostRuntimePrerequisiteInstallResult>(kind);
    runtimeInstallStatus.value[kind] = result.message || t("config.welcome.installSuccess");
    runtimeInstallStatusError.value[kind] = false;
    await loadHostRuntimeState();
  } catch (error) {
    const err = toErrorMessage(error);
    runtimeInstallStatus.value[kind] = t("config.welcome.installFailedFallback", { err });
    runtimeInstallStatusError.value[kind] = true;
    // kind 当前只会是 rg；git/node 分支保留，与上方常量配套，供后续 UI 复用
    const fallbackUrl = kind === "git" ? GIT_DOWNLOAD_URL : kind === "node" ? NODE_DOWNLOAD_URL : RG_DOWNLOAD_URL;
    void openTransportExternalUrl(fallbackUrl);
  } finally {
    installingPrerequisite.value = null;
  }
}
</script>
