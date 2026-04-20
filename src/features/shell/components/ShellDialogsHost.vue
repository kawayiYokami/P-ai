<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { RuntimeLogEntry } from "../../../types/app";
import type { TerminalApprovalRequestPayload } from "../composables/use-terminal-approval";
import RuntimeLogsDialog from "./RuntimeLogsDialog.vue";
import TerminalApprovalDialog from "./TerminalApprovalDialog.vue";

type UpdateDialogKind = "error" | "info" | "warning";
type UpdateDialogPrimaryAction = "force" | "download" | "restart" | null | undefined;
type ConfigSaveErrorDialogKind = "warning" | "error";
type ArchiveImportPreview = {
  fileName: string;
  total: number;
  imported: number;
  replaced: number;
} | null;
type ForceArchivePreviewResult = {
  canArchive: boolean;
  canDropConversation: boolean;
  messageCount: number;
  hasAssistantReply: boolean;
  archiveDisabledReason?: string | null;
} | null;
type ForceCompactionPreviewResult = {
  canCompact: boolean;
  contextUsagePercent: number;
  compactionDisabledReason?: string | null;
} | null;

const props = defineProps<{
  updateDialogOpen: boolean;
  updateDialogTitle: string;
  updateDialogBody: string;
  updateDialogKind: UpdateDialogKind;
  updateDialogReleaseUrl?: string;
  updateDialogPrimaryAction?: UpdateDialogPrimaryAction;
  updateProgressPercent?: number | null;
  runtimeLogsDialogOpen: boolean;
  runtimeLogs: RuntimeLogEntry[];
  runtimeLogsLoading: boolean;
  runtimeLogsError: string;
  rewindConfirmDialogOpen: boolean;
  rewindConfirmCanUndoPatch: boolean;
  configSaveErrorDialogOpen: boolean;
  configSaveErrorDialogTitle: string;
  configSaveErrorDialogBody: string;
  configSaveErrorDialogKind: ConfigSaveErrorDialogKind;
  terminalApprovalDialogOpen: boolean;
  terminalApprovalCurrent: TerminalApprovalRequestPayload | null;
  terminalApprovalResolving: boolean;
  terminalApprovalQueueLength: number;
  archiveImportPreviewDialogOpen: boolean;
  archiveImportPreview: ArchiveImportPreview;
  archiveImportRunning: boolean;
  skillPlaceholderDialogOpen: boolean;
  forceArchiveActionDialogOpen: boolean;
  forceArchivePreviewLoading: boolean;
  forceArchivePreview: ForceArchivePreviewResult;
  forceCompactionPreview: ForceCompactionPreviewResult;
  forcingArchive: boolean;
}>();

const emit = defineEmits<{
  closeUpdateDialog: [];
  confirmUpdateDialogPrimary: [];
  openUpdateRelease: [];
  closeRuntimeLogsDialog: [];
  refreshRuntimeLogs: [];
  clearRuntimeLogs: [];
  confirmRewindWithPatch: [];
  confirmRewindMessageOnly: [];
  cancelRewindConfirm: [];
  closeConfigSaveErrorDialog: [];
  approveTerminalApproval: [];
  denyTerminalApproval: [];
  closeArchiveImportPreviewDialog: [];
  confirmArchiveImport: [];
  closeSkillPlaceholderDialog: [];
  confirmDeleteConversationFromArchiveDialog: [];
  confirmForceCompactionAction: [];
  confirmForceArchiveAction: [];
  closeForceArchiveActionDialog: [];
}>();

const { t } = useI18n();

function handleConfirmForceArchiveAction() {
  emit("confirmForceArchiveAction");
}

function handleCloseForceArchiveActionDialog() {
  emit("closeForceArchiveActionDialog");
}

function handleConfirmForceCompactionAction() {
  emit("confirmForceCompactionAction");
}

function handleConfirmDeleteConversationFromArchiveDialog() {
  emit("confirmDeleteConversationFromArchiveDialog");
}
</script>

