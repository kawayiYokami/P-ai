<template>
  <dialog ref="dialogRef" class="modal ecall-image-preview-modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box relative flex h-[92vh] max-h-[92vh] w-[92vw] max-w-[92vw] flex-col overflow-hidden rounded-box bg-black/95 p-0 text-white shadow-2xl">
      <button class="btn btn-lg btn-circle absolute right-4 top-4 z-20 border-0 bg-black/55 text-white/85 shadow-lg hover:bg-white/15 hover:text-white" type="button" title="关闭" aria-label="关闭" @click="emit('close')">
        <AppIcon name="close" class="h-7 w-7" />
      </button>
      <div class="absolute bottom-4 left-1/2 z-10 flex max-w-[calc(92vw-2rem)] -translate-x-1/2 items-center gap-1 rounded-2xl border border-white/18 bg-black/60 px-2 py-1.5 backdrop-blur-md shadow-xl">
        <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" :disabled="zoom <= minZoom" @click="emit('zoomOut')">
          <AppIcon name="minus" class="h-4 w-4" />
        </button>
        <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" :disabled="zoom >= maxZoom" @click="emit('zoomIn')">
          <AppIcon name="plus" class="h-4 w-4" />
        </button>
        <button class="btn btn-sm min-w-16 border-0 bg-transparent text-white/85 shadow-none hover:bg-white/12 hover:text-white" :disabled="Math.abs(zoom - 1) < 0.001" @click="emit('reset')">
          100%
        </button>
        <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" @click="emit('rotate')">
          <AppIcon name="rotate-cw" class="h-4 w-4" />
        </button>
        <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" type="button" title="在右栏打开" aria-label="在右栏打开" @click="emit('openInRightPanel')">
          <AppIcon name="panel-right" class="h-4 w-4" />
        </button>
        <template v-if="localPath && localFileSystemAvailable">
          <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" :disabled="copyStatus === 'doing'" @click="emit('copyImage', localPath)">
            <AppIcon name="copy" class="h-4 w-4" />
          </button>
          <button class="btn btn-sm btn-ghost border-0 text-white/85 shadow-none hover:bg-white/12 hover:text-white" :disabled="saveStatus === 'doing'" @click="emit('saveImage', localPath)">
            <AppIcon name="download" class="h-4 w-4" />
          </button>
        </template>
      </div>
      <div
        class="flex min-h-0 flex-1 items-center justify-center overflow-hidden p-0"
        :class="zoom > 1 ? (dragging ? 'cursor-grabbing' : 'cursor-grab') : ''"
        @wheel.prevent="emit('wheel', $event)"
        @pointermove="emit('pointerMove', $event)"
        @pointerup="emit('pointerUp', $event)"
        @pointercancel="emit('pointerUp', $event)"
        @pointerleave="emit('pointerUp', $event)"
      >
        <img
          v-if="dataUrl"
          :src="dataUrl"
          class="max-h-full max-w-full object-contain rounded select-none"
          draggable="false"
          :style="{ transform: `translate(${offsetX}px, ${offsetY}px) scale(${zoom}) rotate(${rotation}deg)`, transformOrigin: 'center center', touchAction: 'none' }"
          @dragstart.prevent
          @pointerdown="emit('pointerDown', $event)"
        />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import AppIcon from "../../../shared/components/AppIcon.vue";
import { getTransportCapabilities } from "../../../../services/tauri-api";

const localFileSystemAvailable = getTransportCapabilities().localFileSystem;

const props = defineProps<{
  open: boolean;
  dataUrl: string;
  zoom: number;
  minZoom: number;
  maxZoom: number;
  offsetX: number;
  offsetY: number;
  rotation: number;
  dragging: boolean;
  localPath?: string;
  copyStatus?: "idle" | "doing";
  saveStatus?: "idle" | "doing";
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "zoomIn"): void;
  (e: "zoomOut"): void;
  (e: "reset"): void;
  (e: "rotate"): void;
  (e: "wheel", event: WheelEvent): void;
  (e: "pointerDown", event: PointerEvent): void;
  (e: "pointerMove", event: PointerEvent): void;
  (e: "pointerUp", event: PointerEvent): void;
  (e: "copyImage", path: string): void;
  (e: "saveImage", path: string): void;
  (e: "openInRightPanel"): void;
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);

function onDialogClose() {
  emit("close");
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
