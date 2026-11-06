<template>
  <div class="grid gap-7">
    <ConfigTemplate :model-value="templateValues" :groups="modeGroups">
      <template #row-lock-target>
        <ConfigRowField
          :label="t('config.searchEngines.lockTargetLabel')"
          :hint="isFixed ? t('config.searchEngines.lockTargetFixedHint') : t('config.searchEngines.lockTargetAutoHint')"
        >
          <ConfigValueSelect
            class="w-56"
            size="sm"
            :model-value="fixedEngineId"
            :options="lockableEngineOptions"
            :placeholder="t('config.searchEngines.lockTargetAuto')"
            @update:model-value="setLockTarget"
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

      <div class="divide-y divide-base-200/60">
        <div class="py-3">
          <p class="text-xs leading-relaxed text-base-content/60">
            {{ t(engine.descriptionKey) }}
            <button class="link link-primary text-xs" type="button" @click="openRegisterPage(engine.registerUrl)">
              {{ t("config.searchEngines.registerLink") }}
            </button>
          </p>
        </div>

        <div class="grid gap-2 py-3">
          <ConfigRowField
            v-for="(apiKey, index) in engineEntry(engine.id).apiKeys"
            :key="index"
            :label="keyRowLabel(index)"
          >
            <input
              :value="apiKey"
              class="input input-bordered input-sm w-72 max-w-full"
              type="text"
              autocomplete="off"
              spellcheck="false"
              :placeholder="engine.keyless
                ? t('config.searchEngines.keyPlaceholderKeyless')
                : t('config.searchEngines.keyPlaceholderRequired')"
              @input="setEngineKey(engine.id, index, ($event.target as HTMLInputElement).value)"
            />
            <button
              class="btn btn-sm btn-ghost btn-square"
              type="button"
              :title="t('config.searchEngines.removeKey')"
              :aria-label="t('config.searchEngines.removeKey')"
              @click="removeEngineKey(engine.id, index)"
            >
              <AppIcon name="trash" class="h-3.5 w-3.5" />
            </button>
          </ConfigRowField>

          <p v-if="engineEntry(engine.id).apiKeys.length === 0" class="text-xs text-base-content/50">
            {{ engine.keyless
              ? t("config.searchEngines.keylessHint")
              : t("config.searchEngines.requiredKeyHint") }}
          </p>

          <div class="flex flex-wrap items-center gap-2 pt-1">
            <button class="btn btn-sm bg-base-100" type="button" @click="addEngineKey(engine.id)">
              <AppIcon name="plus" class="h-3.5 w-3.5" />
              <span>{{ t("config.searchEngines.addKey") }}</span>
            </button>
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
          </div>
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
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConfigCard from "../../components/ConfigCard.vue";
import ConfigRowField from "../../components/ConfigRowField.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import ConfigValueSelect from "../../components/ConfigValueSelect.vue";
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
    rows: [{ key: "lock-target", items: [] }],
  },
]);

const isFixed = computed(() => props.config.searchMode?.mode === "fixed");
const fixedEngineId = computed(() =>
  props.config.searchMode?.mode === "fixed" ? props.config.searchMode.engineId : "",
);
const lockableEngineOptions = computed(() =>
  lockableSearchEngines(props.config.searchEngines).map((engine) => ({
    value: engine.id,
    label: engine.name,
  })),
);

/** 空值就是「自动」；选中某个引擎等于锁定它。 */
function setLockTarget(engineId: string) {
  if (!engineId) {
    props.config.searchMode = { mode: "auto" };
    return;
  }
  const lockable = lockableEngineOptions.value.some((option) => option.value === engineId);
  if (!lockable) {
    // 目标刚被删空密钥：保持自动，别写一个会被后端回落的锁定值。
    props.config.searchMode = { mode: "auto" };
    return;
  }
  props.config.searchMode = { mode: "fixed", engineId };
}

/**
 * 取某个引擎的卡片数据并补齐 apiKeys。
 * 原地改（不换对象），否则编辑写不到 config 上；返回类型已保证 apiKeys 是数组。
 */
function engineEntry(id: string): SearchEngineConfigItem & { apiKeys: string[] } {
  if (!props.config.searchEngines) props.config.searchEngines = [];
  const list = props.config.searchEngines;
  const existing = list.find((entry) => entry.id === id);
  if (existing) {
    if (!Array.isArray(existing.apiKeys)) existing.apiKeys = [];
    return existing as SearchEngineConfigItem & { apiKeys: string[] };
  }
  const created: SearchEngineConfigItem & { apiKeys: string[] } = { id, apiKeys: [] };
  list.push(created);
  return created;
}

function keyRowLabel(index: number): string {
  return t("config.searchEngines.keyRowLabel", { index: index + 1 });
}

function addEngineKey(engineId: string) {
  engineEntry(engineId).apiKeys?.push("");
}

function setEngineKey(engineId: string, index: number, value: string) {
  const entry = engineEntry(engineId);
  if (!entry.apiKeys) entry.apiKeys = [];
  entry.apiKeys[index] = value;
  // 锁定目标的密钥被清空后回落自动：后端保存时也会做一次，这里让界面立刻一致。
  if (fixedEngineId.value === engineId && noKeyLeft(entry)) setLockTarget("");
}

function removeEngineKey(engineId: string, index: number) {
  const entry = engineEntry(engineId);
  entry.apiKeys?.splice(index, 1);
  if (fixedEngineId.value === engineId && noKeyLeft(entry)) setLockTarget("");
}

function noKeyLeft(entry: SearchEngineConfigItem): boolean {
  return !(entry.apiKeys ?? []).some((key) => String(key ?? "").trim().length > 0);
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
      input: { engineId, apiKeys: [...(entry.apiKeys ?? [])] },
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
  if (saved.engines) {
    props.config.searchEngines = saved.engines.map((entry) => ({
      ...entry,
      apiKeys: [...(entry.apiKeys ?? [])],
    }));
  }
  props.config.searchMode = saved.mode ? { ...saved.mode } : { mode: "auto" };
  for (const key of Object.keys(testStates)) delete testStates[key];
}

defineExpose({ dirty, restoreDraft });
</script>
