<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ToolReviewCodePreview from "../ToolReviewCodePreview.vue";

const props = defineProps<{
  open: boolean;
  lang?: string;
  code: string;
  isDark?: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);
const copied = ref(false);
let copiedTimer = 0;

function onDialogClose() {
  requestClose();
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

const languageLabel = computed(() => String(props.lang || "").trim() || "text");

async function copyCode() {
  try {
    await navigator.clipboard.writeText(String(props.code || ""));
    copied.value = true;
    if (copiedTimer) window.clearTimeout(copiedTimer);
    copiedTimer = window.setTimeout(() => {
      copied.value = false;
      copiedTimer = 0;
    }, 1200);
  } catch {
    copied.value = false;
  }
}

function requestClose() {
  emit("close");
}

onBeforeUnmount(() => {
  if (copiedTimer) window.clearTimeout(copiedTimer);
});
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
      <div class="modal-box flex h-[95vh] max-h-[95vh] w-[95vw] max-w-none flex-col overflow-hidden p-0">
        <div class="flex items-center gap-2 border-b border-base-300 px-4 py-2">
          <div class="min-w-0 flex-1 truncate text-xs font-semibold">{{ languageLabel }}</div>
          <button
            type="button"
            class="btn btn-ghost btn-xs gap-1"
            :title="copied ? '已复制' : t('common.copy')"
            @click="copyCode"
          >
            <AppIcon name="copy" class="h-3.5 w-3.5" />
            <span>{{ copied ? "已复制" : t("common.copy") }}</span>
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-square btn-xs"
            :title="t('common.close')"
            @click="requestClose"
          >
            <AppIcon name="close" class="h-3.5 w-3.5" />
          </button>
        </div>
        <ToolReviewCodePreview
          v-if="open"
          class="min-h-0 flex-1"
          :code="code"
          :lang="languageLabel"
          :is-dark="isDark"
          show-line-numbers
        />
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="onDialogClose">close</button>
      </form>
    </dialog>
</template>
