<template>
  <!-- 附件：输入卡内部第一层，图片预览 + 文件徽标 + IDE 桥胶囊，整区折叠 + 条目过渡 -->
  <div>
    <InputPanelCollapse :expanded="images.length > 0">
      <div class="input-panel-image-previews mb-1.5">
        <TransitionGroup name="input-panel-item" tag="div" class="contents">
          <div
            v-for="(img, idx) in images"
            :key="`input-panel-image-${idx}`"
            class="input-panel-image-preview"
          >
            <img
              v-if="img.previewDataUrl"
              class="input-panel-image-preview-media"
              :src="img.previewDataUrl"
              :alt="img.label || `图片 ${idx + 1}`"
              draggable="false"
            />
            <div v-else class="input-panel-file-preview">
              <FileText class="h-5 w-5" />
              <span class="text-xs">{{ img.label || `图片 ${idx + 1}` }}</span>
            </div>
            <button
              type="button"
              class="input-panel-image-remove"
              aria-label="删除图片"
              @pointerdown.prevent
              @mousedown.prevent
              @click.stop="$emit('removeImage', idx)"
            >
              <X class="h-3 w-3" />
            </button>
          </div>
        </TransitionGroup>
      </div>
    </InputPanelCollapse>
    <InputPanelCollapse :expanded="files.length > 0">
      <div class="relative mb-2 flex flex-wrap gap-1">
        <TransitionGroup name="input-panel-item" tag="div" class="contents">
          <div
            v-for="(file, idx) in files"
            :key="file.id"
            class="badge badge-ghost gap-1 py-3"
          >
            <span v-if="file.pending" class="loading loading-spinner loading-xs"></span>
            <FileText v-else class="h-3.5 w-3.5" />
            <span class="text-xs">{{ file.fileName }}</span>
            <button
              v-if="!file.pending"
              class="btn btn-ghost btn-sm btn-square"
              @pointerdown.prevent
              @mousedown.prevent
              @click="$emit('removeFile', idx)"
            >
              <X class="h-3 w-3" />
            </button>
          </div>
        </TransitionGroup>
      </div>
    </InputPanelCollapse>
    <InputPanelCollapse :expanded="bridges.length > 0">
      <div class="relative mb-2 flex flex-col gap-1">
        <div class="flex flex-wrap gap-1">
          <TransitionGroup name="input-panel-item" tag="div" class="contents">
            <button
              v-for="item in bridges"
              :key="item.id"
              type="button"
              class="input-panel-bridge ecall-sweep-fill gap-1 py-3 max-w-full badge"
              :data-attached="item.attached ? 'true' : 'false'"
              :title="item.title || item.fileName"
              @pointerdown.prevent
              @mousedown.prevent
              @click="$emit('toggleBridge', item.id)"
            >
              <Minus v-if="item.attached" class="h-3.5 w-3.5 shrink-0" />
              <Plus v-else class="h-3.5 w-3.5 shrink-0" />
              <span class="flex min-w-0 max-w-72 items-center text-xs">
                <span class="min-w-0 truncate">{{ item.fileName }}</span>
                <span
                  v-if="item.lineSuffix"
                  class="shrink-0 whitespace-nowrap"
                >{{ item.lineSuffix }}</span>
              </span>
            </button>
          </TransitionGroup>
        </div>
      </div>
    </InputPanelCollapse>
  </div>
</template>

<script setup lang="ts">
import { FileText, Minus, Plus, X } from "@lucide/vue";
import InputPanelCollapse from "./InputPanelCollapse.vue";

export type InputPanelImageItem = {
  mime: string;
  label?: string;
  previewDataUrl?: string;
};

export type InputPanelFileItem = {
  id: string;
  fileName: string;
  pending?: boolean;
};

export type InputPanelBridgeItem = {
  id: string;
  fileName: string;
  lineSuffix?: string;
  title?: string;
  attached?: boolean;
};

withDefaults(
  defineProps<{
    images?: InputPanelImageItem[];
    files?: InputPanelFileItem[];
    bridges?: InputPanelBridgeItem[];
  }>(),
  {
    images: () => [],
    files: () => [],
    bridges: () => [],
  },
);

defineEmits<{
  (e: "removeImage", index: number): void;
  (e: "removeFile", index: number): void;
  (e: "toggleBridge", id: string): void;
}>();
</script>

<style scoped>
.input-panel-image-previews {
  position: relative;
  display: flex;
  max-height: 4.5rem;
  flex-wrap: wrap;
  gap: 8px;
  overflow: hidden;
}
.input-panel-image-preview {
  position: relative;
  display: inline-flex;
  min-height: 3.5rem;
  max-width: 9.5rem;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-radius: 0.5rem;
  background: color-mix(in srgb, var(--color-base-200) 72%, transparent);
}
.input-panel-image-preview-media {
  display: block;
  max-height: 3.75rem;
  max-width: 9.5rem;
  object-fit: contain;
}
.input-panel-file-preview {
  display: inline-flex;
  height: 3.5rem;
  min-width: 5.75rem;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  padding: 0 0.75rem;
  color: color-mix(in srgb, var(--color-base-content) 72%, transparent);
}
.input-panel-image-remove {
  position: absolute;
  right: 4px;
  top: 4px;
  display: inline-flex;
  height: 1.25rem;
  width: 1.25rem;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-base-100) 88%, transparent);
  color: color-mix(in srgb, var(--color-base-content) 72%, transparent);
  opacity: 0;
  transition: opacity 120ms ease, color 120ms ease, background-color 120ms ease;
}
.input-panel-image-preview:hover .input-panel-image-remove,
.input-panel-image-remove:focus-visible {
  opacity: 1;
}
.input-panel-image-remove:hover {
  background: color-mix(in srgb, var(--color-error) 90%, transparent);
  color: var(--color-error-content);
}
.input-panel-bridge {
  border-color: transparent;
  background: var(--color-base-200);
  color: var(--color-base-content);
  --sweep-color: var(--color-primary);
  --sweep-color-2: transparent;
  --sweep-progress-2: 0;
}
.input-panel-bridge[data-attached="true"] {
  --sweep-progress: 1;
  border-color: transparent;
  color: var(--color-primary-content);
}
.input-panel-bridge[data-attached="false"] {
  --sweep-progress: 0;
}
.input-panel-item-enter-active,
.input-panel-item-leave-active {
  transition: opacity 180ms ease, transform 180ms ease;
}
.input-panel-item-enter-from,
.input-panel-item-leave-to {
  opacity: 0;
  transform: scale(0.92) translateY(4px);
}
.input-panel-item-leave-active {
  position: absolute;
}
.input-panel-item-move {
  transition: transform 220ms ease;
}
</style>
