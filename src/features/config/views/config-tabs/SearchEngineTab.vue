<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-base-300 px-5 py-3">
      <div class="min-w-0">
        <div class="text-sm font-semibold">{{ t("config.searchEngines.title") }}</div>
        <div class="mt-0.5 text-xs text-base-content/60">{{ t("config.searchEngines.description") }}</div>
      </div>
      <button
        class="btn btn-sm btn-primary shrink-0"
        type="button"
        :disabled="!isDirty || props.savingConfig"
        @click="handleSaveConfig"
      >
        {{ props.savingConfig ? t("common.saving") : t("common.save") }}
      </button>
    </div>

    <OverlayScrollArea class="min-h-0 flex-1">
      <div class="flex flex-col gap-4 px-5 py-4">
        <!-- 调度模式 -->
        <section class="rounded-box border border-base-300 bg-base-100 p-4">
          <div class="flex flex-wrap items-center gap-3">
            <span class="text-sm font-medium">{{ t("config.searchEngines.modeLabel") }}</span>
            <div class="join">
              <button
                class="btn btn-xs join-item"
                :class="isAuto ? 'btn-primary' : 'bg-base-100'"
                type="button"
                @click="selectAutoMode"
              >
                {{ t("config.searchEngines.modeAuto") }}
              </button>
              <button
                class="btn btn-xs join-item"
                :class="!isAuto ? 'btn-primary' : 'bg-base-100'"
                type="button"
                :disabled="lockableEngines.length === 0"
                @click="selectFixedMode"
              >
                {{ t("config.searchEngines.modeFixed") }}
              </button>
            </div>
            <select
              v-if="!isAuto"
              class="select select-bordered select-xs w-56"
              :value="fixedEngineId"
              @change="onFixedEngineChange(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="engine in lockableEngines" :key="engine.id" :value="engine.id">
                {{ engine.name }}
              </option>
            </select>
          </div>
          <p class="mt-2 text-xs text-base-content/60">
            {{ isAuto ? t("config.searchEngines.modeAutoHint") : t("config.searchEngines.modeFixedHint") }}
          </p>
          <p v-if="lockableEngines.length === 0" class="mt-1 text-xs text-warning">
            {{ t("config.searchEngines.modeFixedUnavailable") }}
          </p>
        </section>

        <!-- 引擎卡片 -->
        <section
          v-for="engine in engineCards"
          :key="engine.id"
          class="rounded-box border border-base-300 bg-base-100 p-4"
        >
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <span class="text-sm font-semibold">{{ engine.name }}</span>
                <span v-if="engine.keyless" class="badge badge-ghost badge-xs">
                  {{ t("config.searchEngines.keylessBadge") }}
                </span>
                <span v-else class="badge badge-warning badge-xs">
                  {{ t("config.searchEngines.paidBadge") }}
                </span>
              </div>
              <p class="mt-1 max-w-2xl text-xs text-base-content/60">{{ t(engine.descriptionKey) }}</p>
              <button
                class="link link-primary mt-1 text-xs"
                type="button"
                @click="openRegisterPage(engine.registerUrl)"
              >
                {{ t("config.searchEngines.registerLink") }}
              </button>
            </div>
            <label class="flex shrink-0 items-center gap-2 text-xs">
              <span class="text-base-content/60">{{ t("config.searchEngines.enabledLabel") }}</span>
              <input
                :checked="engine.config.enabled"
                class="toggle toggle-sm toggle-primary"
                type="checkbox"
                @change="setEngineEnabled(engine.id, ($event.target as HTMLInputElement).checked)"
              />
            </label>
          </div>

          <div class="mt-3 flex flex-wrap items-center gap-2">
            <input
              :value="engine.config.apiKey ?? ''"
              class="input input-bordered input-sm w-80 font-mono text-xs"
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
              :disabled="engine.testing"
              @click="testEngine(engine.id)"
            >
              <span v-if="engine.testing" class="loading loading-spinner loading-xs"></span>
              <span>{{ engine.testing ? t("config.searchEngines.testing") : t("config.searchEngines.test") }}</span>
            </button>
            <span v-if="engine.testResult" class="text-xs" :class="engine.testOk ? 'text-success' : 'text-error'">
              {{ engine.testResult }}
            </span>
          </div>
          <p class="mt-1 text-xs text-base-content/50">
            {{ engine.keyless
              ? t("config.searchEngines.keylessHint")
              : t("config.searchEngines.requiredKeyHint") }}
          </p>
        </section>
      </div>
    </OverlayScrollArea>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri, openTransportExternalUrl } from "../../../../services/tauri-api";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import type { AppConfig, SearchEngineConfigItem } from "../../../../types/app";
