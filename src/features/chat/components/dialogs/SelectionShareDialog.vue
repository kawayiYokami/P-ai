<script setup lang="ts">
import { ref, watch } from "vue";
import AppIcon from "../../../shared/components/AppIcon.vue";

const props = defineProps<{
  open: boolean;
  loading: boolean;
  messageText: string;
  titleText: string;
  hintText: string;
  imageText: string;
  htmlText: string;
  cancelText: string;
}>();

const emit = defineEmits<{
  close: [];
  exportImage: [];
  exportHtml: [];
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);

function requestClose() {
  if (props.loading) return;
  emit("close");
}

function onDialogClose() {
  requestClose();
  const d = dialogRef.value;
  if (props.loading && d && !d.open && props.open) d.showModal();
}

function syncDialog() {
  const d = dialogRef.value;
  if (!d) return;
  if (props.open) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(() => props.open, syncDialog);
watch(dialogRef, syncDialog);
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box max-w-lg">
      <h3 class="font-semibold text-base">{{ titleText }}</h3>
      <div class="mt-2 text-sm opacity-75">{{ messageText }}</div>
      <div v-if="!loading" class="mt-1 text-xs opacity-60">{{ hintText }}</div>
      <div v-if="loading" class="mt-4 flex items-center gap-2 text-sm opacity-70">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{{ hintText }}</span>
      </div>
      <div v-else class="mt-4 grid gap-3 sm:grid-cols-2">
        <button
          type="button"
          class="btn h-auto flex-col items-start gap-2 rounded-box border border-base-300 bg-base-100 px-4 py-4 text-left normal-case hover:border-primary/30 hover:bg-base-200"
          @click="emit('exportImage')"
        >
          <span class="inline-flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 text-primary">
            <AppIcon name="image" class="h-5 w-5" />
          </span>
          <span class="text-sm font-semibold">{{ imageText }}</span>
        </button>
        <button
          type="button"
          class="btn h-auto flex-col items-start gap-2 rounded-box border border-base-300 bg-base-100 px-4 py-4 text-left normal-case hover:border-primary/30 hover:bg-base-200"
          @click="emit('exportHtml')"
        >
          <span class="inline-flex h-10 w-10 items-center justify-center rounded-full bg-secondary/10 text-secondary">
            <AppIcon name="file-code" class="h-5 w-5" />
          </span>
          <span class="text-sm font-semibold">{{ htmlText }}</span>
        </button>
      </div>
      <div class="modal-action">
        <button type="button" class="btn btn-sm" :disabled="loading" @click="requestClose">
          {{ cancelText }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button
        :disabled="loading"
        :aria-disabled="loading ? 'true' : undefined"
        @click.prevent="requestClose"
      >
        close
      </button>
    </form>
  </dialog>
</template>
