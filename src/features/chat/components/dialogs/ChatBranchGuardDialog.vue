<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

// 发送前的分支确认：仓库分支与本会话记录的分支不一致时，拦住这一次发送。
// 点「继续对话」才发送并把会话记录更新为当前分支；点「不发送」什么都不做，草稿留在输入框里。
const props = defineProps<{
  open: boolean;
  repoBranch: string;
  recordedBranch: string;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);

function syncDialog() {
  const d = dialogRef.value;
  if (!d) return;
  if (props.open) {
    if (!d.open) d.showModal();
  } else if (d.open) {
    d.close();
  }
}

// Esc 与点击遮罩都算「不发送」
function onDialogClose() {
  if (props.open) emit("cancel");
}

watch(() => props.open, syncDialog);
watch(dialogRef, syncDialog);
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box max-w-md">
      <h3 class="font-semibold text-base">{{ t("chat.branchGuardTitle") }}</h3>
      <p class="mt-2 text-balance text-sm opacity-75">
        {{
          t("chat.branchGuardMessage", {
            repoBranch: props.repoBranch,
            recordedBranch: props.recordedBranch,
          })
        }}
      </p>
      <p class="mt-1 text-balance text-xs opacity-60">
        {{ t("chat.branchGuardHint", { repoBranch: props.repoBranch }) }}
      </p>
      <div class="modal-action">
        <button
          type="button"
          class="btn btn-sm btn-neutral btn-soft"
          @click="emit('confirm')"
        >
          {{ t("chat.branchGuardConfirm") }}
        </button>
        <button type="button" class="btn btn-sm btn-neutral" @click="emit('cancel')">
          {{ t("chat.branchGuardCancel") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('cancel')">close</button>
    </form>
  </dialog>
</template>
