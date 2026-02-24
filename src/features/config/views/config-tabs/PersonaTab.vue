<template>
  <label class="flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.persona.title") }}</span></div>
    <div class="flex gap-1">
      <select :value="personaEditorId" class="select select-bordered select-sm flex-1" @change="$emit('update:personaEditorId', ($event.target as HTMLSelectElement).value)">
        <option v-for="p in personas" :key="p.id" :value="p.id">{{ p.name }}{{ p.isBuiltInUser ? `（${t("config.persona.userTag")}）` : "" }}</option>
      </select>
      <button class="btn btn-sm btn-square text-primary bg-base-100" :title="t('config.persona.add')" @click="$emit('addPersona')">
        <Plus class="h-3.5 w-3.5" />
      </button>
      <button
        class="btn btn-sm btn-square text-error bg-base-100"
        :title="t('config.persona.remove')"
        :disabled="!selectedPersona || selectedPersona.isBuiltInUser || assistantPersonas.length <= 1"
        @click="$emit('removeSelectedPersona')"
      >
        <Trash2 class="h-3.5 w-3.5" />
      </button>
    </div>
  </label>
  <div class="divider my-0"></div>

  <div v-if="selectedPersona" class="grid gap-2">
    <label class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.persona.name") }}</span></div>
      <div class="flex items-center gap-2">
        <input v-model="selectedPersona.name" class="input input-bordered input-sm flex-1" :placeholder="t('config.persona.name')" />
        <button
          class="btn btn-ghost btn-circle p-0 min-h-0 h-auto w-auto"
          :disabled="avatarSaving"
          :title="avatarSaving ? t('config.persona.avatarSaving') : t('config.persona.editAvatar')"
          @click="$emit('openAvatarEditor')"
        >
          <div v-if="selectedPersonaAvatarUrl" class="avatar">
            <div class="w-10 rounded-full">
              <img :src="selectedPersonaAvatarUrl" :alt="selectedPersona.name" :title="selectedPersona.name" />
            </div>
          </div>
          <div v-else class="avatar placeholder">
            <div class="bg-neutral text-neutral-content w-10 rounded-full">
              <span>{{ avatarInitial(selectedPersona.name) }}</span>
            </div>
          </div>
        </button>
      </div>
      <div v-if="avatarError" class="flex items-center justify-between py-1"><span class="text-error break-all">{{ avatarError }}</span></div>
    </label>
    <label class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.persona.prompt") }}</span></div>
      <textarea
        v-model="selectedPersona.systemPrompt"
        class="textarea textarea-bordered textarea-xs w-full"
        rows="12"
        :placeholder="selectedPersona.isBuiltInUser ? t('config.persona.userPlaceholder') : t('config.persona.assistantPlaceholder')"
      ></textarea>
    </label>

    <div v-if="!selectedPersona.isBuiltInUser" class="text-xs font-medium">{{ t('config.persona.privateMemory') }}</div>
    <div v-if="!selectedPersona.isBuiltInUser" class="card bg-base-100 border border-base-300">
      <div class="card-body gap-3 p-3">
        <div class="flex items-center justify-between">
          <div class="text-xs">
            <div class="opacity-60">{{ t('config.persona.privateMemoryHint') }}</div>
            <div class="mt-1 font-medium">
              {{ t('config.persona.currentStatus') }}{{ selectedPersona.privateMemoryEnabled ? t('config.persona.private') : t('config.persona.public') }}
            </div>
          </div>
          <div class="flex gap-1">
            <button
              class="badge badge-sm cursor-pointer"
              :class="!selectedPersona.privateMemoryEnabled ? 'badge-primary' : 'badge-ghost'"
              :disabled="privateMemoryCounting || privateMemorySwitching"
              @click="setPrivateMemoryMode(false)"
            >
              {{ t('config.persona.global') }}
            </button>
            <button
              class="badge badge-sm cursor-pointer"
              :class="selectedPersona.privateMemoryEnabled ? 'badge-primary' : 'badge-ghost'"
              :disabled="privateMemoryCounting || privateMemorySwitching"
              @click="setPrivateMemoryMode(true)"
            >
              {{ t('config.persona.private') }}
            </button>
          </div>
        </div>
        <div class="flex justify-end">
          <button class="btn btn-xs btn-ghost" @click="triggerPersonaMemoryImport" :title="t('config.persona.import')">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>
            {{ t('config.persona.import') }}
          </button>
        </div>
      </div>
    </div>
    <div v-if="!selectedPersona.isBuiltInUser && privateMemoryError" class="text-xs text-error">
      {{ privateMemoryError }}
    </div>

    <input
      ref="personaMemoryImportInput"
      type="file"
      accept=".json,application/json"
      class="hidden"
      @change="onPersonaMemoryImportFile"
    />
  </div>

  <dialog ref="privateMemoryDialog" class="modal">
    <div class="modal-box max-w-md">
      <h3 class="text-sm font-semibold mb-2">{{ t('config.persona.closePrivateMemoryConfirm') }}</h3>
      <div v-if="privateMemoryCounting" class="flex items-center gap-2 text-sm">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{{ t('config.persona.countingMemory') }}</span>
      </div>
      <div v-else class="text-sm whitespace-pre-wrap leading-relaxed">{{ privateMemoryDialogMessage }}</div>
      <div v-if="!privateMemoryCounting && privateMemoryCount > 0" class="mt-3 rounded-box border border-warning/40 bg-warning/10 p-2 text-xs">
        <div class="font-medium">{{ t('config.persona.mustExportFirst') }}</div>
        <div class="opacity-70 mt-1">{{ t('config.persona.exportedConfirmUnlock') }}</div>
      </div>
      <div v-if="!privateMemoryCounting && privateMemoryCount > 0" class="mt-3">
        <button
          class="btn btn-sm btn-warning"
          :disabled="privateMemoryExporting || privateMemoryExported"
          @click="exportPrivateMemoriesBeforeDisable"
        >
          {{ privateMemoryExported ? t('config.persona.exported') : (privateMemoryExporting ? t('config.persona.exporting') : t('config.persona.exportPrivateMemory')) }}
        </button>
      </div>
      <div class="modal-action">
        <button class="btn btn-sm" :disabled="privateMemoryCounting || privateMemoryExporting || privateMemorySwitching" @click="cancelDisablePrivateMemory">{{ t('common.cancel') }}</button>
        <button
          class="btn btn-sm btn-primary"
          :disabled="privateMemoryCounting || privateMemoryExporting || privateMemorySwitching || (privateMemoryCount > 0 && !privateMemoryExported)"
          @click="confirmDisablePrivateMemory"
        >
          {{ t('common.confirm') }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="cancelDisablePrivateMemory">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Trash2 } from "lucide-vue-next";
