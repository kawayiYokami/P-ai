<template>
  <dialog
    ref="dialogRef"
    class="modal"
    @cancel.prevent="cancelCurrent"
    @keydown="onKeydown"
  >
    <div v-if="current" class="modal-box max-w-md">
      <h3 v-if="current.options.title" class="mb-2 font-semibold text-base">{{ current.options.title }}</h3>
      <p class="whitespace-pre-wrap break-words text-sm">{{ current.options.message }}</p>
      <input
        v-if="current.kind === 'prompt'"
        ref="inputRef"
        v-model="inputValue"
        :type="(current.options as PromptOptions).inputType || 'text'"
        :placeholder="(current.options as PromptOptions).placeholder || ''"
        class="input input-bordered mt-4 w-full"
        autocomplete="off"
      />
      <div class="modal-action mt-4">
        <button
          v-if="current.kind !== 'alert'"
          type="button"
          class="btn btn-ghost"
          @click="cancelCurrent"
        >{{ cancelText }}</button>
        <button
          ref="confirmRef"
          type="button"
          class="btn btn-primary"
          @click="confirmCurrent"
        >{{ confirmText }}</button>
      </div>
    </div>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { activeDialog, settleDialog, type PromptOptions } from "../services/confirm-dialog";

const { t } = useI18n();

const dialogRef = ref<HTMLDialogElement | null>(null);
const inputRef = ref<HTMLInputElement | null>(null);
const confirmRef = ref<HTMLButtonElement | null>(null);
const inputValue = ref("");

const current = computed(() => activeDialog.value);
const confirmText = computed(() => current.value?.options.confirmText || t("common.confirm"));
const cancelText = computed(() => {
  const options = current.value?.options;
  return options && "cancelText" in options && options.cancelText ? options.cancelText : t("common.cancel");
});

watch(current, async (dialog) => {
  inputValue.value = dialog?.kind === "prompt" ? (dialog.options as PromptOptions).defaultValue || "" : "";
  await nextTick();
  const element = dialogRef.value;
  if (!element) return;
  if (dialog && !element.open) element.showModal();
  else if (!dialog && element.open) element.close();
  if (!dialog) return;
  await nextTick();
  (dialog.kind === "prompt" ? inputRef.value : confirmRef.value)?.focus();
}, { immediate: true, flush: "post" });

function cancelCurrent() {
  const kind = current.value?.kind;
  void settleDialog(kind === "prompt" ? null : kind === "confirm" ? false : undefined);
}

function confirmCurrent() {
  const kind = current.value?.kind;
  void settleDialog(kind === "prompt" ? inputValue.value : kind === "confirm" ? true : undefined);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter") return;
  // 按钮上的 Enter 交给按钮自身的 click，其余（含 prompt 输入框）在此提交。
  if ((event.target as HTMLElement | null)?.tagName === "BUTTON") return;
  event.preventDefault();
  confirmCurrent();
}
</script>
