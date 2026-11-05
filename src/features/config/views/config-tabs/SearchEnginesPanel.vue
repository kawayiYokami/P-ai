<template>
  <div class="grid gap-7">
    <ConfigTemplate :model-value="templateValues" :groups="modeGroups">
      <template #row-search-mode>
        <ConfigRowField
          :label="t('config.searchEngines.modeLabel')"
          :hint="isFixed ? t('config.searchEngines.modeFixedHint') : t('config.searchEngines.modeAutoHint')"
        >
          <SegmentedControl
            class="w-44"
            size="sm"
            :model-value="modeValue"
            :options="modeOptions"
            @update:model-value="setMode"
          />
        </ConfigRowField>
      </template>

      <template #row-fixed-engine>
        <ConfigRowField :label="t('config.searchEngines.fixedEngineLabel')">
          <ConfigValueSelect
            class="w-44"
            size="sm"
            :model-value="fixedEngineId"
            :options="lockableEngineOptions"
            :placeholder="t('config.searchEngines.fixedEnginePlaceholder')"
            @update:model-value="setFixedEngine"
          />
        </ConfigRowField>
      </template>
    </ConfigTemplate>

    <ConfigCard v-for="engine in SEARCH_ENGINE_META" :key="engine.id">
      <template #title>
        <div class="flex min-w-0 items-center gap-2">
          <h3 class="text-sm font-semibold">{{ engine.name }}</h3>
          <span class="badge badge-ghost badge-xs">
            {{ engine.keyless
              ? t("config.searchEngines.keylessBadge")
              : t("config.searchEngines.paidBadge") }}
          </span>
        </div>
      </template>

      <template #actions>
        <label class="flex shrink-0 items-center gap-2">
          <span class="text-xs text-base-content/60">{{ t("config.searchEngines.enabledLabel") }}</span>
          <input
            :checked="engineEntry(engine.id).enabled"
            class="toggle toggle-sm toggle-primary shrink-0"
            type="checkbox"
            @change="setEngineEnabled(engine.id, ($event.target as HTMLInputElement).checked)"
          />
        </label>
      </template>

      <div class="divide-y divide-base-200/60">
        <div class="py-3">
          <p class="text-xs leading-relaxed text-base-content/60">
            {{ t(engine.descriptionKey) }}
            <button class="link link-primary text-xs" type="button" @click="openRegisterPage(engine.registerUrl)">
              {{ t("config.searchEngines.registerLink") }}
            </button>
          </p>
        </div>

        <div class="py-3">
          <ConfigRowField
            :label="t('config.searchEngines.keyLabel')"
            :hint="engine.keyless
              ? t('config.searchEngines.keylessHint')
              : t('config.searchEngines.requiredKeyHint')"
          >
            <input
              :value="engineEntry(engine.id).apiKey ?? ''"
              class="input input-bordered input-sm w-64 max-w-full"
              type="text"
              autocomplete="off"
              spellcheck="false"
              :placeholder="engine.keyless
                ? t('config.searchEngines.keyPlaceholderKeyless')
                : t('config.searchEngines.keyPlaceholderRequired')"
              @input="setEngineApiKey(engine.id, ($event.target as HTMLInputElement).value)"
            />
            <button
              class="btn btn-sm bg-base-100"
              type="button"
              :disabled="testStates[engine.id]?.testing === true"
              @click="testEngine(engine.id)"
            >
              <span v-if="testStates[engine.id]?.testing" class="loading loading-spinner loading-xs"></span>
              <span>
                {{ testStates[engine.id]?.testing
                  ? t("config.searchEngines.testing")
                  : t("config.searchEngines.test") }}
              </span>
            </button>
            <span
              v-if="testStates[engine.id]?.message"
              class="text-xs"
              :class="testStates[engine.id]?.ok ? 'text-success' : 'text-error'"
            >
              {{ testStates[engine.id]?.message }}
            </span>
          </ConfigRowField>
        </div>
      </div>
    </ConfigCard>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri, openTransportExternalUrl } from "../../../../services/tauri-api";
import type { AppConfig, SearchEngineConfigItem } from "../../../../types/app";
import ConfigCard from "../../components/ConfigCard.vue";
import ConfigRowField from "../../components/ConfigRowField.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import ConfigValueSelect from "../../components/ConfigValueSelect.vue";
import SegmentedControl from "../../components/SegmentedControl.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import {
  SEARCH_ENGINE_META,
  createSearchBaselineResolver,
  isSearchConfigDirty,
  lockableSearchEngines,
} from "./search-engine-model";

const props = defineProps<{
  config: AppConfig;
  lastSavedConfigJson: string;
}>();

