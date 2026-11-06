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

    <ConfigCard>
      <div class="grid gap-4 py-3">
        <div class="grid gap-2 text-xs text-base-content/50 sm:grid-cols-[10rem_minmax(0,1fr)] sm:gap-4">
          <span>{{ t("config.searchEngines.engineColumnLabel") }}</span>
          <span>{{ t("config.searchEngines.apiKeysLabel") }}</span>
        </div>

        <div
          v-for="engine in SEARCH_ENGINE_META"
          :key="engine.id"
          class="grid gap-2 sm:grid-cols-[10rem_minmax(0,1fr)] sm:items-start sm:gap-4"
        >
          <div class="flex min-w-0 items-center gap-2 sm:pt-1">
            <span class="text-sm">{{ engine.name }}</span>
            <span class="badge badge-ghost badge-xs shrink-0">
              {{ engine.keyless
                ? t("config.searchEngines.keylessBadge")
                : t("config.searchEngines.requiredKeyBadge") }}
            </span>
          </div>

          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <template v-for="(apiKey, index) in engineView(engine.id).apiKeys" :key="index">
              <span
                v-if="apiKey.trim()"
                class="badge badge-lg h-7 gap-1.5 rounded-full border-base-300 bg-base-200 pl-3 pr-1.5 font-mono text-xs text-base-content/80"
              >
                <span class="max-w-56 truncate">{{ apiKey }}</span>
                <button
                  class="btn btn-ghost btn-xs btn-circle h-5 min-h-0 w-5 shrink-0"
                  type="button"
                  :title="t('config.searchEngines.removeKey')"
                  :aria-label="t('config.searchEngines.removeKey')"
                  @click="removeEngineKey(engine.id, index)"
                >
                  <AppIcon name="close" class="h-3 w-3" />
                </button>
              </span>
            </template>

            <input
              :value="keyDrafts[engine.id] ?? ''"
              class="input input-bordered input-sm min-w-0 flex-1 basis-56"
              type="text"
              autocomplete="off"
              spellcheck="false"
              :placeholder="engine.keyless
                ? t('config.searchEngines.keyPlaceholderKeyless')
                : t('config.searchEngines.keyPlaceholderRequired')"
              @input="keyDrafts[engine.id] = ($event.target as HTMLInputElement).value"
              @keydown.enter="addEngineKey(engine.id, $event)"
            />
          </div>
        </div>
      </div>
    </ConfigCard>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from "vue";
import { useI18n } from "vue-i18n";
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

/** 只读视图：渲染期只查不写。缺的引擎就地补一个空条目给模板用，**不回写 config**。
 *  以前这里直接 push 进 props.config.searchEngines，于是「什么都没改」也会被
 *  整份配置比对判成脏，且每次重渲染都会再补一遍。 */
function engineView(id: string): SearchEngineConfigItem & { apiKeys: string[] } {
  const existing = (props.config.searchEngines ?? []).find((entry) => entry.id === id);
  if (!existing) return { id, apiKeys: [] };
  if (!Array.isArray(existing.apiKeys)) existing.apiKeys = [];
  return existing as SearchEngineConfigItem & { apiKeys: string[] };
}

/**
 * 写入入口：只在用户动作里调用（加 key、删 key）。
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

/** 每个引擎的待提交输入框内容；回车才变成药丸，加完清空。 */
const keyDrafts = reactive<Record<string, string>>({});

function addEngineKey(engineId: string, event: KeyboardEvent) {
  // 输入法候选里的回车不算确认：先让输入法把候选上屏。
  if (event.isComposing) return;
  // 让回车只在这里生效，别触发页面上别的事件。
  event.preventDefault();
  const draft = (keyDrafts[engineId] ?? "").trim();
  keyDrafts[engineId] = "";
  if (!draft) return;
  const entry = engineEntry(engineId);
  if (entry.apiKeys.includes(draft)) return;
  entry.apiKeys.push(draft);
}

function removeEngineKey(engineId: string, index: number) {
  const entry = engineEntry(engineId);
  entry.apiKeys?.splice(index, 1);
  if (fixedEngineId.value === engineId && noKeyLeft(entry)) setLockTarget("");
}

function noKeyLeft(entry: SearchEngineConfigItem): boolean {
  return !(entry.apiKeys ?? []).some((key) => String(key ?? "").trim().length > 0);
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
}

defineExpose({ dirty, restoreDraft });
</script>