import type { PersonaProfile } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";

const props = defineProps<{
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  personaEditorId: string;
  selectedPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  avatarSaving: boolean;
  avatarError: string;
}>();

const emit = defineEmits<{
  (e: "update:personaEditorId", value: string): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "openAvatarEditor"): void;
  (e: "importPersonaMemories", value: { agentId: string; file: File }): void;
}>();

const { t } = useI18n();
const personaMemoryImportInput = ref<HTMLInputElement | null>(null);
const privateMemoryDialog = ref<HTMLDialogElement | null>(null);
const privateMemoryCounting = ref(false);
const privateMemorySwitching = ref(false);
const privateMemoryExporting = ref(false);
const privateMemoryDialogMessage = ref("");
const privateMemoryError = ref("");
const privateMemoryCount = ref(0);
const privateMemoryExported = ref(false);
const pendingDisableAgentId = ref("");

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function triggerPersonaMemoryImport() {
  if (!personaMemoryImportInput.value) return;
  personaMemoryImportInput.value.value = "";
  personaMemoryImportInput.value.click();
}

function onPersonaMemoryImportFile(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  const agentId = props.selectedPersona?.id;
  if (!agentId) return;
  emit("importPersonaMemories", { agentId, file });
}

async function setPrivateMemoryMode(enabled: boolean) {
  const agentId = props.selectedPersona?.id;
  if (!agentId) return;
  const current = !!props.selectedPersona?.privateMemoryEnabled;
  if (current === enabled) return;
  privateMemoryError.value = "";
  if (enabled) {
    privateMemorySwitching.value = true;
    try {
      await invokeTauri("set_agent_private_memory_enabled", {
        input: { agentId, enabled: true },
      });
      if (props.selectedPersona) props.selectedPersona.privateMemoryEnabled = true;
    } catch (error) {
      privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
    } finally {
      privateMemorySwitching.value = false;
    }
    return;
  }
  pendingDisableAgentId.value = agentId;
  privateMemoryDialogMessage.value = "";
  privateMemoryCount.value = 0;
  privateMemoryExported.value = false;
  privateMemoryCounting.value = true;
  privateMemoryDialog.value?.showModal();
  try {
    const result = await invokeTauri<{ count: number }>("get_agent_private_memory_count", {
      input: { agentId },
    });
    const count = Math.max(0, Number(result.count || 0));
    privateMemoryCount.value = count;
    privateMemoryDialogMessage.value = count <= 0
      ? t('config.persona.noPrivateMemorySafe')
      : `t('config.persona.hasPrivateMemory', { count })\n\n请先点击“导出私有记忆”，导出成功后才可确认关闭。\n关闭后这些私有记忆将从本 App 永久删除。\n你需要手动重新导入才能恢复。`;
  } catch {
    privateMemoryCount.value = 0;
    privateMemoryDialogMessage.value = t('config.persona.countFailedButCanClose');
  } finally {
    privateMemoryCounting.value = false;
  }
}

