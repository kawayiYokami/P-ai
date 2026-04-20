<template>
  <div class="contents">
    <button
      class="btn btn-sm btn-ghost"
      :disabled="loading"
      :title="t('config.memory.importCardTitle')"
      @click="openDialog"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="7 10 12 15 17 10"/>
        <line x1="12" x2="12" y1="15" y2="3"/>
      </svg>
      <span>{{ t("config.memory.importShort") }}</span>
    </button>

    <input
      ref="importInputRef"
      type="file"
      accept=".json,application/json"
      class="hidden"
      @change="handleImportFile"
    />

    <dialog ref="dialogRef" class="modal">
      <div class="modal-box max-w-4xl">
        <h3 class="text-lg font-semibold">{{ t("config.memory.importDialogTitle") }}</h3>
        <div class="mt-2 text-sm opacity-70">
          {{ t("config.memory.importDialogHint") }}
        </div>

        <div v-if="message" class="mt-4 text-sm break-all rounded-box bg-base-200/50 px-2 py-1.5">
          {{ message }}
        </div>

        <div class="mt-4 rounded-box border border-base-300 bg-base-200/40 p-4 space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <button class="btn btn-sm btn-primary" :disabled="loading" @click="triggerFilePicker">
              {{ t("config.memory.selectImportFile") }}
            </button>
            <div v-if="fileName" class="text-sm opacity-70 break-all">{{ fileName }}</div>
          </div>

          <div v-if="preview" class="space-y-3">
            <div class="text-sm font-medium">
              {{ t("config.memory.importDetected", { count: preview.totalCount }) }}
            </div>

            <div class="space-y-3">
              <div class="text-sm font-medium">{{ t("config.memory.importScopeMappingsTitle") }}</div>
              <div
                v-for="scopeItem in preview.scopes"
                :key="scopeItem.scope"
                class="grid grid-cols-1 gap-2 rounded-box border border-base-300 bg-base-100 p-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] md:items-center"
              >
                <div class="min-w-0">
                  <div class="text-sm font-semibold break-all">{{ scopeItem.scope }}</div>
                  <div class="text-xs opacity-60">
                    {{ t("config.memory.importScopeCount", { count: scopeItem.count }) }}
                  </div>
                </div>
                <select v-model="scopeTargetMap[scopeItem.scope]" class="select select-bordered w-full">
                  <option value="">{{ t("config.memory.importSelectPersona") }}</option>
                  <option v-for="persona in importablePersonas" :key="persona.id" :value="persona.id">
                    {{ persona.name }}
                  </option>
                </select>
              </div>
              <div class="text-xs opacity-60">{{ t("config.memory.importScopeMappingsHint") }}</div>
            </div>

            <div>
              <div class="mb-3 text-sm font-medium">{{ t("config.memory.importSampleTitle") }}</div>
              <div v-if="!preview.samples.length" class="rounded-box border border-base-300 bg-base-100 p-4 text-sm opacity-60">
                {{ t("config.memory.importNoSamples") }}
              </div>
              <div v-else class="max-h-[50vh] space-y-2 overflow-auto pr-1">
                <div
                  v-for="sample in preview.samples"
                  :key="sample.id"
                  class="rounded-box border border-base-300 bg-base-100 p-3"
                >
                  <div class="whitespace-pre-wrap break-words text-sm font-semibold leading-relaxed">{{ sample.judgment }}</div>
                  <div
                    v-if="sample.reasoning"
                    class="mt-2 border-l-2 border-base-300 pl-2 text-sm italic opacity-70 whitespace-pre-wrap break-words"
                  >
                    {{ sample.reasoning }}
                  </div>
                  <div class="mt-3 flex flex-wrap items-center gap-2 text-sm">
                    <span class="badge badge-sm badge-outline">{{ sample.memoryScope }}</span>
                    <span class="badge badge-sm" :class="memoryTypeBadgeClass(sample.memoryType)">
                      {{ memoryTypeLabel(sample.memoryType) }}
                    </span>
                    <span class="opacity-50">{{ formatMemoryTime(sample.updatedAt || sample.createdAt) }}</span>
                    <span
                      v-for="(kw, idx) in sample.tags"
                      :key="`${sample.id}-${idx}`"
                      class="badge badge-sm badge-neutral opacity-80"
                    >
                      {{ kw }}
                    </span>
                    <span v-if="!sample.tags.length" class="opacity-40 text-[11px]">
                      {{ t("config.memory.noTags") }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="modal-action">
          <button class="btn" :disabled="loading" @click="closeDialog">
            {{ t("config.memory.importCancel") }}
          </button>
          <button class="btn btn-primary" :disabled="loading || !canConfirm" @click="confirmImport">
            {{ t("config.memory.importConfirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeDialog">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";

type PersonaLite = {
  id: string;
  name: string;
  isBuiltInUser?: boolean;
  isBuiltInSystem?: boolean;
};

type ImportPreviewScopeItem = {
  scope: string;
  count: number;
};

type ImportPreviewSample = {
  id: string;
  memoryScope: string;
  memoryType: "knowledge" | "skill" | "emotion" | "event";
  judgment: string;
  reasoning: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
};

type ImportPreviewResult = {
  totalCount: number;
  scopes: ImportPreviewScopeItem[];
  samples: ImportPreviewSample[];
};

type ImportResult = {
  importedCount: number;
  createdCount: number;
  mergedCount: number;
  totalCount: number;
};

const { t } = useI18n();
const emit = defineEmits<{
  (e: "imported", payload: ImportResult): void;
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);
const importInputRef = ref<HTMLInputElement | null>(null);
const loading = ref(false);
const message = ref("");
const personaOptions = ref<PersonaLite[]>([]);
const fileText = ref("");
const fileName = ref("");
const preview = ref<ImportPreviewResult | null>(null);
const scopeTargetMap = ref<Record<string, string>>({});

const importablePersonas = computed(() =>
  personaOptions.value.filter((persona) => !persona.isBuiltInUser && !persona.isBuiltInSystem),
);

const canConfirm = computed(() => {
  if (!preview.value || preview.value.totalCount <= 0) return false;
  const scopes = preview.value.scopes || [];
  if (scopes.length === 0) return false;
  return scopes.every((item) => !!String(scopeTargetMap.value[item.scope] || "").trim());
});

function memoryTypeBadgeClass(type: string): string {
  const classes: Record<string, string> = {
    knowledge: "badge-primary",
    skill: "badge-secondary",
    emotion: "badge-accent",
    event: "badge-info",
  };
  return classes[type] || "badge-ghost";
}

function memoryTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    knowledge: t("config.memory.typeKnowledge"),
    skill: t("config.memory.typeSkill"),
    emotion: t("config.memory.typeEmotion"),
    event: t("config.memory.typeEvent"),
  };
  return labels[type] || type;
}

function formatMemoryTime(iso: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;

  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) return t("config.memory.justNow");
  if (minutes < 60) return t("config.memory.minutesAgo", { count: minutes });
  if (hours < 24) return t("config.memory.hoursAgo", { count: hours });
  if (days < 7) return t("config.memory.daysAgo", { count: days });

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  if (year === now.getFullYear()) {
    return `${month}-${day} ${hour}:${minute}`;
  }
  return `${year}-${month}-${day}`;
}

async function withLoading<T>(fn: () => Promise<T>): Promise<T | null> {
  loading.value = true;
  try {
    return await fn();
  } catch (err) {
    message.value = `${t("config.memory.operationFailed")}: ${String(err)}`;
    return null;
  } finally {
    loading.value = false;
  }
}

async function loadPersonas() {
  const agents = await withLoading(() => invokeTauri<PersonaLite[]>("load_agents"));
  if (!agents) return;
  personaOptions.value = agents;
}

function buildDefaultScopeTargets(result: ImportPreviewResult): Record<string, string> {
  const next: Record<string, string> = {};
  for (const item of result.scopes || []) {
    const scope = String(item.scope || "").trim();
    if (!scope) continue;
    const exact = importablePersonas.value.find((persona) => String(persona.name || "").trim() === scope);
    if (exact) {
      next[scope] = exact.id;
      continue;
    }
    const insensitive = importablePersonas.value.find(
      (persona) => String(persona.name || "").trim().toLowerCase() === scope.toLowerCase(),
    );
    next[scope] = insensitive?.id || "";
  }
  return next;
}

function resetDialogState() {
  message.value = "";
  fileText.value = "";
  fileName.value = "";
  preview.value = null;
  scopeTargetMap.value = {};
  if (importInputRef.value) {
    importInputRef.value.value = "";
  }
}

function openDialog() {
  resetDialogState();
  dialogRef.value?.showModal();
}

function closeDialog() {
  dialogRef.value?.close();
  resetDialogState();
}

function triggerFilePicker() {
  importInputRef.value?.click();
}

async function handleImportFile(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  await withLoading(async () => {
    const text = await file.text();
    const result = await invokeTauri<ImportPreviewResult>("preview_import_angel_memories", {
      input: { payload: text },
    });
    fileText.value = text;
    fileName.value = file.name;
    preview.value = result;
    scopeTargetMap.value = buildDefaultScopeTargets(result);
  });
}

async function confirmImport() {
  if (!fileText.value || !preview.value) return;
  const scopeAgentMappings = preview.value.scopes.map((item) => ({
    scope: item.scope,
    agentId: String(scopeTargetMap.value[item.scope] || "").trim(),
  }));
  const result = await withLoading(() =>
    invokeTauri<ImportResult>("import_angel_memories", {
      input: {
        payload: fileText.value,
        scopeAgentMappings,
      },
    }),
  );
  if (!result) return;
  closeDialog();
  emit("imported", result);
}

onMounted(() => {
  void loadPersonas();
});
</script>
