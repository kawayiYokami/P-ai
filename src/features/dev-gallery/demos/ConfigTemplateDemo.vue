<template>
  <div class="grid gap-3">
    <ConfigTemplate v-model="configTemplateDemo" :groups="configTemplateGroups" />

    <div class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">{{ t("config.demo.nativeNotificationTitle") }}</h3>
          <p class="text-sm text-base-content/70">
            {{ t("config.demo.nativeNotificationSummary") }}
          </p>
          <p class="text-xs text-base-content/60">
            {{ t("config.demo.nativeNotificationDevHint") }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-primary"
            :disabled="sending"
            @click="sendNativeNotification"
          >
            {{ sending ? t("config.demo.sending") : t("config.demo.sendNativeNotification") }}
          </button>
          <span class="text-xs text-base-content/60">{{ t("config.demo.backgroundHint") }}</span>
        </div>

        <div v-if="errorText" class="alert alert-error text-sm">
          <span>{{ errorText }}</span>
        </div>

        <div v-else-if="resultText" class="alert alert-success text-sm whitespace-pre-wrap">
          <span>{{ resultText }}</span>
        </div>
      </div>
    </div>

    <div class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">{{ t("config.demo.restartTitle") }}</h3>
          <p class="text-sm text-base-content/70">
            {{ t("config.demo.restartSummary") }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-warning"
            :disabled="restarting"
            @click="restartApp"
          >
            <AppIcon name="rotate-ccw" class="size-4" aria-hidden="true" />
            {{ restarting ? t("config.demo.restarting") : t("config.demo.restartApp") }}
          </button>
        </div>
      </div>
    </div>

    <div class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">后端内存快照</h3>
          <p class="text-sm text-base-content/70">
            调用后端调试命令，查看会话缓存、message_store 缓存和其他长生命周期状态的占用概况。
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-secondary"
            :disabled="loadingMemoryStats"
            @click="loadMemoryStats"
          >
            {{ loadingMemoryStats ? "查询中..." : "查询后端内存" }}
          </button>
        </div>

        <div v-if="memoryStatsText" class="mockup-code max-h-96 overflow-auto text-xs">
          <pre class="whitespace-pre-wrap break-all"><code>{{ memoryStatsText }}</code></pre>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import ConfigTemplate from "../../config/components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../config/components/config-template";
import {
  invokeTauri,
  restartTransportApplicationDemo,
  sendTransportNativeNotificationDemo,
} from "../../../services/tauri-api";

type NativeNotificationDemoResult = {
  permissionBefore: string;
  permissionAfter: string;
  title: string;
  body: string;
  sentAt: string;
};

const { t } = useI18n();

const sending = ref(false);
const restarting = ref(false);
const loadingMemoryStats = ref(false);
const errorText = ref("");
const resultText = ref("");
const memoryStatsText = ref("");
const configTemplateDemo = ref<Record<string, unknown>>({
  openLocalDocument: true,
  openOnlineDocument: true,
  autoHideAtScreenEdge: true,
  showTaskbarIcon: true,
  closeApp: "tray",
  openWebInSystemBrowser: true,
  homepage: "https://pai.example.com",
  browserNote: "",
});

const configTemplateGroups: ConfigTemplateGroup[] = [
  {
    title: t("config.demo.generalSection"),
    rows: [
      {
        items: [
          {
            key: "openLocalDocument",
            label: t("config.demo.openLocalDocument"),
            description: t("config.demo.openLocalDocumentHint"),
            type: "toggle",
          },
        ],
      },
      {
        items: [
          {
            key: "openOnlineDocument",
            label: t("config.demo.openOnlineDocument"),
            type: "toggle",
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.mainPanelSection"),
    rows: [
      {
        items: [
          { key: "autoHideAtScreenEdge", label: t("config.demo.autoHideAtScreenEdge"), type: "toggle" },
          { key: "showTaskbarIcon", label: t("config.demo.showTaskbarIcon"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "closeApp",
            label: t("config.demo.closeApp"),
            type: "select",
            options: [
              { value: "tray", label: t("config.demo.closeAppToTray") },
              { value: "quit", label: t("config.demo.closeAppQuit") },
            ],
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.browseSection"),
    rows: [
      {
        items: [
          { key: "openWebInSystemBrowser", label: t("config.demo.openWebInSystemBrowser"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "homepage",
            label: t("config.demo.homepage"),
            type: "text",
            placeholder: t("config.demo.homepagePlaceholder"),
          },
          {
            key: "browserNote",
            label: t("config.demo.browserNote"),
            type: "textarea",
            placeholder: t("config.demo.browserNotePlaceholder"),
          },
        ],
      },
    ],
  },
];

async function sendNativeNotification() {
  sending.value = true;
  errorText.value = "";
  resultText.value = "";

  try {
    const result = await sendTransportNativeNotificationDemo<NativeNotificationDemoResult>();
    resultText.value = [
      t("config.demo.nativeNotificationSent"),
      `title: ${result.title}`,
      `permissionBefore: ${result.permissionBefore}`,
      `permissionAfter: ${result.permissionAfter}`,
      `sentAt: ${result.sentAt}`,
    ].join("\n");
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
  } finally {
    sending.value = false;
  }
}

async function restartApp() {
  restarting.value = true;
  errorText.value = "";
  resultText.value = "";

  try {
    await restartTransportApplicationDemo();
    resultText.value = t("config.demo.restartRequested");
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
    restarting.value = false;
  }
}

async function loadMemoryStats() {
  loadingMemoryStats.value = true;
  errorText.value = "";
  memoryStatsText.value = "";

  try {
    const result = await invokeTauri<unknown>("dump_memory_cache_stats");
    memoryStatsText.value = JSON.stringify(result, undefined, 2);
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
  } finally {
    loadingMemoryStats.value = false;
  }
}
</script>