function cancelDisablePrivateMemory() {
  pendingDisableAgentId.value = "";
  privateMemoryCount.value = 0;
  privateMemoryExported.value = false;
  privateMemoryExporting.value = false;
  privateMemoryDialog.value?.close();
}

async function exportPrivateMemoriesBeforeDisable() {
  const agentId = pendingDisableAgentId.value;
  if (!agentId || privateMemoryCount.value <= 0) return;
  privateMemoryError.value = "";
  privateMemoryExporting.value = true;
  try {
    const result = await invokeTauri<{ count: number; path: string }>("export_agent_private_memories", {
      input: { agentId },
    });
    privateMemoryExported.value = true;
    privateMemoryDialogMessage.value = `t('config.persona.exportSuccess', { count: result.count })\n路径：${result.path}\n\n现在可以点击“确认”关闭私有记忆。`;
  } catch (error) {
    privateMemoryExported.value = false;
    privateMemoryError.value = `导出失败：${String(error ?? "unknown")}`;
  } finally {
    privateMemoryExporting.value = false;
  }
}

async function confirmDisablePrivateMemory() {
  const agentId = pendingDisableAgentId.value;
  if (!agentId) {
    privateMemoryDialog.value?.close();
    return;
  }
  privateMemoryError.value = "";
  privateMemorySwitching.value = true;
  try {
    await invokeTauri("disable_agent_private_memory", {
      input: { agentId },
    });
    const persona = props.personas.find((p) => p.id === agentId);
    if (persona && !persona.isBuiltInUser) {
      persona.privateMemoryEnabled = false;
    }
    pendingDisableAgentId.value = "";
    privateMemoryCount.value = 0;
    privateMemoryExported.value = false;
    privateMemoryDialog.value?.close();
  } catch (error) {
    privateMemoryError.value = `${t('config.persona.switchFailed')}: ${String(error ?? "unknown")}`;
  } finally {
    privateMemorySwitching.value = false;
  }
}
</script>
