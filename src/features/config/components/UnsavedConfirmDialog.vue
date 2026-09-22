<template>
  <dialog
    ref="dialogRef"
    class="modal"
    @close="handleDialogClose"
    @cancel.prevent="guard.handleCancel"
  >
    <div class="modal-box max-w-md border border-base-300 bg-base-100 shadow-2xl">
      <div class="flex items-start gap-3">
        <div class="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-warning/15 text-warning">
          <AlertTriangle class="h-5 w-5" />
        </div>
        <div class="min-w-0 flex-1">
          <h3 class="text-base font-semibold text-base-content">
            {{ guard.dialogOptions.value.title || t("config.unsavedConfirm.title") }}
          </h3>
          <p class="mt-2 text-sm text-base-content/70 leading-relaxed whitespace-pre-wrap">
            {{ guard.dialogOptions.value.message || t("config.unsavedConfirm.message") }}
          </p>
        </div>
      </div>

      <div class="modal-action mt-6 flex flex-wrap items-center justify-end gap-2">
        <template v-if="guard.dialogOptions.value.canSaveAndLeave">
          <button
            class="btn btn-sm btn-ghost text-error hover:bg-error/10"
            type="button"
            :disabled="guard.saving.value"
            @click="guard.handleDiscard"
          >
            {{ guard.dialogOptions.value.confirmDiscardText || t("config.unsavedConfirm.discard") }}
          </button>
          <button
            class="btn btn-sm btn-ghost"
            type="button"
            :disabled="guard.saving.value"
            @click="guard.handleCancel"
          >
            {{ guard.dialogOptions.value.cancelText || t("config.unsavedConfirm.stay") }}
          </button>
          <button
            class="btn btn-sm btn-primary gap-1.5"
            type="button"
            :disabled="guard.saving.value"
            @click="guard.handleSaveAndLeave"
          >
            <span v-if="guard.saving.value" class="loading loading-spinner loading-xs"></span>
            <Save v-else class="h-4 w-4" />
            <span>{{ guard.dialogOptions.value.saveAndLeaveText || t("config.unsavedConfirm.saveAndLeave") }}</span>
          </button>
        </template>

        <template v-else>
          <button
            class="btn btn-sm btn-ghost"
            type="button"
            @click="guard.handleCancel"
          >
            {{ guard.dialogOptions.value.cancelText || t("config.unsavedConfirm.stay") }}
          </button>
          <button
            class="btn btn-sm btn-warning"
            type="button"
            @click="guard.handleDiscard"
          >
            {{ guard.dialogOptions.value.confirmDiscardText || t("config.unsavedConfirm.discard") }}
          </button>
        </template>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close" @click.prevent="guard.handleCancel">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { AlertTriangle, Save } from "@lucide/vue";
import { useUnsavedChangesGuard } from "../composables/use-unsaved-changes-guard";

const { t } = useI18n();
const guard = useUnsavedChangesGuard();
const dialogRef = ref<HTMLDialogElement | null>(null);

watch(
  () => guard.dialogOpen.value,
  (open) => {
    if (open) {
      if (dialogRef.value && !dialogRef.value.open) {
        dialogRef.value.showModal();
      }
    } else {
      if (dialogRef.value?.open) {
        dialogRef.value.close();
      }
    }
  },
);

function handleDialogClose() {
  if (guard.dialogOpen.value) {
    guard.handleCancel();
  }
}
</script>
