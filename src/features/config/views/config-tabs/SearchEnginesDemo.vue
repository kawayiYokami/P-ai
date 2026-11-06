<template>
  <div class="flex flex-col gap-2">
    <div class="flex flex-wrap items-center gap-2 text-xs" data-demo-chrome>
      <label class="flex items-center gap-1">
        <span class="opacity-70">宽度</span>
        <select v-model.number="demoWidth" class="select select-bordered select-xs">
          <option :value="640">640</option>
          <option :value="760">760</option>
          <option :value="900">900</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        <span class="opacity-70">数据档位</span>
        <select v-model="demoState" class="select select-bordered select-xs">
          <option value="auto">自动模式（未改动）</option>
          <option value="fixed">指定 DeepSeek</option>
          <option value="dirty">改过两项（脏草稿）</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        <span class="opacity-70">高度</span>
        <select v-model.number="demoHeight" class="select select-bordered select-xs">
          <option :value="720">720</option>
          <option :value="1200">1200</option>
          <option :value="1600">1600</option>
        </select>
      </label>
      <span class="opacity-60">脏态：{{ dirty ? "有改动" : "干净" }}</span>
    </div>

    <div
      class="overflow-hidden rounded-box border border-base-300"
      :style="{ width: `min(${demoWidth}px, 100%)`, height: `${demoHeight}px` }"
    >
      <SettingsPageShell
        v-model:tab="activeTab"
        :tabs="demoTabs"
        :breadcrumb="demoBreadcrumb"
        header-class="pb-0"
      >
        <template #actions>
          <div class="flex flex-wrap items-center gap-2">
            <button
              v-if="dirty"
              class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1.5 px-3"
              type="button"
              @click="restoreDraft"
            >
              <AppIcon name="rotate-ccw" class="h-4 w-4" />
              <span>{{ t("config.api.restoreProviderDraft") }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3.5"
              :class="dirty ? 'btn-primary' : 'bg-base-100'"
              type="button"
              :disabled="!dirty"
              @click="mockSave"
            >
              <AppIcon name="save" class="h-4 w-4" />
              <span>{{ t("common.save") }}</span>
            </button>
          </div>
        </template>

        <SearchEnginesPanel
          ref="panelRef"
          :config="config"
          :last-saved-config-json="savedJson"
        />
      </SettingsPageShell>
    </div>

    <p class="text-xs opacity-60" data-demo-chrome>
      渲染的是真实的 SettingsPageShell + SearchEnginesPanel：配置为 mock（五个引擎、多把密钥、含掩码），
      「测试连接」真的会打后端，这里不点；保存按钮只把当前值写成新的基准快照，不落库。
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import type { AppConfig, SearchEngineConfigItem, SearchMode } from "../../../../types/app";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbItem } from "../../components/SettingsBreadcrumb.vue";
import type { UnderlineTabItem } from "../../../shared/components/UnderlineTabs.vue";
import SearchEnginesPanel from "./SearchEnginesPanel.vue";

const { t } = useI18n();

const demoWidth = ref(760);
const demoHeight = ref(1200);
const demoState = ref<"auto" | "fixed" | "dirty">("auto");
const activeTab = ref("searchEngines");

const demoTabs: UnderlineTabItem[] = [
  { key: "text", label: t("config.api.capabilityText") },
  { key: "voice", label: t("config.api.capabilityVoice") },
  { key: "embedding", label: t("config.api.capabilityEmbedding") },
  { key: "rerank", label: t("config.api.capabilityRerank") },
  { key: "imageGeneration", label: t("config.tabs.imageGeneration") },
  { key: "searchEngines", label: t("config.tabs.searchEngines") },
];

const demoBreadcrumb: SettingsBreadcrumbItem[] = [{ label: t("config.tabs.api") }];

function baseEngines(): SearchEngineConfigItem[] {
  return [
    { id: "exa", apiKeys: [] },
    { id: "parallel", apiKeys: ["pl-demo-1", "pl-demo-2"] },
    { id: "firecrawl", apiKeys: [] },
    { id: "tavily", apiKeys: ["tvly***ey"] },
    { id: "deepseek", apiKeys: [] },
  ];
}

function buildConfig(state: "auto" | "fixed" | "dirty"): AppConfig {
  const engines = baseEngines();
  let mode: SearchMode = { mode: "auto" };
  if (state === "fixed") {
    const deepseek = engines.find((entry) => entry.id === "deepseek");
    if (deepseek) {
      deepseek.apiKeys = ["sk-demo-deepseek"];
    }
    mode = { mode: "fixed", engineId: "deepseek" };
  }
  if (state === "dirty") {
    const tavily = engines.find((entry) => entry.id === "tavily");
    if (tavily) tavily.apiKeys = [];
    const exa = engines.find((entry) => entry.id === "exa");
    if (exa) exa.apiKeys = ["sk-demo-exa", ""];
  }
  return { searchEngines: engines, searchMode: mode } as unknown as AppConfig;
}

const config = ref<AppConfig>(buildConfig("auto"));
const savedJson = ref(JSON.stringify({ searchEngines: buildConfig("auto").searchEngines, searchMode: { mode: "auto" } }));
const panelRef = ref<{ dirty: boolean; restoreDraft: () => void } | null>(null);
const dirty = computed(() => panelRef.value?.dirty === true);

watch(demoState, (state) => {
  const next = buildConfig(state);
  config.value = next;
  const baseline = state === "dirty" ? buildConfig("auto") : next;
  savedJson.value = JSON.stringify({
    searchEngines: baseline.searchEngines,
    searchMode: baseline.searchMode,
  });
});

function mockSave() {
  savedJson.value = JSON.stringify({
    searchEngines: config.value.searchEngines,
    searchMode: config.value.searchMode,
  });
}

function restoreDraft() {
  panelRef.value?.restoreDraft();
}
</script>
