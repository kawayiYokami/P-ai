<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import { AppMarkdownRenderer, initKatex } from "../../chat/markdown";
import type { RuntimeLogEntry } from "../../../types/app";
import RuntimeLogsDialog from "./RuntimeLogsDialog.vue";
import ConversationMaintenanceDialog from "../../chat/components/dialogs/ConversationMaintenanceDialog.vue";
import type {
  TrimCompactionPreviewResult,
  TrimPreviewResult,
} from "../../chat/composables/use-conversation-maintenance-dialog";

initKatex();

type UpdateDialogKind = "error" | "info" | "warning";
type UpdateDialogPrimaryAction = "force" | "download" | "restart" | null | undefined;
type PortablePendingProp = {
  plan: { staging_dir: string; target_dir: string; target_version?: string; log_path: string };
  backup_dir: string;
  failure_reason: string;
  created_at: string;
} | null;
type ConfigSaveErrorDialogKind = "warning" | "error";
type ArchiveImportPreview = {
  fileName: string;
  total: number;
  imported: number;
  replaced: number;
} | null;
const props = defineProps<{
  updateDialogOpen: boolean;
  updateDialogTitle: string;
  updateDialogBody: string;
  updateDialogKind: UpdateDialogKind;
  updateDialogReleaseUrl?: string;
  updateDialogPrimaryAction?: UpdateDialogPrimaryAction;
  updateProgressPercent?: number | null;
  updateDialogSkipVersionVisible?: boolean;
  updateDialogCancelUpdateVisible?: boolean;
  updateDialogCancelPending?: boolean;
  portablePending?: PortablePendingProp;
  isPortablePending?: boolean;
  markdownIsDark?: boolean;
  runtimeLogsDialogOpen: boolean;
  runtimeLogs: RuntimeLogEntry[];
  runtimeLogsLoading: boolean;
  runtimeLogsError: string;
  rewindConfirmDialogOpen: boolean;
  rewindConfirmCanUndoPatch: boolean;
  branchFromMessageConfirmDialogOpen: boolean;
  configSaveErrorDialogOpen: boolean;
  configSaveErrorDialogTitle: string;
  configSaveErrorDialogBody: string;
  configSaveErrorDialogKind: ConfigSaveErrorDialogKind;
  archiveImportPreviewDialogOpen: boolean;
  archiveImportPreview: ArchiveImportPreview;
  archiveImportRunning: boolean;
  skillPlaceholderDialogOpen: boolean;
  trimActionDialogOpen: boolean;
  trimPreviewLoading: boolean;
  trimPreview: TrimPreviewResult | null;
  trimCompactionPreview: TrimCompactionPreviewResult | null;
  trimming: boolean;
}>();

const emit = defineEmits<{
  closeUpdateDialog: [];
  confirmUpdateDialogPrimary: [];
  openPortablePendingDir: [kind: string];
  retryPortablePending: [];
  dismissPortablePending: [];
  openUpdateRelease: [];
  openUpdateRepository: [];
  skipUpdateVersion: [];
  cancelUpdate: [];
  closeRuntimeLogsDialog: [];
  refreshRuntimeLogs: [];
  clearRuntimeLogs: [];
  confirmRewindWithPatch: [];
  confirmRewindMessageOnly: [];
  cancelRewindConfirm: [];
  confirmBranchFromMessage: [];
  cancelBranchFromMessageConfirm: [];
  closeSettingsSaveErrorDialog: [];
  closeArchiveImportPreviewDialog: [];
  confirmArchiveImport: [];
  closeSkillPlaceholderDialog: [];
  confirmTrimCompactionAction: [];
  confirmTrimAction: [];
  confirmTrimDeleteAction: [];
  closeTrimActionDialog: [];
}>();

const { t } = useI18n();

const updateDialogRef = ref<HTMLDialogElement | null>(null);
const rewindConfirmDialogRef = ref<HTMLDialogElement | null>(null);
const branchFromMessageConfirmDialogRef = ref<HTMLDialogElement | null>(null);
const configSaveErrorDialogRef = ref<HTMLDialogElement | null>(null);
const archiveImportPreviewDialogRef = ref<HTMLDialogElement | null>(null);
const skillPlaceholderDialogRef = ref<HTMLDialogElement | null>(null);