const { t } = useI18n();

const templateValues = {};
const modeGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "searchMode",
    title: t("config.searchEngines.dispatchTitle"),
    rows: [
      { key: "search-mode", items: [] },
      ...(isFixed.value ? [{ key: "fixed-engine", items: [] }] : []),
    ],
  },
]);

const isFixed = computed(() => props.config.searchMode?.mode === "fixed");
const modeValue = computed(() => (isFixed.value ? "fixed" : "auto"));
const fixedEngineId = computed(() =>
  props.config.searchMode?.mode === "fixed" ? props.config.searchMode.engineId : "",
);
const lockableEngineOptions = computed(() =>
  lockableSearchEngines(props.config.searchEngines).map((engine) => ({
    value: engine.id,
    label: engine.name,
  })),
);
const modeOptions = computed(() => [
  { value: "auto", label: t("config.searchEngines.modeAuto") },
  {
    value: "fixed",
    label: t("config.searchEngines.modeFixed"),
    disabled: lockableEngineOptions.value.length === 0,
  },
]);

function engineEntry(id: string): SearchEngineConfigItem {
  if (!props.config.searchEngines) props.config.searchEngines = [];
  const list = props.config.searchEngines;
  const existing = list.find((entry) => entry.id === id);
  if (existing) return existing;
  const created: SearchEngineConfigItem = { id, enabled: false };
  list.push(created);
  return created;
}

function setMode(value: string | number | boolean) {
  if (value !== "fixed") {
    props.config.searchMode = { mode: "auto" };
    return;
  }
  const first = lockableEngineOptions.value[0];
  if (!first) return;
  props.config.searchMode = { mode: "fixed", engineId: fixedEngineId.value || first.value };
}

function setFixedEngine(engineId: string) {
  if (!engineId) return;
  props.config.searchMode = { mode: "fixed", engineId };
}

function setEngineEnabled(engineId: string, enabled: boolean) {
  engineEntry(engineId).enabled = enabled;
  // 锁定的引擎被停用后回落自动；后端保存时也会做一次，这里让界面立刻一致。
  if (!enabled && fixedEngineId.value === engineId) setMode("auto");
}

function setEngineApiKey(engineId: string, apiKey: string) {
  engineEntry(engineId).apiKey = apiKey;
  if (apiKey.trim() === "" && fixedEngineId.value === engineId) setMode("auto");
}

async function openRegisterPage(url: string) {
  try {
    await openTransportExternalUrl(url);
  } catch {
    // 打不开浏览器不影响配置，用户可以自己复制链接
  }
}

// ==================== 连接测试（纯界面态，不进配置） ====================

type TestState = { testing: boolean; ok: boolean; message: string };
const testStates = reactive<Record<string, TestState>>({});

async function testEngine(engineId: string) {
  const entry = engineEntry(engineId);
  testStates[engineId] = { testing: true, ok: false, message: "" };
  try {
    const result = await invokeTauri<{ ok: boolean; resultCount?: number }>("test_search_engine", {
      input: { engineId, apiKey: entry.apiKey ?? "" },
    });
    const count = Number(result?.resultCount ?? 0);
    testStates[engineId] = {
      testing: false,
      ok: true,
      message: t("config.searchEngines.testSuccess", { count }),
    };
  } catch (error) {
    testStates[engineId] = { testing: false, ok: false, message: toErrorMessage(error) };
  }
}

function toErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error ?? "");
}

// ==================== 脏判定与草稿还原（对 header 的保存按钮负责） ====================

/**
 * 基准解析器：快照里没有 searchEngines 字段时（首次打开、老快照、坏 JSON）取面板第一次
 * 读到的值并冻结，原地编辑不会再改基准，所以「打开不脏 → 改了才脏」两头都成立。
 */
const resolveSearchBaseline = createSearchBaselineResolver();

const savedSearchConfig = computed(() =>
  resolveSearchBaseline(props.lastSavedConfigJson, {
    engines: props.config.searchEngines,
    mode: props.config.searchMode,
  }),
);

const dirty = computed(() =>
  isSearchConfigDirty(
    { engines: props.config.searchEngines, mode: props.config.searchMode },
    savedSearchConfig.value,
  ),
);

function restoreDraft() {
  const saved = savedSearchConfig.value;
  if (saved.engines) props.config.searchEngines = saved.engines.map((entry) => ({ ...entry }));
  props.config.searchMode = saved.mode ? { ...saved.mode } : { mode: "auto" };
  for (const key of Object.keys(testStates)) delete testStates[key];
}

defineExpose({ dirty, restoreDraft });
</script>