import {
  SEARCH_ENGINE_META,
  isSearchConfigDirty,
  lockableSearchEngines,
  type SearchEngineMeta,
} from "./search-engine-model";

const props = defineProps<{
  config: AppConfig;
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  lastSavedConfigJson: string;
}>();

const { t } = useI18n();

type EngineCard = SearchEngineMeta & {
  config: SearchEngineConfigItem;
  testing: boolean;
  testResult: string;
  testOk: boolean;
};

const testStates = reactive<Record<string, { testing: boolean; ok: boolean; message: string }>>({});

function engineEntry(id: string): SearchEngineConfigItem {
  if (!props.config.searchEngines) props.config.searchEngines = [];
  const list = props.config.searchEngines;
  const existing = list.find((entry) => entry.id === id);
  if (existing) return existing;
  const created: SearchEngineConfigItem = { id, enabled: false };
  list.push(created);
  return created;
}

const engineCards = computed<EngineCard[]>(() =>
  SEARCH_ENGINE_META.map((meta) => ({
    ...meta,
    config: engineEntry(meta.id),
    ...(testStates[meta.id]
      ? {
          testing: testStates[meta.id].testing,
          testResult: testStates[meta.id].message,
          testOk: testStates[meta.id].ok,
        }
      : { testing: false, testResult: "", testOk: false }),
  })),
);

const isAuto = computed(() => props.config.searchMode?.mode !== "fixed");
const fixedEngineId = computed(() =>
  props.config.searchMode?.mode === "fixed" ? props.config.searchMode.engineId : "",
);

/** 只有填了密钥的引擎能锁定；免密钥引擎不进锁定候选，避免把公共免费额度打光。 */
const lockableEngines = computed(() => lockableSearchEngines(props.config.searchEngines));

/** 上次保存下来的配置快照；解析失败按「全部缺省」处理，界面会表现为有改动。 */
const savedSearchConfig = computed(() => {
  try {
    const parsed = JSON.parse(String(props.lastSavedConfigJson || "{}")) as Partial<AppConfig>;
    return { engines: parsed.searchEngines, mode: parsed.searchMode };
  } catch {
    return { engines: undefined, mode: undefined };
  }
});

const isDirty = computed(() =>
  isSearchConfigDirty(
    { engines: props.config.searchEngines, mode: props.config.searchMode },
    savedSearchConfig.value,
  ),
);

function selectAutoMode() {
  props.config.searchMode = { mode: "auto" };
}

function selectFixedMode() {
  const first = lockableEngines.value[0];
  if (!first) return;
  props.config.searchMode = {
    mode: "fixed",
    engineId: fixedEngineId.value || first.id,
  };
}

function onFixedEngineChange(engineId: string) {
  if (!engineId) return;
  props.config.searchMode = { mode: "fixed", engineId };
}

function setEngineEnabled(engineId: string, enabled: boolean) {
  engineEntry(engineId).enabled = enabled;
  // 锁定的引擎被停用后，模式回落自动（后端保存时也会做一次，这里让界面立刻一致）。
  if (!enabled && fixedEngineId.value === engineId) selectAutoMode();
}

function setEngineApiKey(engineId: string, apiKey: string) {
  const entry = engineEntry(engineId);
  entry.apiKey = apiKey;
  if (apiKey.trim() === "" && fixedEngineId.value === engineId) selectAutoMode();
}

async function openRegisterPage(url: string) {
  try {
    await openTransportExternalUrl(url);
  } catch {
    // 打不开浏览器时不影响配置，用户可自行复制链接
  }
}

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

async function handleSaveConfig() {
  if (!isDirty.value) return;
  await Promise.resolve(props.saveConfigAction());
}

function toErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error ?? "");
}
</script>