<template>
  <dialog class="modal" :class="{ 'modal-open': updateDialogOpen }">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">
        {{ updateDialogTitle }}
      </h3>
      <pre
        class="mt-2 whitespace-pre-wrap text-sm"
        :class="updateDialogKind === 'error' ? 'text-error' : 'text-base-content'"
      >{{ updateDialogBody }}</pre>
      <progress
        v-if="typeof updateProgressPercent === 'number'"
        class="progress progress-primary mt-4 w-full"
        :value="Math.max(0, Math.min(100, updateProgressPercent))"
        max="100"
      />
      <div class="modal-action">
        <button
          v-if="updateDialogPrimaryAction"
          class="btn btn-sm btn-primary"
          @click="emit('confirmUpdateDialogPrimary')"
        >
          {{
            updateDialogPrimaryAction === 'force'
              ? '强制下载更新'
              : updateDialogPrimaryAction === 'restart'
                ? '更新并重启'
                : '下载更新'
          }}
        </button>
        <button
          v-if="updateDialogReleaseUrl"
          class="btn btn-sm"
          @click="emit('openUpdateRelease')"
        >
          打开 Releases
        </button>
        <button class="btn btn-sm" @click="emit('closeUpdateDialog')">
          {{ updateDialogPrimaryAction ? '取消' : '知道了' }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('closeUpdateDialog')">close</button>
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

  <dialog class="modal" :class="{ 'modal-open': rewindConfirmDialogOpen }">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">撤回选项</h3>
      <div class="mt-2 text-sm opacity-80">请选择本次撤回要执行的范围：</div>
      <div class="mt-4 flex flex-col items-center gap-2">
        <button
          v-if="rewindConfirmCanUndoPatch"
          class="btn btn-sm btn-error w-full"
          @click="emit('confirmRewindWithPatch')"
        >
          撤回消息并撤回修改
        </button>
        <button class="btn btn-sm w-full" @click="emit('confirmRewindMessageOnly')">
          仅撤回消息
        </button>
        <button class="btn btn-sm btn-primary w-full" @click="emit('cancelRewindConfirm')">取消</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('cancelRewindConfirm')">close</button>
    </form>
  </dialog>

  <dialog class="modal" :class="{ 'modal-open': configSaveErrorDialogOpen }">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">
        {{ configSaveErrorDialogTitle }}
      </h3>
      <pre
        class="mt-2 whitespace-pre-wrap text-sm"
        :class="configSaveErrorDialogKind === 'warning' ? 'text-warning' : 'text-error'"
      >{{ configSaveErrorDialogBody }}</pre>
      <div class="modal-action">
        <button class="btn btn-sm btn-primary" @click="emit('closeConfigSaveErrorDialog')">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('closeConfigSaveErrorDialog')">close</button>
    </form>
  </dialog>

  <TerminalApprovalDialog
    :open="terminalApprovalDialogOpen"
    :payload="terminalApprovalCurrent"
    :resolving="terminalApprovalResolving"
    :queue-length="terminalApprovalQueueLength"
    @approve="emit('approveTerminalApproval')"
    @deny="emit('denyTerminalApproval')"
  />

  <dialog class="modal" :class="{ 'modal-open': archiveImportPreviewDialogOpen }">
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
      <button @click.prevent="emit('closeArchiveImportPreviewDialog')">close</button>
    </form>
  </dialog>

  <dialog class="modal" :class="{ 'modal-open': skillPlaceholderDialogOpen }">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">Skill 列表</h3>
      <div class="mt-2 text-sm opacity-80">预留功能，暂未实现。</div>
      <div class="modal-action">
        <button class="btn btn-sm btn-primary" @click="emit('closeSkillPlaceholderDialog')">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('closeSkillPlaceholderDialog')">close</button>
    </form>
  </dialog>

  <dialog class="modal" :class="{ 'modal-open': forceArchiveActionDialogOpen }">
    <div class="modal-box w-[80vw] max-w-[80vw]">
      <h3 class="font-semibold text-base">处理当前会话</h3>
      <div v-if="forceArchivePreviewLoading" class="mt-3 text-sm opacity-70">正在判断当前会话适合压缩、归档还是丢弃...</div>
      <template v-else>
        <div class="mt-3 rounded-box border border-base-300 bg-base-200/40 px-3 py-3 text-sm">
          <div class="font-medium">压缩</div>
          <div class="mt-1 opacity-80">整理较早历史，保留当前会话继续聊。</div>
          <div class="mt-2 text-xs opacity-70">适合上下文占用偏高，但你还想继续当前话题时使用。</div>
          <div
            v-if="forceCompactionPreview?.compactionDisabledReason"
            class="mt-3 rounded border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-warning-content"
          >
            {{ forceCompactionPreview.compactionDisabledReason }}
          </div>
        </div>
        <div class="mt-3 rounded-box border border-base-300 bg-base-200/40 px-3 py-3 text-sm">
          <div class="font-medium">丢弃</div>
          <div class="mt-1 opacity-80">直接删除当前会话，不生成摘要，也不保留归档。</div>
          <div class="mt-2 text-xs opacity-70">适合测试、误触发，或确认这段内容不需要留痕时使用。</div>
        </div>
        <div class="mt-3 rounded-box border border-base-300 bg-base-200/40 px-3 py-3 text-sm">
          <div class="font-medium">归档</div>
          <div class="mt-1 opacity-80">生成结论汇报并提炼记忆，保留为归档记录。</div>
          <div class="mt-2 text-xs opacity-70">适合这段会话已经结束，准备沉淀为历史记录时使用。</div>
          <div
            v-if="forceArchivePreview?.archiveDisabledReason"
            class="mt-3 rounded border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-warning-content"
          >
            {{ forceArchivePreview.archiveDisabledReason }}
          </div>
        </div>
      </template>
      <div class="mt-4 flex items-end justify-between gap-4">
        <div class="text-xs opacity-60">
          <div>当前会话消息数：{{ forceArchivePreview?.messageCount ?? 0 }}</div>
          <div>当前上下文占用：{{ forceCompactionPreview?.contextUsagePercent ?? 0 }}%</div>
        </div>
        <div class="modal-action mt-0">
        <button
          class="btn btn-sm btn-error"
          :disabled="forceArchivePreviewLoading || !forceArchivePreview?.canDropConversation || forcingArchive"
          @click="handleConfirmDeleteConversationFromArchiveDialog"
        >
          丢弃
        </button>
        <button
          class="btn btn-sm btn-primary"
          :disabled="forceArchivePreviewLoading || !forceCompactionPreview?.canCompact || forcingArchive"
          @click="handleConfirmForceCompactionAction"
        >
          压缩
        </button>
        <button
          class="btn btn-sm btn-secondary"
          :disabled="forceArchivePreviewLoading || !forceArchivePreview?.canArchive || forcingArchive"
          @click="handleConfirmForceArchiveAction"
        >
          归档
        </button>
        <button class="btn btn-sm" :disabled="forceArchivePreviewLoading || forcingArchive" @click="handleCloseForceArchiveActionDialog">
          {{ t("common.cancel") }}
        </button>
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="handleCloseForceArchiveActionDialog">close</button>
    </form>
  </dialog>
</template>