function onUpdateDialogClose() {
  emit("closeUpdateDialog");
}
function syncUpdateDialog() {
  const d = updateDialogRef.value;
  if (!d) return;
  if (props.updateDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function onRewindConfirmDialogClose() {
  emit("cancelRewindConfirm");
}
function syncRewindConfirmDialog() {
  const d = rewindConfirmDialogRef.value;
  if (!d) return;
  if (props.rewindConfirmDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function onBranchFromMessageDialogClose() {
  emit("cancelBranchFromMessageConfirm");
}
function syncBranchFromMessageDialog() {
  const d = branchFromMessageConfirmDialogRef.value;
  if (!d) return;
  if (props.branchFromMessageConfirmDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function onConfigSaveErrorDialogClose() {
  emit("closeSettingsSaveErrorDialog");
}
function syncConfigSaveErrorDialog() {
  const d = configSaveErrorDialogRef.value;
  if (!d) return;
  if (props.configSaveErrorDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function onArchiveImportPreviewDialogClose() {
  if (props.archiveImportRunning) {
    const d = archiveImportPreviewDialogRef.value;
    if (d && !d.open && props.archiveImportPreviewDialogOpen) d.showModal();
    return;
  }
  emit("closeArchiveImportPreviewDialog");
}
function syncArchiveImportPreviewDialog() {
  const d = archiveImportPreviewDialogRef.value;
  if (!d) return;
  if (props.archiveImportPreviewDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function onSkillPlaceholderDialogClose() {
  emit("closeSkillPlaceholderDialog");
}
function syncSkillPlaceholderDialog() {
  const d = skillPlaceholderDialogRef.value;
  if (!d) return;
  if (props.skillPlaceholderDialogOpen) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(() => props.updateDialogOpen, syncUpdateDialog);
watch(updateDialogRef, syncUpdateDialog);
watch(() => props.rewindConfirmDialogOpen, syncRewindConfirmDialog);
watch(rewindConfirmDialogRef, syncRewindConfirmDialog);
watch(() => props.branchFromMessageConfirmDialogOpen, syncBranchFromMessageDialog);
watch(branchFromMessageConfirmDialogRef, syncBranchFromMessageDialog);
watch(() => props.configSaveErrorDialogOpen, syncConfigSaveErrorDialog);
watch(configSaveErrorDialogRef, syncConfigSaveErrorDialog);
watch(() => props.archiveImportPreviewDialogOpen, syncArchiveImportPreviewDialog);
watch(archiveImportPreviewDialogRef, syncArchiveImportPreviewDialog);
watch(() => props.skillPlaceholderDialogOpen, syncSkillPlaceholderDialog);
watch(skillPlaceholderDialogRef, syncSkillPlaceholderDialog);

function handleConfirmTrimAction() {
  emit("confirmTrimAction");
}

function handleConfirmTrimDeleteAction() {
  emit("confirmTrimDeleteAction");
}

function handleCloseTrimActionDialog() {
  emit("closeTrimActionDialog");
}

function handleConfirmTrimCompactionAction() {
  emit("confirmTrimCompactionAction");
}

function updateDialogCloseLabel() {
  if (props.updateDialogPrimaryAction) return t("common.cancel");
  if (props.updateDialogCancelUpdateVisible) return t("common.close");
  return t("common.confirm");
}

function canShowUpdateSecondaryActions() {
  return true;
}
</script>

<template>
  <dialog ref="updateDialogRef" class="modal" @close="onUpdateDialogClose" @cancel.prevent="onUpdateDialogClose">
    <div class="modal-box w-[min(92vw,48rem)] max-w-[48rem] flex max-h-[85dvh] flex-col overflow-hidden">
      <h3 class="font-semibold text-base">
        {{ updateDialogTitle }}
      </h3>
      <progress
        v-if="typeof updateProgressPercent === 'number'"
        class="progress progress-primary mt-3 w-full"
        :value="Math.max(0, Math.min(100, updateProgressPercent))"
        max="100"
      />
      <pre
        v-if="updateDialogOpen && updateDialogKind === 'error' && !isPortablePending"
        class="mt-3 min-h-0 flex-1 whitespace-pre-wrap break-words text-sm overflow-y-auto text-error"
      >{{ updateDialogBody }}</pre>
      <div v-else-if="updateDialogOpen && !isPortablePending" class="mt-3 min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
        <AppMarkdownRenderer
          :text="updateDialogBody"
          :is-dark="!!props.markdownIsDark"
          variant="document"
        />
      </div>
      <div v-if="isPortablePending && portablePending" class="mt-3 rounded-box border border-warning/30 bg-warning/10 p-3 text-xs">
        <div class="font-semibold">{{ t("about.portablePendingIntro") }}</div>
        <div class="mt-1 opacity-70">{{ t("about.portablePendingHint") }}</div>
      </div>
      <div class="modal-action mt-4 flex items-center justify-between gap-3">
        <button
          class="btn btn-sm btn-warning"
          @click="emit('openUpdateRepository')"
        >
          <AppIcon name="star" class="h-4 w-4" />
          {{ t("about.starAuthor") }}
        </button>
        <div class="flex items-center gap-2">
          <template v-if="isPortablePending">
            <button class="btn btn-sm btn-primary" @click="emit('openPortablePendingDir','pending')">{{ t("common.openFolder") }}</button>
            <button class="btn btn-sm btn-ghost" @click="emit('dismissPortablePending')">{{ t("common.close") }}</button>
          </template>
          <template v-else>
          <button
            v-if="updateDialogPrimaryAction"
            class="btn btn-sm btn-primary"
            @click="emit('confirmUpdateDialogPrimary')"
          >
            {{
              updateDialogPrimaryAction === 'force'
                ? t("dialogs.update.forceDownload")
                : updateDialogPrimaryAction === 'restart'
                  ? t("dialogs.update.restart")
                  : t("dialogs.update.download")
              }}
          </button>
          <button
            v-if="canShowUpdateSecondaryActions() && updateDialogCancelUpdateVisible"
            class="btn btn-sm btn-warning btn-outline"
            :disabled="updateDialogCancelPending"
            @click="emit('cancelUpdate')"
          >
            {{ updateDialogCancelPending ? t("about.cancellingUpdate") : t("about.cancelUpdate") }}
          </button>
          <button
            v-if="canShowUpdateSecondaryActions() && updateDialogReleaseUrl"
            class="btn btn-sm"
            @click="emit('openUpdateRelease')"
          >
            {{ t("dialogs.update.openReleases") }}
          </button>
          <button
            v-if="canShowUpdateSecondaryActions() && updateDialogSkipVersionVisible"
            class="btn btn-sm btn-ghost"
            @click="emit('skipUpdateVersion')"
          >
            {{ t("about.skipVersion") }}
          </button>
          <button class="btn btn-sm" @click="emit('closeUpdateDialog')">
            {{ updateDialogCloseLabel() }}
          </button>
          </template>
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onUpdateDialogClose">close</button>
    </form>
  </dialog>

  <RuntimeLogsDialog
    :open="runtimeLogsDialogOpen"
    :logs="runtimeLogs"
    :loading="runtimeLogsLoading"
    :error-text="runtimeLogsError"
    @close="emit('closeRuntimeLogsDialog')"
    @refresh="emit('refreshRuntimeLogs')"
    @clear="emit('clearRuntimeLogs')"
  />

  <dialog ref="rewindConfirmDialogRef" class="modal" @close="onRewindConfirmDialogClose" @cancel.prevent="onRewindConfirmDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">{{ t("dialogs.rewind.title") }}</h3>
      <div class="mt-2 text-sm opacity-80">{{ t("dialogs.rewind.hint") }}</div>
      <div class="mt-4 flex flex-col items-center gap-2">
        <button
          v-if="rewindConfirmCanUndoPatch"
          class="btn btn-sm btn-error w-full"
          @click="emit('confirmRewindWithPatch')"
        >
          {{ t("dialogs.rewind.withPatch") }}
        </button>
        <button class="btn btn-sm w-full" @click="emit('confirmRewindMessageOnly')">
          {{ t("dialogs.rewind.messageOnly") }}
        </button>
        <button class="btn btn-sm btn-primary w-full" @click="emit('cancelRewindConfirm')">{{ t("common.cancel") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onRewindConfirmDialogClose">close</button>
    </form>
  </dialog>

  <dialog ref="branchFromMessageConfirmDialogRef" class="modal" @close="onBranchFromMessageDialogClose" @cancel.prevent="onBranchFromMessageDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">{{ t("dialogs.branchFromMessage.title") }}</h3>
      <div class="mt-2 text-sm opacity-80">{{ t("dialogs.branchFromMessage.hint") }}</div>
      <div class="modal-action">
        <button class="btn btn-sm" @click="emit('cancelBranchFromMessageConfirm')">{{ t("common.cancel") }}</button>
        <button class="btn btn-sm btn-primary" @click="emit('confirmBranchFromMessage')">{{ t("dialogs.branchFromMessage.confirm") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onBranchFromMessageDialogClose">close</button>
    </form>
  </dialog>

  <dialog ref="configSaveErrorDialogRef" class="modal" @close="onConfigSaveErrorDialogClose" @cancel.prevent="onConfigSaveErrorDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">
        {{ configSaveErrorDialogTitle }}
      </h3>
      <pre
        class="mt-2 whitespace-pre-wrap text-sm"
        :class="configSaveErrorDialogKind === 'warning' ? 'text-warning' : 'text-error'"
      >{{ configSaveErrorDialogBody }}</pre>
      <div class="modal-action">
        <button class="btn btn-sm btn-primary" @click="emit('closeSettingsSaveErrorDialog')">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onConfigSaveErrorDialogClose">close</button>
    </form>
  </dialog>

  <dialog ref="archiveImportPreviewDialogRef" class="modal" @close="onArchiveImportPreviewDialogClose" @cancel.prevent="onArchiveImportPreviewDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">
        {{ t("archives.importPreviewTitle") }}
      </h3>
      <div v-if="archiveImportPreview" class="mt-3 space-y-1 text-sm">
        <div>{{ t("archives.importPreviewFile", { name: archiveImportPreview.fileName }) }}</div>
        <div>{{ t("archives.importPreviewTotal", { count: archiveImportPreview.total }) }}</div>
        <div>{{ t("archives.importPreviewAdd", { count: archiveImportPreview.imported }) }}</div>
        <div>{{ t("archives.importPreviewReplace", { count: archiveImportPreview.replaced }) }}</div>
        <div class="text-sm opacity-70 mt-2">{{ t("archives.importPreviewHint") }}</div>
      </div>
      <div class="modal-action">
        <button class="btn btn-sm" :disabled="archiveImportRunning" @click="emit('closeArchiveImportPreviewDialog')">
          {{ t("common.cancel") }}
        </button>
        <button class="btn btn-sm btn-primary" :disabled="archiveImportRunning" @click="emit('confirmArchiveImport')">
          {{ archiveImportRunning ? t("common.loading") : t("archives.importConfirm") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onArchiveImportPreviewDialogClose">close</button>
    </form>
  </dialog>

  <dialog ref="skillPlaceholderDialogRef" class="modal" @close="onSkillPlaceholderDialogClose" @cancel.prevent="onSkillPlaceholderDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">{{ t("dialogs.skill.title") }}</h3>
      <div class="mt-2 text-sm opacity-80">{{ t("dialogs.skill.placeholder") }}</div>
      <div class="modal-action">
        <button class="btn btn-sm btn-primary" @click="emit('closeSkillPlaceholderDialog')">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onSkillPlaceholderDialogClose">close</button>
    </form>
  </dialog>

  <ConversationMaintenanceDialog
    :open="trimActionDialogOpen"
    :loading="trimPreviewLoading"
    :running="trimming"
    :preview="trimPreview"
    :compaction-preview="trimCompactionPreview"
    @close="handleCloseTrimActionDialog"
    @confirm-compaction="handleConfirmTrimCompactionAction"
    @confirm-archive="handleConfirmTrimAction"
    @confirm-delete="handleConfirmTrimDeleteAction"
  />
</template>
